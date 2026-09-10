use std::fmt;
use std::path::PathBuf;

/// Ordered by path, then line, then column, so conflicts can list their call sites in a
/// stable order.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct CodeLocation {
    pub path: PathBuf,
    pub line: usize,
    pub column: usize,
}

impl fmt::Display for CodeLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}:{}", self.path.display(), self.line, self.column)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExtractionDiagnosticKind {
    /// The same key is used with different `_path=` values.
    KeyPathConflict,
    /// The same key is used with different keyword arguments.
    KeyMessageConflict,
    /// The same key resolves to different Fluent entry types.
    KeyTypeConflict,
    /// A Python file could not be parsed.
    ParseError,
    /// A Python file could not be opened or mapped into memory.
    ReadError,
    /// A Python file is not valid UTF-8.
    InvalidUtf8,
}

impl ExtractionDiagnosticKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::KeyPathConflict => "key-path-conflict",
            Self::KeyMessageConflict => "key-message-conflict",
            Self::KeyTypeConflict => "key-type-conflict",
            Self::ParseError => "parse-error",
            Self::ReadError => "read-error",
            Self::InvalidUtf8 => "invalid-utf8",
        }
    }

    /// Diagnostics about a whole file that could not be processed, as opposed to
    /// conflicts between otherwise valid keys.
    pub fn is_file_error(self) -> bool {
        matches!(self, Self::ParseError | Self::ReadError | Self::InvalidUtf8)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExtractionDiagnostic {
    pub kind: ExtractionDiagnosticKind,
    /// The Fluent key the diagnostic is about. `None` for file-level errors.
    pub key: Option<String>,
    pub message: String,
    pub locations: Vec<CodeLocation>,
}

impl ExtractionDiagnostic {
    pub fn is_file_error(&self) -> bool {
        self.kind.is_file_error()
    }
}

impl fmt::Display for ExtractionDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.kind.as_str(), self.message)?;
        if !self.locations.is_empty() {
            let locations = self
                .locations
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ");
            write!(f, " ({locations})")?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExtractedFluentKey {
    pub key: String,
    pub ftl_path: PathBuf,
    pub code_location: Option<CodeLocation>,
    pub kwargs: Vec<String>,
    /// The first call site that passed `**kwargs`, if any. The key's variables cannot be
    /// verified then, and the `kwargs` check skips it.
    pub kwargs_unknown: Option<CodeLocation>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExtractedCode {
    pub keys: Vec<ExtractedFluentKey>,
    pub diagnostics: Vec<ExtractionDiagnostic>,
    pub py_files_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diagnostic_display_includes_kind_and_locations() {
        let diagnostic = ExtractionDiagnostic {
            kind: ExtractionDiagnosticKind::ParseError,
            key: None,
            message: "Failed to parse Python file: unexpected EOF".to_string(),
            locations: vec![CodeLocation {
                path: PathBuf::from("app.py"),
                line: 3,
                column: 7,
            }],
        };

        assert_eq!(
            diagnostic.to_string(),
            "[parse-error] Failed to parse Python file: unexpected EOF (app.py:3:7)"
        );
        assert!(diagnostic.is_file_error());
    }

    #[test]
    fn test_conflicts_are_not_file_errors() {
        for kind in [
            ExtractionDiagnosticKind::KeyPathConflict,
            ExtractionDiagnosticKind::KeyMessageConflict,
            ExtractionDiagnosticKind::KeyTypeConflict,
        ] {
            assert!(!kind.is_file_error(), "{}", kind.as_str());
        }
        for kind in [
            ExtractionDiagnosticKind::ParseError,
            ExtractionDiagnosticKind::ReadError,
            ExtractionDiagnosticKind::InvalidUtf8,
        ] {
            assert!(kind.is_file_error(), "{}", kind.as_str());
        }
    }
}
