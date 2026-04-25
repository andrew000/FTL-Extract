use clap::{Parser, Subcommand};
use extractor::ftl::consts::{
    CommentsKeyModes, DEFAULT_EXCLUDE_DIRS, DEFAULT_FTL_FILENAME, DEFAULT_I18N_KEYS,
    DEFAULT_IGNORE_ATTRIBUTES, DEFAULT_IGNORE_KWARGS, LineEndings,
};
use extractor::ftl::ftl_extractor::{ExtractConfig, extract};
use extractor::ftl::utils::FastHashSet;
use log::{error, info};
use mimalloc::MiMalloc;
use std::path::{Path, PathBuf};
use stub::{StubConfig, generate_stub};
use untranslated::{
    CheckUntranslatedConfig, check_untranslated, render_untranslated_json,
    render_untranslated_terminal, render_untranslated_txt,
};

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

#[derive(Parser)]
#[command(name = "ftl", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Verbose output
    #[arg(short = 'v', long, global = true, default_value_t = false)]
    verbose: bool,
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

        /// Cache Python extraction results between runs
        #[arg(long, default_value_t = false)]
        cache: bool,

        /// Directory or file path for the extraction cache
        #[arg(long)]
        cache_path: Option<PathBuf>,

        /// Clear the extraction cache before running
        #[arg(long, default_value_t = false)]
        clear_cache: bool,
    },
    Stub {
        /// Path to the FTL files directory
        #[arg()]
        ftl_path: PathBuf,

        /// Output path for the .pyi stub file
        #[arg()]
        output_path: PathBuf,

        /// Export intermediate tree structure as JSON
        #[arg(long, default_value_t = false)]
        export_tree: bool,
    },
    Untranslated {
        /// Path to locales directory containing locale folders (e.g. en, uk)
        #[arg()]
        locales_path: PathBuf,

        /// Locale codes to check (if omitted, all locale directories are checked)
        #[arg(short = 'l', long, default_values_t = Vec::<String>::new())]
        language: Vec<String>,

        /// Suggest translations from these locales
        #[arg(long, default_values_t = Vec::<String>::new())]
        suggest_from: Vec<String>,

        /// Exit with code 1 if untranslated keys are found
        #[arg(long, default_value_t = false)]
        fail_on_untranslated: bool,

        /// Output report path for batch processing
        #[arg(long)]
        output: Option<PathBuf>,

        /// Output format for report file
        #[arg(long, value_enum, default_value_t = OutputFormat::Txt)]
        output_format: OutputFormat,
    },
}

#[derive(PartialEq, Clone, Debug, clap::ValueEnum)]
enum OutputFormat {
    Txt,
    Json,
}

