use extractor::ftl::diagnostics as extractor_diagnostics;
use extractor::ftl::utils::FastHashSet;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct CheckUntranslatedConfig {
    pub locales_path: PathBuf,
    pub locales: Vec<String>,
    pub suggest_from: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct CheckSyntaxConfig {
    pub locales_path: PathBuf,
    pub locales: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct CheckReferencesConfig {
    pub locales_path: PathBuf,
    pub locales: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct CheckMissingConfig {
    pub locales_path: PathBuf,
    pub code_path: PathBuf,
    pub locales: Vec<String>,
    pub i18n_keys: FastHashSet<String>,
    pub i18n_keys_prefix: FastHashSet<String>,
    pub exclude_dirs: FastHashSet<String>,
    pub ignore_attributes: FastHashSet<String>,
    pub ignore_kwargs: FastHashSet<String>,
    pub default_ftl_file: PathBuf,
}

#[derive(Debug, Clone)]
pub struct CheckStaleConfig {
    pub locales_path: PathBuf,
    pub code_path: PathBuf,
    pub locales: Vec<String>,
    pub i18n_keys: FastHashSet<String>,
    pub i18n_keys_prefix: FastHashSet<String>,
    pub exclude_dirs: FastHashSet<String>,
    pub ignore_attributes: FastHashSet<String>,
    pub ignore_kwargs: FastHashSet<String>,
    pub default_ftl_file: PathBuf,
}

#[derive(Debug, Clone)]
pub struct CheckKwargsConfig {
    pub locales_path: PathBuf,
    pub code_path: PathBuf,
    pub locales: Vec<String>,
    pub i18n_keys: FastHashSet<String>,
    pub i18n_keys_prefix: FastHashSet<String>,
    pub exclude_dirs: FastHashSet<String>,
    pub ignore_attributes: FastHashSet<String>,
    pub ignore_kwargs: FastHashSet<String>,
    pub default_ftl_file: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warn,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warn => "warn",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticKind {
    Extraction,
    Kwargs,
    Missing,
    References,
    Stale,
    Syntax,
    Untranslated,
}

impl DiagnosticKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Extraction => "extraction",
            Self::Kwargs => "kwargs",
            Self::Missing => "missing",
            Self::References => "references",
            Self::Stale => "stale",
            Self::Syntax => "syntax",
            Self::Untranslated => "untranslated",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceLocation {
    pub path: PathBuf,
    pub line: Option<usize>,
    pub column: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranslationSuggestion {
    pub locale: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub kind: DiagnosticKind,
    pub locale: Option<String>,
    pub key: Option<String>,
    pub ftl_location: Option<SourceLocation>,
    pub code_location: Option<SourceLocation>,
    pub message: String,
    pub suggestions: Vec<TranslationSuggestion>,
    pub missing_kwargs: Vec<String>,
    pub unused_kwargs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckResult {
    pub checked_kinds: Vec<DiagnosticKind>,
    pub diagnostics: Vec<Diagnostic>,
}

impl CheckResult {
    pub fn error_count(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == Severity::Error)
            .count()
    }

    pub fn warning_count(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == Severity::Warn)
            .count()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UntranslatedKey {
    pub locale: String,
    pub file_path: PathBuf,
    pub key: String,
    pub value: String,
    pub line: Option<usize>,
    pub suggestions: Vec<TranslationSuggestion>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckUntranslatedResult {
    pub checked_locales: Vec<String>,
    pub fully_translated_locales: Vec<String>,
    pub untranslated: Vec<UntranslatedKey>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxError {
    pub locale: String,
    pub file_path: PathBuf,
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckSyntaxResult {
    pub checked_locales: Vec<String>,
    pub errors: Vec<SyntaxError>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingReference {
    pub locale: String,
    pub file_path: PathBuf,
    pub line: Option<usize>,
    pub key: Option<String>,
    pub reference: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckReferencesResult {
    pub checked_locales: Vec<String>,
    pub missing_references: Vec<MissingReference>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingKey {
    pub locale: String,
    pub key: String,
    pub expected_file_path: PathBuf,
    pub code_location: Option<SourceLocation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeExtractionError {
    pub key: String,
    pub message: String,
    pub locations: Vec<SourceLocation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckMissingResult {
    pub checked_locales: Vec<String>,
    pub missing_keys: Vec<MissingKey>,
    pub extraction_errors: Vec<CodeExtractionError>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaleKey {
    pub locale: String,
    pub file_path: PathBuf,
    pub line: Option<usize>,
    pub key: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckStaleResult {
    pub checked_locales: Vec<String>,
    pub stale_keys: Vec<StaleKey>,
    pub extraction_errors: Vec<CodeExtractionError>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KwargsMismatch {
    pub locale: String,
    pub key: String,
    pub file_path: PathBuf,
    pub line: Option<usize>,
    pub code_location: Option<SourceLocation>,
    pub missing_kwargs: Vec<String>,
    pub unused_kwargs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckKwargsResult {
    pub checked_locales: Vec<String>,
    pub mismatches: Vec<KwargsMismatch>,
    pub extraction_errors: Vec<CodeExtractionError>,
}

impl From<extractor_diagnostics::CodeLocation> for SourceLocation {
    fn from(location: extractor_diagnostics::CodeLocation) -> Self {
        Self {
            path: location.path,
            line: Some(location.line),
            column: Some(location.column),
        }
    }
}

impl From<CheckUntranslatedResult> for CheckResult {
    fn from(result: CheckUntranslatedResult) -> Self {
        let diagnostics = result
            .untranslated
            .into_iter()
            .map(|item| Diagnostic {
                severity: Severity::Error,
                kind: DiagnosticKind::Untranslated,
                locale: Some(item.locale.clone()),
                key: Some(item.key.clone()),
                ftl_location: Some(SourceLocation {
                    path: item.file_path.clone(),
                    line: item.line,
                    column: None,
                }),
                code_location: None,
                message: format!(
                    "key `{}` is untranslated in locale `{}`",
                    item.key, item.locale
                ),
                suggestions: item.suggestions,
                missing_kwargs: Vec::new(),
                unused_kwargs: Vec::new(),
            })
            .collect();

        Self {
            checked_kinds: vec![DiagnosticKind::Untranslated],
            diagnostics,
        }
    }
}

impl From<CheckSyntaxResult> for CheckResult {
    fn from(result: CheckSyntaxResult) -> Self {
        let diagnostics = result
            .errors
            .into_iter()
            .map(|item| Diagnostic {
                severity: Severity::Error,
                kind: DiagnosticKind::Syntax,
                locale: Some(item.locale.clone()),
                key: None,
                ftl_location: Some(SourceLocation {
                    path: item.file_path.clone(),
                    line: item.line,
                    column: item.column,
                }),
                code_location: None,
                message: format!(
                    "invalid Fluent syntax in locale `{}`: {}",
                    item.locale, item.message
                ),
                suggestions: Vec::new(),
                missing_kwargs: Vec::new(),
                unused_kwargs: Vec::new(),
            })
            .collect();

        Self {
            checked_kinds: vec![DiagnosticKind::Syntax],
            diagnostics,
        }
    }
}

impl From<CheckReferencesResult> for CheckResult {
    fn from(result: CheckReferencesResult) -> Self {
        let diagnostics = result
            .missing_references
            .into_iter()
            .map(|item| Diagnostic {
                severity: Severity::Error,
                kind: DiagnosticKind::References,
                locale: Some(item.locale.clone()),
                key: item.key.clone(),
                ftl_location: Some(SourceLocation {
                    path: item.file_path.clone(),
                    line: item.line,
                    column: None,
                }),
                code_location: None,
                message: format!(
                    "missing Fluent reference `{}` in locale `{}`",
                    item.reference, item.locale
                ),
                suggestions: Vec::new(),
                missing_kwargs: Vec::new(),
                unused_kwargs: Vec::new(),
            })
            .collect();

        Self {
            checked_kinds: vec![DiagnosticKind::References],
            diagnostics,
        }
    }
}

impl From<CheckMissingResult> for CheckResult {
    fn from(result: CheckMissingResult) -> Self {
        let mut diagnostics = result
            .missing_keys
            .into_iter()
            .map(|item| Diagnostic {
                severity: Severity::Error,
                kind: DiagnosticKind::Missing,
                locale: Some(item.locale.clone()),
                key: Some(item.key.clone()),
                ftl_location: Some(SourceLocation {
                    path: item.expected_file_path.clone(),
                    line: None,
                    column: None,
                }),
                code_location: item.code_location,
                message: format!(
                    "key `{}` is used in code but missing in locale `{}`",
                    item.key, item.locale
                ),
                suggestions: Vec::new(),
                missing_kwargs: Vec::new(),
                unused_kwargs: Vec::new(),
            })
            .collect::<Vec<_>>();

        diagnostics.extend(result.extraction_errors.into_iter().map(|item| Diagnostic {
            severity: Severity::Error,
            kind: DiagnosticKind::Extraction,
            locale: None,
            key: Some(item.key),
            ftl_location: None,
            code_location: item.locations.first().cloned(),
            message: item.message,
            suggestions: Vec::new(),
            missing_kwargs: Vec::new(),
            unused_kwargs: Vec::new(),
        }));

        Self {
            checked_kinds: vec![DiagnosticKind::Missing],
            diagnostics,
        }
    }
}

impl From<CheckStaleResult> for CheckResult {
    fn from(result: CheckStaleResult) -> Self {
        let mut diagnostics = result
            .stale_keys
            .into_iter()
            .map(|item| Diagnostic {
                severity: Severity::Error,
                kind: DiagnosticKind::Stale,
                locale: Some(item.locale.clone()),
                key: Some(item.key.clone()),
                ftl_location: Some(SourceLocation {
                    path: item.file_path.clone(),
                    line: item.line,
                    column: None,
                }),
                code_location: None,
                message: format!(
                    "key `{}` exists in locale `{}` but is not used in code",
                    item.key, item.locale
                ),
                suggestions: Vec::new(),
                missing_kwargs: Vec::new(),
                unused_kwargs: Vec::new(),
            })
            .collect::<Vec<_>>();

        diagnostics.extend(result.extraction_errors.into_iter().map(|item| Diagnostic {
            severity: Severity::Error,
            kind: DiagnosticKind::Extraction,
            locale: None,
            key: Some(item.key),
            ftl_location: None,
            code_location: item.locations.first().cloned(),
            message: item.message,
            suggestions: Vec::new(),
            missing_kwargs: Vec::new(),
            unused_kwargs: Vec::new(),
        }));

        Self {
            checked_kinds: vec![DiagnosticKind::Stale],
            diagnostics,
        }
    }
}

impl From<CheckKwargsResult> for CheckResult {
    fn from(result: CheckKwargsResult) -> Self {
        let mut diagnostics = result
            .mismatches
            .into_iter()
            .map(|item| Diagnostic {
                severity: Severity::Error,
                kind: DiagnosticKind::Kwargs,
                locale: Some(item.locale.clone()),
                key: Some(item.key.clone()),
                ftl_location: Some(SourceLocation {
                    path: item.file_path.clone(),
                    line: item.line,
                    column: None,
                }),
                code_location: item.code_location,
                message: format!(
                    "key `{}` in locale `{}` has variable mismatch",
                    item.key, item.locale
                ),
                suggestions: Vec::new(),
                missing_kwargs: item.missing_kwargs,
                unused_kwargs: item.unused_kwargs,
            })
            .collect::<Vec<_>>();

        diagnostics.extend(result.extraction_errors.into_iter().map(|item| Diagnostic {
            severity: Severity::Error,
            kind: DiagnosticKind::Extraction,
            locale: None,
            key: Some(item.key),
            ftl_location: None,
            code_location: item.locations.first().cloned(),
            message: item.message,
            suggestions: Vec::new(),
            missing_kwargs: Vec::new(),
            unused_kwargs: Vec::new(),
        }));

        Self {
            checked_kinds: vec![DiagnosticKind::Kwargs],
            diagnostics,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct MessageEntry {
    pub(crate) locale: String,
    pub(crate) file_path: PathBuf,
    pub(crate) key: String,
    pub(crate) value: Option<String>,
    pub(crate) line: Option<usize>,
    pub(crate) ignore_untranslated: bool,
}
