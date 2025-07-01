use clap::Parser;
use extractor::ftl::consts::{
    CommentsKeyModes, DEFAULT_EXCLUDE_DIRS, DEFAULT_FTL_FILENAME, DEFAULT_I18N_KEYS,
    DEFAULT_IGNORE_ATTRIBUTES, DEFAULT_IGNORE_KWARGS,
};
use extractor::ftl::ftl_extractor::extract;
use hashbrown::HashSet;
use std::path::PathBuf;

/// Fluent extractor CLI
#[derive(Parser, Debug)]
#[command(name = "fast-ftl-extract", version, about)]
struct Args {
    /// Path to the code directory
    #[arg()]
    code_path: PathBuf,

    /// Path to the output directory
    #[arg()]
    output_path: PathBuf,

    /// Language codes to extract
    #[arg(short = 'l', long, default_values_t = Vec::from(["en".to_string()]))]
    language: Vec<String>,

    /// Names of function that is used to get translation
    #[arg(short = 'k', long, default_values_t = DEFAULT_I18N_KEYS.clone())]
    i18n_keys: Vec<String>,

    /// Append names of function that is used to get translation
    #[arg(short = 'K', long, default_values_t = Vec::<String>::new())]
    i18n_keys_append: Vec<String>,

    /// Prefix names of function that is used to get translation. `self.i18n.*()`
    #[arg(short = 'p', long, default_values_t = Vec::<String>::new())]
    i18n_keys_prefix: Vec<String>,

    /// Exclude directories
    #[arg(short = 'e', long, default_values_t = DEFAULT_EXCLUDE_DIRS.clone())]
    exclude_dirs: Vec<String>,

    /// Append directories to exclude
    #[arg(short = 'E', long, default_values_t = Vec::<String>::new())]
    exclude_dirs_append: Vec<String>,

    /// Ignore attributes, e.g. `i18n.set_locale()`
    #[arg(short = 'i', long, default_values_t = DEFAULT_IGNORE_ATTRIBUTES.clone())]
    ignore_attributes: Vec<String>,

    /// Append attributes to ignore
    #[arg(short = 'I', long, default_values_t = Vec::<String>::new())]
    append_ignore_attributes: Vec<String>,

    /// Ignore kwargs, like `when` from `aiogram_dialog.I18nFormat(..., when=...)`
    #[arg(long, default_values_t = DEFAULT_IGNORE_KWARGS.clone())]
    ignore_kwargs: Vec<String>,

    /// Comment Junk elements
    #[arg(long, default_value_t = true)]
    comment_junks: bool,

    /// Default FTL filename
    #[arg(long, default_value = DEFAULT_FTL_FILENAME)]
    default_ftl_file: PathBuf,

    /// Comment keys mode
    #[arg(long, value_enum, default_value_t = CommentsKeyModes::Comment)]
    comment_keys_mode: CommentsKeyModes,

    /// Verbose output
    #[arg(short = 'v', long, default_value_t = false)]
    verbose: bool,

    /// Dry run, do not write to files
    #[arg(long, default_value_t = false)]
    dry_run: bool,
}

fn main() {
    let args = Args::parse();

    let start_time = std::time::Instant::now();
    println!("Code path: {}", args.code_path.display());
    println!("Output path: {}", args.output_path.display());
    let statistics = extract(
        &args.code_path,
        &args.output_path,
        args.language,
        HashSet::from_iter(args.i18n_keys),
        HashSet::from_iter(args.i18n_keys_append),
        HashSet::from_iter(args.i18n_keys_prefix),
        HashSet::from_iter(args.exclude_dirs),
        HashSet::from_iter(args.exclude_dirs_append),
        HashSet::from_iter(args.ignore_attributes),
        HashSet::from_iter(args.append_ignore_attributes),
        HashSet::from_iter(args.ignore_kwargs),
        args.comment_junks,
        &args.default_ftl_file,
        args.comment_keys_mode,
        args.dry_run,
    )
    .unwrap();

    if args.verbose {
        println!("Extraction statistics:");
        println!("  - Py files count: {}", statistics.py_files_count);
        println!("  - FTL files count: {:?}", statistics.ftl_files_count);
        println!(
            "  - FTL keys in code: {:?}",
            statistics.ftl_in_code_keys_count
        );
        println!(
            "  - FTL keys stored: {:?}",
            statistics.ftl_stored_keys_count
        );
        println!("  - FTL keys updated: {:?}", statistics.ftl_keys_updated);
        println!("  - FTL keys added: {:?}", statistics.ftl_keys_added);
        println!(
            "  - FTL keys commented: {:?}",
            statistics.ftl_keys_commented
        );
    }

    println!("[Rust] Done in {:?}", start_time.elapsed());
}
