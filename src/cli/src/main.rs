use clap::{Parser, Subcommand};
use extractor::ftl::consts::{
    CommentsKeyModes, DEFAULT_EXCLUDE_DIRS, DEFAULT_FTL_FILENAME, DEFAULT_I18N_KEYS,
    DEFAULT_IGNORE_ATTRIBUTES, DEFAULT_IGNORE_KWARGS, LineEndings,
};
use extractor::ftl::ftl_extractor::{ExtractConfig, extract};

use hashbrown::HashSet;
use mimalloc::MiMalloc;
use std::path::PathBuf;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

/// Fast Fluent CLI
#[derive(Parser)]
#[command(name = "fast-ftl", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Verbose output
    #[arg(short = 'v', long, default_value_t = false)]
    verbose: bool,

    /// Silent mode, only output errors
    #[arg(long, default_value_t = false)]
    silent: bool,
}

#[derive(Subcommand)]
enum Commands {
    Extract {
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
        #[arg(long, default_value_t = false)]
        comment_junks: bool,

        /// Default FTL filename
        #[arg(long, default_value = DEFAULT_FTL_FILENAME)]
        default_ftl_file: PathBuf,

        /// Comment keys mode
        #[arg(long, value_enum, default_value_t = CommentsKeyModes::Comment)]
        comment_keys_mode: CommentsKeyModes,

        /// Line endings in output FTL files
        #[arg(long, value_enum, default_value_t = LineEndings::Default)]
        line_endings: LineEndings,

        /// Dry run, do not write to files
        #[arg(long, default_value_t = false)]
        dry_run: bool,
    },
}

fn main() {
    let cli = Cli::parse();
    let start_time = std::time::Instant::now();

    match cli.command {
        Some(Commands::Extract {
            code_path,
            output_path,
            language,
            i18n_keys,
            i18n_keys_append,
            i18n_keys_prefix,
            exclude_dirs,
            exclude_dirs_append,
            ignore_attributes,
            append_ignore_attributes,
            ignore_kwargs,
            comment_junks,
            default_ftl_file,
            comment_keys_mode,
            line_endings,
            dry_run,
        }) => {
            println!("Code path: {}", code_path.display());
            println!("Output path: {}", output_path.display());

            let mut i18n_keys_set: HashSet<String> = HashSet::from_iter(i18n_keys);
            i18n_keys_set.extend(i18n_keys_append);

            let mut exclude_dirs_set: HashSet<String> = HashSet::from_iter(exclude_dirs);
            exclude_dirs_set.extend(exclude_dirs_append);

            let mut ignore_attributes_set: HashSet<String> = HashSet::from_iter(ignore_attributes);
            ignore_attributes_set.extend(append_ignore_attributes);

            let config = ExtractConfig {
                code_path,
                output_path,
                languages: language,
                i18n_keys: i18n_keys_set,
                i18n_keys_prefix: HashSet::from_iter(i18n_keys_prefix),
                exclude_dirs: exclude_dirs_set,
                ignore_attributes: ignore_attributes_set,
                ignore_kwargs: HashSet::from_iter(ignore_kwargs),
                comment_junks,
                default_ftl_file,
                comment_keys_mode,
                line_endings,
                dry_run,
                silent: cli.silent,
            };

            match extract(config) {
                Ok(statistics) => {
                    if cli.verbose {
                        println!("Extraction statistics:");
                        println!("  - Py files count: {}", statistics.py_files_count);
                        println!("  - FTL files count: {:?}", statistics.ftl_files_count);
                        println!(
                            "  - FTL keys in code: {}",
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
                }
                Err(e) => {
                    eprintln!("Error during extraction: {}", e);
                    std::process::exit(1);
                }
            }
        }
        None => {
            println!("No command provided. Use --help for more information.");
        }
    }

    println!(
        "[Rust] Done in {:.3?}s.",
        start_time.elapsed().as_secs_f64()
    );
}
