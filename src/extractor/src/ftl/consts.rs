use hashbrown::HashSet;
use lazy_static::lazy_static;

pub const I18N_LITERAL: &str = "i18n";
pub const GET_LITERAL: &str = "get";
pub const PATH_LITERAL: &str = "_path";
pub const DEFAULT_FTL_FILENAME: &str = "_default.ftl";

lazy_static! {
    pub static ref DEFAULT_I18N_KEYS: HashSet<String> = HashSet::from([
        I18N_LITERAL.to_string(),
        "L".to_string(),
        "LazyProxy".to_string(),
        "LazyFilter".to_string()
    ]);
    pub static ref DEFAULT_EXCLUDE_DIRS: HashSet<String> = HashSet::from([
        "**/.venv/**".to_string(),
        "**/venv/**".to_string(),
        "**/.git/**".to_string(),
        "**/__pycache__/**".to_string(),
        "**/.pytest_cache/**".to_string(),
    ]);
    pub static ref DEFAULT_IGNORE_ATTRIBUTES: HashSet<String> = HashSet::from([
        "set_locale".to_string(),
        "use_locale".to_string(),
        "use_context".to_string(),
        "set_context".to_string()
    ]);
    pub static ref DEFAULT_IGNORE_KWARGS: HashSet<String> = HashSet::new();
}

#[derive(PartialEq, Clone, Debug, clap::ValueEnum)]
pub enum CommentsKeyModes {
    Comment,
    Warn,
}
