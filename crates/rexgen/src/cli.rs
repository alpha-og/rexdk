// src/cli.rs

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use crate::metadata::{LOCK_SCHEMA_VERSION, SchemaLock, ServerVersion};

#[derive(Debug, Parser)]
#[command(name = "rexgen", about = "Generate Rust SDK bindings from the Rex API")]
pub struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Generate Rust bindings from a local API schema.
    Generate {
        #[arg(long, default_value = "api/rex-api.json")]
        spec: PathBuf,

        #[arg(long, default_value = "api/rex-api.lock.json")]
        lock: PathBuf,

        /// Directory for the generated module tree.
        #[arg(long, default_value = "crates/rexdk/src/bindings")]
        out: PathBuf,

        /// Fail if generated output is stale instead of writing it.
        #[arg(long)]
        check: bool,
    },

    /// Fetch the API schema and provenance from a Rex server.
    UpdateSchema {
        #[arg(long, default_value = "api/rex-api.json")]
        spec: PathBuf,

        #[arg(long, default_value = "api/rex-api.lock.json")]
        lock: PathBuf,

        /// Rex server socket.
        #[arg(long)]
        socket: Option<PathBuf>,
    },
}

pub async fn run() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Generate {
            spec,
            lock,
            out,
            check,
        } => generate(spec, lock, out, check),

        Command::UpdateSchema { spec, lock, socket } => update_schema(spec, lock, socket).await,
    }
}

fn generate(spec_path: PathBuf, lock_path: PathBuf, out_path: PathBuf, check: bool) -> Result<()> {
    let spec_bytes =
        fs::read(&spec_path).with_context(|| format!("reading schema {}", spec_path.display()))?;

    let spec = std::str::from_utf8(&spec_bytes).context("API schema is not valid UTF-8")?;

    let lock_bytes = fs::read(&lock_path)
        .with_context(|| format!("reading metadata {}", lock_path.display()))?;

    let schema_lock = SchemaLock::try_from(lock_bytes)
        .with_context(|| format!("parsing metadata {}", lock_path.display()))?;

    schema_lock.verify_integrity(&spec_bytes)?;

    let header = schema_lock.generate_header();
    let provenance = rexgen::Provenance {
        schema_version: schema_lock.schema_version,
        api_revision: schema_lock.api_revision,
        rex_version: schema_lock.rex_version.clone(),
        server_commit: schema_lock.server_commit.clone(),
        server_dirty: schema_lock.server_dirty,
        schema_sha256: schema_lock.schema_sha256.clone(),
    };

    if check {
        let files =
            rexgen::generate(spec, &out_path, &provenance).context("generating bindings")?;
        check_output(&out_path, &header, files)
    } else {
        write_output(&out_path, spec, &header, &provenance)
    }
}

fn write_output(
    out_path: &Path,
    spec: &str,
    header: &str,
    provenance: &rexgen::Provenance,
) -> Result<()> {
    let tmp_path = PathBuf::from(format!("{}.tmp", out_path.display()));

    remove_path(&tmp_path)?;
    fs::create_dir_all(&tmp_path).with_context(|| format!("creating {}", tmp_path.display()))?;

    let files = rexgen::generate(spec, &tmp_path, provenance).context("generating bindings")?;

    for file in &files {
        if let Some(parent) = file.path.parent() {
            fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
        }

        fs::write(&file.path, format!("{header}{}", file.contents))
            .with_context(|| format!("writing {}", file.path.display()))?;
    }

    remove_path(out_path)?;

    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    }

    fs::rename(&tmp_path, out_path)
        .with_context(|| format!("moving {} to {}", tmp_path.display(), out_path.display()))?;

    Ok(())
}

fn check_output(out_path: &Path, header: &str, files: Vec<rexgen::GeneratedFile>) -> Result<()> {
    let mut stale = Vec::new();
    let expected = files
        .iter()
        .map(|file| file.path.clone())
        .collect::<BTreeSet<_>>();

    for file in files {
        let expected_contents = format!("{header}{}", file.contents);

        match fs::read_to_string(&file.path) {
            Ok(actual) if actual == expected_contents => {}
            Ok(_) => stale.push(format!("{} (out of date)", file.path.display())),
            Err(_) => stale.push(format!("{} (missing)", file.path.display())),
        }
    }

    for path in collect_files(out_path)? {
        if !expected.contains(&path) {
            stale.push(format!("{} (unexpected)", path.display()));
        }
    }

    if stale.is_empty() {
        return Ok(());
    }

    bail!(
        "generated bindings are out of date:\n  {}",
        stale.join("\n  ")
    )
}

fn collect_files(dir: &Path) -> Result<Vec<PathBuf>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }

    if dir.is_file() {
        return Ok(vec![dir.to_path_buf()]);
    }

    let mut files = Vec::new();
    let mut pending = vec![dir.to_path_buf()];

    while let Some(current) = pending.pop() {
        for entry in
            fs::read_dir(&current).with_context(|| format!("reading {}", current.display()))?
        {
            let path = entry?.path();

            if path.is_dir() {
                pending.push(path);
            } else {
                files.push(path);
            }
        }
    }

    files.sort();

    Ok(files)
}

fn remove_path(path: &Path) -> Result<()> {
    if path.is_dir() {
        fs::remove_dir_all(path).with_context(|| format!("removing {}", path.display()))?;
    } else if path.exists() {
        fs::remove_file(path).with_context(|| format!("removing {}", path.display()))?;
    }

    Ok(())
}

async fn update_schema(
    spec_path: PathBuf,
    lock_path: PathBuf,
    socket_path: Option<PathBuf>,
) -> Result<()> {
    let transport = match socket_path {
        Some(path) => rexdk::Transport::UnixSocket { path },
        None => rexdk::Transport::default(),
    };
    let client_config = rexdk::ClientConfig {
        transport,
        ..Default::default()
    };
    let mut client = rexdk::Client::new(Some(client_config));
    client.connect().await?;
    let help = client
        .invoke::<(), serde_json::Value>("help.all", None)
        .await?;
    let server_version = client
        .invoke::<(), ServerVersion>("server.version", None)
        .await
        .context("fetching server version")?;

    let spec_bytes = serde_json::to_vec_pretty(&help)?;
    let schema_lock = SchemaLock::new(LOCK_SCHEMA_VERSION, server_version, &spec_bytes);
    let lock_bytes = serde_json::to_vec_pretty(&schema_lock)?;

    tokio::fs::write(&spec_path, spec_bytes)
        .await
        .with_context(|| format!("writing {}", spec_path.display()))?;

    tokio::fs::write(&lock_path, lock_bytes)
        .await
        .with_context(|| format!("writing {}", lock_path.display()))?;

    Ok(())
}
