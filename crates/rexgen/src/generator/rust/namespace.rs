use heck::ToSnakeCase;

use crate::generator::rust::{Generator, error::RustGenerateError};

const RESERVED_SEGMENTS: [&str; 5] = ["self", "super", "crate", "types", "generated"];

const WINDOWS_RESERVED: [&str; 22] = [
    "con", "aux", "nul", "prn", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

/// A single namespace segment with both its raw API name and the canonical
/// (snake_case) Rust module name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModuleSegment {
    pub raw: String,
    pub canonical: String,
}

/// A validated namespace path for the Rust generator.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ModulePath(pub Vec<ModuleSegment>);

impl ModulePath {
    pub fn try_from_segments(segments: &[String]) -> Result<Self, RustGenerateError> {
        let segments = segments
            .iter()
            .map(|raw| {
                let canonical = raw.to_snake_case();
                validate_segment(raw, &canonical)?;

                Ok(ModuleSegment {
                    raw: raw.clone(),
                    canonical,
                })
            })
            .collect::<Result<Vec<_>, RustGenerateError>>()?;

        Ok(Self(segments))
    }
}

impl From<&ModulePath> for String {
    fn from(value: &ModulePath) -> Self {
        if value.0.is_empty() {
            return "<root>".to_owned();
        }

        value
            .0
            .iter()
            .map(|segment| segment.canonical.as_str())
            .collect::<Vec<_>>()
            .join("::")
    }
}

fn validate_segment(raw: &str, canonical: &str) -> Result<(), RustGenerateError> {
    let invalid = raw.is_empty()
        || canonical.is_empty()
        || canonical.starts_with(|c: char| c.is_ascii_digit())
        || !canonical
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
        || !raw
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        || RESERVED_SEGMENTS.contains(&canonical)
        || WINDOWS_RESERVED.contains(&canonical);

    if invalid {
        return Err(RustGenerateError::InvalidNamespace(raw.to_owned()));
    }

    Generator::generate_identifier(canonical, "module").map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(segments: &[&str]) -> Result<Vec<String>, RustGenerateError> {
        let segments = segments.iter().map(|s| s.to_string()).collect::<Vec<_>>();

        ModulePath::try_from_segments(&segments).map(|path| {
            path.0
                .into_iter()
                .map(|segment| segment.canonical)
                .collect()
        })
    }

    #[test]
    fn canonicalizes_segments() {
        assert_eq!(
            path(&["block-creator", "tailScale"]).unwrap(),
            ["block_creator", "tail_scale"]
        );
    }

    #[test]
    fn keywords_are_allowed() {
        assert_eq!(path(&["type"]).unwrap(), ["type"]);
    }

    #[test]
    fn rejects_reserved_and_invalid_segments() {
        for segment in ["self", "super", "crate", "types", "con", "", "a/b", "1abc"] {
            assert!(path(&[segment]).is_err(), "`{segment}` should be rejected");
        }
    }
}
