mod config;

use crate::config::{
    ConfigSampleCommand, load_pyproject_config, render_config_sample, resolve_config_path,
};
use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
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
    /// Path to pyproject.toml with [tool.ftl-extract.<command>] config
    #[arg(long)]
    config: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<Commands>,

    /// Verbose output
    #[arg(short = 'v', long, global = true, default_value_t = false)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    Config {
        #[command(subcommand)]
        command: ConfigCommands,
    },
    Extract {
        /// Path to the code directory
        #[arg()]
        code_path: Option<PathBuf>,

        /// Path to the output directory
        #[arg()]
        output_path: Option<PathBuf>,

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
    },
    Stub {
        /// Path to the FTL files directory
        #[arg()]
        ftl_path: Option<PathBuf>,

        /// Output path for the .pyi stub file
        #[arg()]
        output_path: Option<PathBuf>,

        /// Export intermediate tree structure as JSON
        #[arg(long, default_value_t = false)]
        export_tree: bool,
    },
    Untranslated {
        /// Path to locales directory containing locale folders (e.g. en, uk)
        #[arg()]
        locales_path: Option<PathBuf>,

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
        #[arg(long, value_enum)]
        output_format: Option<OutputFormat>,
    },
}

#[derive(Subcommand)]
enum ConfigCommands {
    Sample {
        /// Print only one command-specific pyproject.toml section
        #[arg(long, value_enum)]
        command: Option<ConfigSampleCommand>,
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

    let project_config = if matches!(cli.command, Some(Commands::Config { .. }) | None) {
        None
    } else {
        match load_pyproject_config(cli.config) {
            Ok(config) => config,
            Err(e) => {
                error!(target: "cli", "Error loading config: {}", e);
                std::process::exit(1);
            }
        }
    };

    let elapsed = match cli.command {
        Some(Commands::Config {
            command: ConfigCommands::Sample {
                command: sample_command,
            },
        }) => {
            println!("{}", render_config_sample(sample_command));
            None
        }
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
            let config_source = project_config.as_ref();
            let pyproject = config_source
                .and_then(|loaded| loaded.config.extract.clone())
                .unwrap_or_default();
            let base_dir = config_source
                .map(|loaded| loaded.base_dir.as_path())
                .unwrap_or_else(|| Path::new("."));

            let code_path = match resolve_required_path(
                code_path,
                pyproject.code_path,
                base_dir,
                "Missing code path. Pass it as an argument or set tool.ftl-extract.extract.code-path",
            ) {
                Ok(path) => path,
                Err(e) => exit_config_error(e),
            };
            let output_path = match resolve_required_path(
                output_path,
                pyproject.output_path,
                base_dir,
                "Missing output path. Pass it as an argument or set tool.ftl-extract.extract.output-path",
            ) {
                Ok(path) => path,
                Err(e) => exit_config_error(e),
            };
            let default_ftl_file = default_ftl_file
                .or(pyproject.default_ftl_file)
                .unwrap_or_else(|| PathBuf::from(DEFAULT_FTL_FILENAME));
            let cache_path = cli_or_config_path(cache_path, pyproject.cache_path, base_dir);
            let comment_keys_mode = match cli_or_config_enum(
                comment_keys_mode,
                pyproject.comment_keys_mode,
                "comment-keys-mode",
            ) {
                Ok(mode) => mode,
                Err(e) => exit_config_error(e),
            }
            .unwrap_or(CommentsKeyModes::Comment);
            let line_endings =
                match cli_or_config_enum(line_endings, pyproject.line_endings, "line-endings") {
                    Ok(line_endings) => line_endings,
                    Err(e) => exit_config_error(e),
                }
                .unwrap_or(LineEndings::Default);

            info!(target: "cli", "Code path: {}", code_path.display());
            info!(target: "cli", "Output path: {}", output_path.display());

            let mut i18n_keys_set: FastHashSet<String> = FastHashSet::from_iter(cli_or_config_vec(
                i18n_keys,
                pyproject.i18n_keys,
                DEFAULT_I18N_KEYS.iter().cloned().collect(),
            ));
            i18n_keys_set.extend(cli_or_config_vec(
                i18n_keys_append,
                pyproject.i18n_keys_append,
                Vec::new(),
            ));

            let mut exclude_dirs_set: FastHashSet<String> =
                FastHashSet::from_iter(cli_or_config_vec(
                    exclude_dirs,
                    pyproject.exclude_dirs,
                    DEFAULT_EXCLUDE_DIRS.iter().cloned().collect(),
                ));
            exclude_dirs_set.extend(cli_or_config_vec(
                exclude_dirs_append,
                pyproject.exclude_dirs_append,
                Vec::new(),
            ));

            let mut ignore_attributes_set: FastHashSet<String> =
                FastHashSet::from_iter(cli_or_config_vec(
                    ignore_attributes,
                    pyproject.ignore_attributes,
                    DEFAULT_IGNORE_ATTRIBUTES.iter().cloned().collect(),
                ));
            ignore_attributes_set.extend(cli_or_config_vec(
                append_ignore_attributes,
                pyproject.ignore_attributes_append,
                Vec::new(),
            ));

            let config = ExtractConfig {
                code_path,
                output_path,
                languages: cli_or_config_vec(language, pyproject.languages, vec!["en".to_string()]),
                i18n_keys: i18n_keys_set,
                i18n_keys_prefix: FastHashSet::from_iter(cli_or_config_vec(
                    i18n_keys_prefix,
                    pyproject.i18n_keys_prefix,
                    Vec::new(),
                )),
                exclude_dirs: exclude_dirs_set,
                ignore_attributes: ignore_attributes_set,
                ignore_kwargs: FastHashSet::from_iter(cli_or_config_vec(
                    ignore_kwargs,
                    pyproject.ignore_kwargs,
                    DEFAULT_IGNORE_KWARGS.iter().cloned().collect(),
                )),
                comment_junks: comment_junks || pyproject.comment_junks.unwrap_or(false),
                default_ftl_file,
                comment_keys_mode,
                line_endings,
                dry_run: dry_run || pyproject.dry_run.unwrap_or(false),
                cache: cache
                    || pyproject.cache.unwrap_or(false)
                    || cache_path.is_some()
                    || clear_cache
                    || pyproject.clear_cache.unwrap_or(false),
                cache_path,
                clear_cache: clear_cache || pyproject.clear_cache.unwrap_or(false),
            };

            let start_time = std::time::Instant::now();
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
            Some(start_time.elapsed())
        }
        Some(Commands::Stub {
            ftl_path,
            output_path,
            export_tree,
        }) => {
            let config_source = project_config.as_ref();
            let pyproject = config_source
                .and_then(|loaded| loaded.config.stub.clone())
                .unwrap_or_default();
            let base_dir = config_source
                .map(|loaded| loaded.base_dir.as_path())
                .unwrap_or_else(|| Path::new("."));
            let ftl_path = match resolve_required_path(
                ftl_path,
                pyproject.ftl_path,
                base_dir,
                "Missing FTL path. Pass it as an argument or set tool.ftl-extract.stub.ftl-path",
            ) {
                Ok(path) => path,
                Err(e) => exit_config_error(e),
            };
            let output_path = match resolve_required_path(
                output_path,
                pyproject.output_path,
                base_dir,
                "Missing output path. Pass it as an argument or set tool.ftl-extract.stub.output-path",
            ) {
                Ok(path) => path,
                Err(e) => exit_config_error(e),
            };

            info!(target: "cli", "FTL path: {}", ftl_path.display());
            info!(target: "cli", "Output path: {}", output_path.display());

            let config = StubConfig {
                ftl_path,
                output_path,
                export_tree: export_tree || pyproject.export_tree.unwrap_or(false),
            };

            let start_time = std::time::Instant::now();
            match generate_stub(config) {
                Ok(()) => {
                    info!(target: "cli", "Stub generation completed successfully");
                }
                Err(e) => {
                    error!(target: "cli", "Error during stub generation: {}", e);
                    std::process::exit(1);
                }
            }
            Some(start_time.elapsed())
        }
        Some(Commands::Untranslated {
            locales_path,
            language,
            suggest_from,
            fail_on_untranslated,
            output,
            output_format,
        }) => {
            let config_source = project_config.as_ref();
            let pyproject = config_source
                .and_then(|loaded| loaded.config.untranslated.clone())
                .unwrap_or_default();
            let base_dir = config_source
                .map(|loaded| loaded.base_dir.as_path())
                .unwrap_or_else(|| Path::new("."));
            let locales_path = match resolve_required_path(
                locales_path,
                pyproject.locales_path,
                base_dir,
                "Missing locales path. Pass it as an argument or set tool.ftl-extract.untranslated.locales-path",
            ) {
                Ok(path) => path,
                Err(e) => exit_config_error(e),
            };
            let output_format =
                match cli_or_config_enum(output_format, pyproject.output_format, "output-format") {
                    Ok(output_format) => output_format,
                    Err(e) => exit_config_error(e),
                };
            let output_format = output_format.unwrap_or(OutputFormat::Txt);
            let output = cli_or_config_path(output, pyproject.output, base_dir);

            info!(target: "cli", "Locales path: {}", locales_path.display());

            let config = CheckUntranslatedConfig {
                locales_path,
                locales: cli_or_config_vec(language, pyproject.languages, Vec::new()),
                suggest_from: cli_or_config_vec(suggest_from, pyproject.suggest_from, Vec::new()),
            };

            let start_time = std::time::Instant::now();
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

                    if (fail_on_untranslated || pyproject.fail_on_untranslated.unwrap_or(false))
                        && !result.untranslated.is_empty()
                    {
                        std::process::exit(1);
                    }
                }
                Err(e) => {
                    error!(target: "cli", "Error during untranslated check: {}", e);
                    std::process::exit(1);
                }
            }
            Some(start_time.elapsed())
        }
        None => {
            info!(target: "cli", "No command provided. Use --help for more information.");
            None
        }
    };

    if let Some(elapsed) = elapsed {
        info!(target: "cli", "✅ Done in {:.3?}s.", elapsed.as_secs_f64());
    }
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

