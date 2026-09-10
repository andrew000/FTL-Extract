use crate::config::ConfigSampleCommand;
use clap::{Parser, Subcommand};
use extractor::ftl::consts::{CommentsKeyModes, LineEndings};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "ftl", version, about)]
pub(crate) struct Cli {
    /// Path to pyproject.toml with [tool.ftl-extract.<command>] config
    #[arg(long)]
    pub(crate) config: Option<PathBuf>,

    #[command(subcommand)]
    pub(crate) command: Option<Commands>,

    /// Verbose output
    #[arg(short = 'v', long, global = true, default_value_t = false)]
    pub(crate) verbose: bool,
}

#[derive(Subcommand)]
pub(crate) enum Commands {
    Config {
        #[command(subcommand)]
        command: ConfigCommands,
    },
    Extract {
        /// Path to the code directory
        #[arg()]
        code_path: Option<PathBuf>,

        /// Path to the locales directory
        #[arg()]
        locales_path: Option<PathBuf>,

        /// Language codes to extract
        #[arg(short = 'l', long)]
        language: Vec<String>,

        /// Names of function that is used to get translation
        #[arg(short = 'k', long)]
        i18n_keys: Vec<String>,

        /// Append names of function that is used to get translation
        #[arg(short = 'K', long, default_values_t = Vec::<String>::new())]
        i18n_keys_append: Vec<String>,

        /// Prefix names of function that is used to get translation. `self.i18n.*()`
        #[arg(short = 'p', long, default_values_t = Vec::<String>::new())]
        i18n_keys_prefix: Vec<String>,

        /// Exclude directories
        #[arg(short = 'e', long)]
        exclude_dirs: Vec<String>,

        /// Append directories to exclude
        #[arg(short = 'E', long, default_values_t = Vec::<String>::new())]
        exclude_dirs_append: Vec<String>,

        /// Ignore attributes, e.g. `i18n.set_locale()`
        #[arg(short = 'i', long)]
        ignore_attributes: Vec<String>,

        /// Append attributes to ignore
        #[arg(short = 'I', long, default_values_t = Vec::<String>::new())]
        append_ignore_attributes: Vec<String>,

        /// Ignore kwargs, like `when` from `aiogram_dialog.I18nFormat(..., when=...)`
        #[arg(long)]
        ignore_kwargs: Vec<String>,

        /// Comment Junk elements
        #[arg(long, default_value_t = false)]
        comment_junks: bool,

        /// Default FTL filename
        #[arg(long)]
        default_ftl_file: Option<PathBuf>,

        /// Comment keys mode
        #[arg(long, value_enum)]
        comment_keys_mode: Option<CommentsKeyModes>,

        /// Line endings in output FTL files
        #[arg(long, value_enum)]
        line_endings: Option<LineEndings>,

        /// Dry run, do not write to files
        #[arg(long, default_value_t = false)]
        dry_run: bool,

        /// Cache Python extraction results between runs
        #[arg(long, default_value_t = false)]
        cache: bool,

        /// Directory or file path for the extraction cache
        #[arg(long)]
        cache_path: Option<PathBuf>,

        /// Clear the extraction cache before running
        #[arg(long, default_value_t = false)]
        clear_cache: bool,

        /// Skip Python files that cannot be read or parsed instead of aborting
        #[arg(long, default_value_t = false)]
        allow_parse_errors: bool,
    },
    Stub {
        /// Path to the FTL files directory
        #[arg()]
        locales_path: Option<PathBuf>,

        /// Path for the .pyi stub file
        #[arg()]
        stub_path: Option<PathBuf>,

        /// Export intermediate tree structure as JSON
        #[arg(long, default_value_t = false)]
        export_tree: bool,
    },
    Check {
        /// Path to locales directory containing locale folders (e.g. en, uk)
        #[arg()]
        locales_path: Option<PathBuf>,

        /// Path to Python code for checks that inspect source usage
        #[arg(long)]
        code_path: Option<PathBuf>,

        /// Checks to run
        #[arg(long = "check", value_enum)]
        checks: Vec<CheckKind>,

        /// Locale codes to check
        #[arg(short = 'l', long = "language", default_values_t = Vec::<String>::new())]
        language: Vec<String>,

        /// Suggest translations from these locales
        #[arg(long, default_values_t = Vec::<String>::new())]
        suggest_from: Vec<String>,

        /// Minimum diagnostic severity that should fail the command
        #[arg(long, value_enum, default_values_t = Vec::<FailSeverity>::new())]
        fail_on: Vec<FailSeverity>,

        /// Override the severity of a check, e.g. `--severity stale=error`
        #[arg(long, value_name = "CHECK=SEVERITY", default_values_t = Vec::<String>::new())]
        severity: Vec<String>,

        /// Report path for batch processing
        #[arg(long)]
        report_path: Option<PathBuf>,

        /// Format for report file
        #[arg(long, value_enum)]
        report_format: Option<CheckReportFormat>,
    },
}

#[derive(Subcommand)]
pub(crate) enum ConfigCommands {
    Sample {
        /// Print only one command-specific pyproject.toml section
        #[arg(long, value_enum)]
        command: Option<ConfigSampleCommand>,
    },
}

#[derive(PartialEq, Eq, Clone, Debug, clap::ValueEnum)]
pub(crate) enum CheckKind {
    All,
    Kwargs,
    Missing,
    References,
    Stale,
    Syntax,
    Untranslated,
}

#[derive(PartialEq, Clone, Debug, clap::ValueEnum)]
pub(crate) enum CheckReportFormat {
    Terminal,
    Json,
}

#[derive(PartialEq, Clone, Debug, clap::ValueEnum)]
pub(crate) enum FailSeverity {
    Error,
    Warn,
}
