use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct CheckUntranslatedConfig {
    pub locales_path: PathBuf,
    pub locales: Vec<String>,
    pub suggest_from: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranslationSuggestion {
    pub locale: String,
    pub value: String,
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

#[derive(Debug, Clone)]
pub(crate) struct MessageEntry {
    pub(crate) locale: String,
    pub(crate) file_path: PathBuf,
    pub(crate) key: String,
    pub(crate) value: Option<String>,
    pub(crate) line: Option<usize>,
    pub(crate) ignore_untranslated: bool,
}