fn main() {
    let cli = Cli::parse();

    env_logger::Builder::new()
        .format_timestamp(None)
        .filter_level({
            if cli.verbose {
                log::LevelFilter::Debug
            } else {
                log::LevelFilter::Info
            }
        })
        .filter_module("ignore::walk", log::LevelFilter::Warn)
        .filter_module("ignore::gitignore", log::LevelFilter::Warn)
        .filter_module("globset", log::LevelFilter::Warn)
        .init();

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
            cache,
            cache_path,
            clear_cache,
        }) => {
            info!(target: "cli", "Code path: {}", code_path.display());
            info!(target: "cli", "Output path: {}", output_path.display());

            let mut i18n_keys_set: FastHashSet<String> = FastHashSet::from_iter(i18n_keys);
            i18n_keys_set.extend(i18n_keys_append);

            let mut exclude_dirs_set: FastHashSet<String> = FastHashSet::from_iter(exclude_dirs);
            exclude_dirs_set.extend(exclude_dirs_append);

            let mut ignore_attributes_set: FastHashSet<String> =
                FastHashSet::from_iter(ignore_attributes);
            ignore_attributes_set.extend(append_ignore_attributes);

            let config = ExtractConfig {
                code_path,
                output_path,
                languages: language,
                i18n_keys: i18n_keys_set,
                i18n_keys_prefix: FastHashSet::from_iter(i18n_keys_prefix),
                exclude_dirs: exclude_dirs_set,
                ignore_attributes: ignore_attributes_set,
                ignore_kwargs: FastHashSet::from_iter(ignore_kwargs),
                comment_junks,
                default_ftl_file,
                comment_keys_mode,
                line_endings,
                dry_run,
                cache: cache || cache_path.is_some() || clear_cache,
                cache_path,
                clear_cache,
            };

            match extract(config) {
                Ok(statistics) => {
                    info!(target: "cli", "Extraction statistics:");
                    info!(target: "cli", "  - Py files count: {}", statistics.py_files_count);
                    info!(target: "cli", "  - FTL files count: {:?}", statistics.ftl_files_count);
                    info!(target: "cli", "  - FTL keys in code: {}", statistics.ftl_in_code_keys_count);
                    info!(target: "cli", "  - FTL keys stored: {:?}", statistics.ftl_stored_keys_count);
                    info!(target: "cli", "  - FTL keys updated: {:?}", statistics.ftl_keys_updated);
                    info!(target: "cli", "  - FTL keys added: {:?}", statistics.ftl_keys_added);
                    info!(target: "cli", "  - FTL keys commented: {:?}", statistics.ftl_keys_commented);
                }
                Err(e) => {
                    error!(target: "cli", "Error during extraction: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Some(Commands::Stub {
            ftl_path,
            output_path,
            export_tree,
        }) => {
            info!(target: "cli", "FTL path: {}", ftl_path.display());
            info!(target: "cli", "Output path: {}", output_path.display());

            let config = StubConfig {
                ftl_path,
                output_path,
                export_tree,
            };

            match generate_stub(config) {
                Ok(()) => {
                    info!(target: "cli", "Stub generation completed successfully");
                }
                Err(e) => {
                    error!(target: "cli", "Error during stub generation: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Some(Commands::Untranslated {
            locales_path,
            language,
            suggest_from,
            fail_on_untranslated,
            output,
            output_format,
        }) => {
            info!(target: "cli", "Locales path: {}", locales_path.display());

            let config = CheckUntranslatedConfig {
                locales_path,
                locales: language,
                suggest_from,
            };

            match check_untranslated(config) {
                Ok(result) => {
                    println!("{}", render_untranslated_terminal(&result));

                    if let Some(output_path) = output {
                        let output_path = normalize_output_path(output_path, &output_format);
                        let output_content = match output_format {
                            OutputFormat::Txt => render_untranslated_txt(&result),
                            OutputFormat::Json => render_untranslated_json(&result),
                        };

                        if let Err(e) = write_output_file(&output_path, output_content) {
                            error!(
                                target: "cli",
                                "Failed to write output file `{}`: {}",
                                output_path.display(),
                                e
                            );
                            std::process::exit(1);
                        }

                        info!(target: "cli", "Saved report to {}", output_path.display());
                    }

                    if fail_on_untranslated && !result.untranslated.is_empty() {
                        std::process::exit(1);
                    }
                }
                Err(e) => {
                    error!(target: "cli", "Error during untranslated check: {}", e);
                    std::process::exit(1);
                }
            }
        }
        None => {
            info!(target: "cli", "No command provided. Use --help for more information.");
        }
    }

    info!(target: "cli", "✅ Done in {:.3?}s.", start_time.elapsed().as_secs_f64()
    );
}

fn write_output_file(path: &Path, content: String) -> std::io::Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, content)
}

fn normalize_output_path(path: PathBuf, format: &OutputFormat) -> PathBuf {
    if path.extension().is_some() {
        return path;
    }

    let suffix = match format {
        OutputFormat::Txt => "txt",
        OutputFormat::Json => "json",
    };

    path.with_extension(suffix)
}
