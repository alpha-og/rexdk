// src/cli.rs

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use std::{fs, path::PathBuf};

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

        #[arg(long, default_value = "crates/rexdk/src/generated.rs")]
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
    let generated = rexgen::generate(spec).context("generating bindings")?;
    let output = format!("{header}{generated}");

    if check {
        let existing = fs::read_to_string(&out_path).ok();

        if existing.as_deref() != Some(output.as_str()) {
            bail!(
                "{} is out of date; re-run rexgen generate",
                out_path.display()
            );
        }
    } else {
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
        }

        fs::write(&out_path, output).with_context(|| format!("writing {}", out_path.display()))?;
    }

    Ok(())
}

async fn update_schema(
    spec_path: PathBuf,
    lock_path: PathBuf,
    socket_path: Option<PathBuf>,
) -> Result<()> {
    let client = rexdk::RexClient::connect(socket_path).await?;
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
