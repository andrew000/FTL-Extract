use crate::ftl::utils::FastHashSet;
use std::sync::LazyLock;

pub const I18N_LITERAL: &str = "i18n";
pub const GET_LITERAL: &str = "get";
pub const PATH_LITERAL: &str = "_path";
pub const DEFAULT_FTL_FILENAME: &str = "_default.ftl";

pub static DEFAULT_I18N_KEYS: LazyLock<FastHashSet<String>> = LazyLock::new(|| {
    FastHashSet::from_iter([
        I18N_LITERAL.to_string(),
        "L".to_string(),
        "LazyProxy".to_string(),
        "LazyFilter".to_string(),
    ])
});

pub static DEFAULT_EXCLUDE_DIRS: LazyLock<FastHashSet<String>> = LazyLock::new(|| {
    FastHashSet::from_iter([
        "**/.venv/**".to_string(),
        "**/venv/**".to_string(),
        "**/.git/**".to_string(),
        "**/__pycache__/**".to_string(),
        "**/.pytest_cache/**".to_string(),
    ])
});

pub static DEFAULT_IGNORE_ATTRIBUTES: LazyLock<FastHashSet<String>> = LazyLock::new(|| {
    FastHashSet::from_iter([
        "set_locale".to_string(),
        "use_locale".to_string(),
        "use_context".to_string(),
        "set_context".to_string(),
    ])
});

pub static DEFAULT_IGNORE_KWARGS: LazyLock<FastHashSet<String>> =
    LazyLock::new(|| FastHashSet::default());

#[derive(PartialEq, Clone, Debug, clap::ValueEnum)]
pub enum CommentsKeyModes {
    Comment,
    Warn,
}

#[derive(PartialEq, Clone, Debug, clap::ValueEnum)]
pub enum LineEndings {
    Default,
    LF,
    CR,
    CRLF,
}
