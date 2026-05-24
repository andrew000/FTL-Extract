use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodeLocation {
    pub path: PathBuf,
    pub line: usize,
    pub column: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExtractionDiagnosticKind {
    KeyPathConflict,
    KeyMessageConflict,
    KeyTypeConflict,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExtractionDiagnostic {
    pub kind: ExtractionDiagnosticKind,
    pub key: String,
    pub message: String,
    pub locations: Vec<CodeLocation>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExtractedFluentKey {
    pub key: String,
    pub ftl_path: PathBuf,
    pub code_location: Option<CodeLocation>,
    pub kwargs: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExtractedCode {
    pub keys: Vec<ExtractedFluentKey>,
    pub diagnostics: Vec<ExtractionDiagnostic>,
    pub py_files_count: usize,
}