fn cli_or_config_vec<T>(cli: Vec<T>, config: Option<Vec<T>>, default: Vec<T>) -> Vec<T> {
    if !cli.is_empty() {
        cli
    } else if let Some(config) = config {
        config
    } else {
        default
    }
}

fn cli_or_config_path(
    cli: Option<PathBuf>,
    config: Option<PathBuf>,
    base_dir: &Path,
) -> Option<PathBuf> {
    cli.or_else(|| resolve_config_path(config, base_dir))
}

fn resolve_required_path(
    cli: Option<PathBuf>,
    config: Option<PathBuf>,
    base_dir: &Path,
    error: &'static str,
) -> Result<PathBuf> {
    cli_or_config_path(cli, config, base_dir).context(error)
}

fn cli_or_config_enum<T>(cli: Option<T>, config: Option<String>, field: &str) -> Result<Option<T>>
where
    T: ValueEnum,
{
    if cli.is_some() {
        return Ok(cli);
    }

    let Some(config) = config else {
        return Ok(None);
    };

    T::from_str(&config, true).map(Some).map_err(|_| {
        let values = T::value_variants()
            .iter()
            .filter_map(|variant| variant.to_possible_value())
            .map(|value| value.get_name().to_string())
            .collect::<Vec<_>>()
            .join(", ");
        anyhow::anyhow!("Invalid `{field}` value `{config}`. Expected one of: {values}")
    })
}

fn exit_config_error(error: anyhow::Error) -> ! {
    error!(target: "cli", "Configuration error: {}", error);
    std::process::exit(1);
}
