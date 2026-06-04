mod config;

use crate::config::{
    ConfigSampleCommand, load_pyproject_config, render_config_sample, resolve_config_path,
};
use anyhow::{Context, Result};
use check::{
    CheckCodeConfig, CheckLocaleCache, CheckResult, Diagnostic, DiagnosticKind, Severity,
    check_kwargs_with_cache, check_missing_with_cache, check_references_with_cache,
    check_stale_with_cache, check_syntax_with_cache, check_untranslated_with_cache,
    code_extraction_errors, extract_check_code, has_failing_diagnostics, render_check_json,
    render_check_terminal, validate_check_locales,
};
use clap::{Parser, Subcommand, ValueEnum};
use extractor::ftl::consts::{
    CommentsKeyModes, DEFAULT_EXCLUDE_DIRS, DEFAULT_FTL_FILENAME, DEFAULT_I18N_KEYS,
    DEFAULT_IGNORE_ATTRIBUTES, DEFAULT_IGNORE_KWARGS, LineEndings,
};
use extractor::ftl::diagnostics::ExtractedCode;
use extractor::ftl::ftl_extractor::{ExtractConfig, extract};
use extractor::ftl::utils::FastHashSet;
use log::{error, info};
use mimalloc::MiMalloc;
use std::path::{Path, PathBuf};
use stub::{StubConfig, generate_stub};

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

        /// Output report path for batch processing
        #[arg(long)]
        output: Option<PathBuf>,

        /// Output format for report file
        #[arg(long, value_enum)]
        output_format: Option<CheckOutputFormat>,
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

#[derive(PartialEq, Eq, Clone, Debug, clap::ValueEnum)]
enum CheckKind {
    All,
    Kwargs,
    Missing,
    References,
    Stale,
    Syntax,
    Untranslated,
}

#[derive(PartialEq, Clone, Debug, clap::ValueEnum)]
enum CheckOutputFormat {
    Terminal,
    Json,
}

#[derive(PartialEq, Clone, Debug, clap::ValueEnum)]
enum FailSeverity {
    Error,
    Warn,
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
        Some(Commands::Check {
            locales_path,
            code_path,
            checks,
            language,
            suggest_from,
            fail_on,
            output,
            output_format,
        }) => {
            let config_source = project_config.as_ref();
            let pyproject = config_source
                .and_then(|loaded| loaded.config.check.clone())
                .unwrap_or_default();
            let extract_pyproject = config_source
                .and_then(|loaded| loaded.config.extract.clone())
                .unwrap_or_default();
            let base_dir = config_source
                .map(|loaded| loaded.base_dir.as_path())
                .unwrap_or_else(|| Path::new("."));
            let locales_path = match resolve_required_path(
                locales_path,
                pyproject.locales_path,
                base_dir,
                "Missing locales path. Pass it as an argument or set tool.ftl-extract.check.locales-path",
            ) {
                Ok(path) => path,
                Err(e) => exit_config_error(e),
            };
            let checks =
                match cli_or_config_enum_vec(checks, pyproject.checks, "checks", Vec::new()) {
                    Ok(checks) => checks,
                    Err(e) => exit_config_error(e),
                };
            let checks = expand_check_kinds(checks);
            let code_path = cli_or_config_path(
                code_path,
                pyproject
                    .code_path
                    .clone()
                    .or_else(|| extract_pyproject.code_path.clone()),
                base_dir,
            );
            let cache_path =
                cli_or_config_path(None, extract_pyproject.cache_path.clone(), base_dir);
            let output_format =
                match cli_or_config_enum(output_format, pyproject.output_format, "output-format") {
                    Ok(output_format) => output_format,
                    Err(e) => exit_config_error(e),
                };
            let output_format = output_format.unwrap_or(CheckOutputFormat::Json);
            let output = cli_or_config_path(output, pyproject.output, base_dir);
            let fail_on = match cli_or_config_enum_vec(
                fail_on,
                pyproject.fail_on,
                "fail-on",
                vec![FailSeverity::Error],
            ) {
                Ok(fail_on) => fail_on.into_iter().map(Into::into).collect::<Vec<_>>(),
                Err(e) => exit_config_error(e),
            };

            info!(target: "cli", "Locales path: {}", locales_path.display());
            if let Some(code_path) = &code_path {
                info!(target: "cli", "Code path: {}", code_path.display());
            }

            let mut i18n_keys_set: FastHashSet<String> = FastHashSet::from_iter(
                extract_pyproject
                    .i18n_keys
                    .unwrap_or_else(|| DEFAULT_I18N_KEYS.iter().cloned().collect()),
            );
            i18n_keys_set.extend(extract_pyproject.i18n_keys_append.unwrap_or_default());

            let mut exclude_dirs_set: FastHashSet<String> = FastHashSet::from_iter(
                extract_pyproject
                    .exclude_dirs
                    .unwrap_or_else(|| DEFAULT_EXCLUDE_DIRS.iter().cloned().collect()),
            );
            exclude_dirs_set.extend(extract_pyproject.exclude_dirs_append.unwrap_or_default());

            let mut ignore_attributes_set: FastHashSet<String> = FastHashSet::from_iter(
                extract_pyproject
                    .ignore_attributes
                    .unwrap_or_else(|| DEFAULT_IGNORE_ATTRIBUTES.iter().cloned().collect()),
            );
            ignore_attributes_set.extend(
                extract_pyproject
                    .ignore_attributes_append
                    .unwrap_or_default(),
            );

            let config = CheckRunConfig {
                locales_path,
                code_path,
                locales: cli_or_config_vec(language, pyproject.languages, Vec::new()),
                suggest_from: cli_or_config_vec(suggest_from, pyproject.suggest_from, Vec::new()),
                i18n_keys: i18n_keys_set,
                i18n_keys_prefix: FastHashSet::from_iter(
                    extract_pyproject.i18n_keys_prefix.unwrap_or_default(),
                ),
                exclude_dirs: exclude_dirs_set,
                ignore_attributes: ignore_attributes_set,
                ignore_kwargs: FastHashSet::from_iter(
                    extract_pyproject
                        .ignore_kwargs
                        .unwrap_or_else(|| DEFAULT_IGNORE_KWARGS.iter().cloned().collect()),
                ),
                default_ftl_file: extract_pyproject
                    .default_ftl_file
                    .unwrap_or_else(|| PathBuf::from(DEFAULT_FTL_FILENAME)),
                cache: extract_pyproject.cache.unwrap_or(false)
                    || cache_path.is_some()
                    || extract_pyproject.clear_cache.unwrap_or(false),
                cache_path,
                clear_cache: extract_pyproject.clear_cache.unwrap_or(false),
            };

            let start_time = std::time::Instant::now();
            match run_check(checks, config) {
                Ok(result) => {
                    println!("{}", render_check_terminal(&result));

                    if let Some(output_path) = output {
                        let output_path = normalize_output_path(output_path, &output_format);
                        let output_content = match output_format {
                            CheckOutputFormat::Terminal => render_check_terminal(&result),
                            CheckOutputFormat::Json => render_check_json(&result),
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

                    if has_failing_diagnostics(&result, &fail_on) {
                        std::process::exit(1);
                    }
                }
                Err(e) => {
                    error!(target: "cli", "Error during check: {}", e);
                    std::process::exit(2);
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

#[derive(Clone)]
struct CheckRunConfig {
    locales_path: PathBuf,
    code_path: Option<PathBuf>,
    locales: Vec<String>,
    suggest_from: Vec<String>,
    i18n_keys: FastHashSet<String>,
    i18n_keys_prefix: FastHashSet<String>,
    exclude_dirs: FastHashSet<String>,
    ignore_attributes: FastHashSet<String>,
    ignore_kwargs: FastHashSet<String>,
    default_ftl_file: PathBuf,
    cache: bool,
    cache_path: Option<PathBuf>,
    clear_cache: bool,
}

fn run_check(expanded_checks: ExpandedChecks, config: CheckRunConfig) -> Result<CheckResult> {
    run_check_with_extractor(expanded_checks, config, extract_check_code)
}

fn run_check_with_extractor<F>(
    expanded_checks: ExpandedChecks,
    config: CheckRunConfig,
    mut extract_code: F,
) -> Result<CheckResult>
where
    F: FnMut(CheckCodeConfig) -> Result<ExtractedCode>,
{
    let mut result = CheckResult {
        checked_kinds: Vec::new(),
        diagnostics: Vec::new(),
    };
    let mut extracted_code = None;
    let mut locale_cache = None;
    let mut extraction_diagnostics_added = false;

    for check in expanded_checks.checks {
        match check {
            CheckKind::All => unreachable!("check expansion removes `all`"),
            CheckKind::Kwargs => {
                let extracted =
                    ensure_extracted_code(&mut extracted_code, &config, &mut extract_code)?;
                let cache = ensure_locale_cache(&mut locale_cache, &config, &[])?;
                extend_check_result(
                    &mut result,
                    CheckResult::from(check_kwargs_with_cache(cache, extracted)?),
                );
                add_extraction_diagnostics_once(
                    &mut result,
                    extracted,
                    &mut extraction_diagnostics_added,
                );
            }
            CheckKind::Missing => {
                let extracted =
                    ensure_extracted_code(&mut extracted_code, &config, &mut extract_code)?;
                let cache = ensure_locale_cache(&mut locale_cache, &config, &[])?;
                extend_check_result(
                    &mut result,
                    CheckResult::from(check_missing_with_cache(cache, extracted)?),
                );
                add_extraction_diagnostics_once(
                    &mut result,
                    extracted,
                    &mut extraction_diagnostics_added,
                );
            }
            CheckKind::References => {
                let cache = ensure_locale_cache(&mut locale_cache, &config, &[])?;
                extend_check_result(
                    &mut result,
                    CheckResult::from(check_references_with_cache(cache)?),
                );
            }
            CheckKind::Stale => {
                let extracted =
                    ensure_extracted_code(&mut extracted_code, &config, &mut extract_code)?;
                let cache = ensure_locale_cache(&mut locale_cache, &config, &[])?;
                extend_check_result(
                    &mut result,
                    CheckResult::from(check_stale_with_cache(cache, extracted)?),
                );
                add_extraction_diagnostics_once(
                    &mut result,
                    extracted,
                    &mut extraction_diagnostics_added,
                );
            }
            CheckKind::Syntax => {
                let cache = ensure_locale_cache(&mut locale_cache, &config, &[])?;
                extend_check_result(
                    &mut result,
                    CheckResult::from(check_syntax_with_cache(cache)?),
                );
                if has_fatal_syntax_diagnostics(&result) {
                    break;
                }
            }
            CheckKind::Untranslated => {
                let cache = ensure_locale_cache(&mut locale_cache, &config, &config.suggest_from)?;
                extend_check_result(
                    &mut result,
                    CheckResult::from(check_untranslated_with_cache(cache, &config.suggest_from)?),
                );
            }
        }
    }

    dedup_diagnostics(&mut result);

    Ok(result)
}

fn ensure_locale_cache<'a>(
    locale_cache: &'a mut Option<CheckLocaleCache>,
    config: &CheckRunConfig,
    extra_locales: &[String],
) -> Result<&'a CheckLocaleCache> {
    if locale_cache.is_none() {
        *locale_cache = Some(CheckLocaleCache::load(
            &config.locales_path,
            &config.locales,
            extra_locales,
        )?);
    } else if !extra_locales.is_empty()
        && let Some(cache) = locale_cache.as_mut()
    {
        cache.load_extra_locales(extra_locales)?;
    }

    Ok(locale_cache.as_ref().expect("locale cache is initialized"))
}

fn ensure_extracted_code<'a, F>(
    extracted_code: &'a mut Option<ExtractedCode>,
    config: &CheckRunConfig,
    extract_code: &mut F,
) -> Result<&'a ExtractedCode>
where
    F: FnMut(CheckCodeConfig) -> Result<ExtractedCode>,
{
    if extracted_code.is_none() {
        validate_check_locales(&config.locales_path, &config.locales)?;
        let code_path = config.code_path.clone().context(
            "Missing code path. Pass --code-path or set tool.ftl-extract.check.code-path",
        )?;
        *extracted_code = Some(extract_code(CheckCodeConfig {
            code_path,
            i18n_keys: config.i18n_keys.clone(),
            i18n_keys_prefix: config.i18n_keys_prefix.clone(),
            exclude_dirs: config.exclude_dirs.clone(),
            ignore_attributes: config.ignore_attributes.clone(),
            ignore_kwargs: config.ignore_kwargs.clone(),
            default_ftl_file: config.default_ftl_file.clone(),
            cache: config.cache,
            cache_path: config.cache_path.clone(),
            clear_cache: config.clear_cache,
        })?);
    }

    Ok(extracted_code
        .as_ref()
        .expect("extracted code is initialized"))
}

fn add_extraction_diagnostics_once(
    result: &mut CheckResult,
    extracted: &ExtractedCode,
    added: &mut bool,
) {
    if *added {
        return;
    }

    result
        .diagnostics
        .extend(
            code_extraction_errors(extracted)
                .into_iter()
                .map(|item| Diagnostic {
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
                }),
        );
    *added = true;
}

fn extend_check_result(target: &mut CheckResult, source: CheckResult) {
    target.checked_kinds.extend(source.checked_kinds);
    target.diagnostics.extend(source.diagnostics);
}

#[derive(Debug, Clone)]
struct ExpandedChecks {
    checks: Vec<CheckKind>,
}

fn expand_check_kinds(checks: Vec<CheckKind>) -> ExpandedChecks {
    let defaults = vec![
        CheckKind::Syntax,
        CheckKind::References,
        CheckKind::Untranslated,
        CheckKind::Missing,
        CheckKind::Stale,
        CheckKind::Kwargs,
    ];

    let is_default_or_all = checks.is_empty() || checks.contains(&CheckKind::All);
    let checks = if is_default_or_all {
        defaults
    } else {
        normalize_check_order(checks)
    };

    let mut expanded = Vec::new();
    for check in checks {
        if !expanded.contains(&check) {
            expanded.push(check);
        }
    }
    ExpandedChecks { checks: expanded }
}

fn normalize_check_order(checks: Vec<CheckKind>) -> Vec<CheckKind> {
    if !checks.contains(&CheckKind::Syntax) {
        return checks;
    }

    let mut normalized = vec![CheckKind::Syntax];
    normalized.extend(
        checks
            .into_iter()
            .filter(|check| *check != CheckKind::Syntax),
    );
    normalized
}

fn has_fatal_syntax_diagnostics(result: &CheckResult) -> bool {
    result.diagnostics.iter().any(|diagnostic| {
        diagnostic.kind == DiagnosticKind::Syntax && diagnostic.severity == Severity::Error
    })
}

fn dedup_diagnostics(result: &mut CheckResult) {
    let mut seen = FastHashSet::default();
    result.diagnostics.retain(|diagnostic| {
        diagnostic.kind != DiagnosticKind::Extraction || seen.insert(diagnostic_key(diagnostic))
    });
}

fn diagnostic_key(diagnostic: &Diagnostic) -> String {
    format!(
        "{}|{}|{}|{}|{}|{}|{}|{}|{}",
        diagnostic.kind.as_str(),
        diagnostic.severity.as_str(),
        diagnostic.locale.as_deref().unwrap_or(""),
        diagnostic.key.as_deref().unwrap_or(""),
        diagnostic.message,
        location_key(diagnostic.ftl_location.as_ref()),
        location_key(diagnostic.code_location.as_ref()),
        diagnostic.missing_kwargs.join(","),
        diagnostic.unused_kwargs.join(","),
    )
}

fn location_key(location: Option<&check::SourceLocation>) -> String {
    let Some(location) = location else {
        return String::new();
    };

    let line = location.line.map_or(String::new(), |line| line.to_string());
    let column = location
        .column
        .map_or(String::new(), |column| column.to_string());

    format!("{}:{line}:{column}", location.path.display())
}

impl From<FailSeverity> for Severity {
    fn from(value: FailSeverity) -> Self {
        match value {
            FailSeverity::Error => Self::Error,
            FailSeverity::Warn => Self::Warn,
        }
    }
}

fn normalize_output_path(path: PathBuf, format: &CheckOutputFormat) -> PathBuf {
    if path.extension().is_some() {
        return path;
    }

    let suffix = match format {
        CheckOutputFormat::Terminal => "txt",
        CheckOutputFormat::Json => "json",
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

fn cli_or_config_enum_vec<T>(
    cli: Vec<T>,
    config: Option<Vec<String>>,
    field: &str,
    default: Vec<T>,
) -> Result<Vec<T>>
where
    T: ValueEnum,
{
    if !cli.is_empty() {
        return Ok(cli);
    }

    let Some(config) = config else {
        return Ok(default);
    };

    config
        .into_iter()
        .map(|value| {
            T::from_str(&value, true).map_err(|_| {
                let values = T::value_variants()
                    .iter()
                    .filter_map(|variant| variant.to_possible_value())
                    .map(|value| value.get_name().to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                anyhow::anyhow!("Invalid `{field}` value `{value}`. Expected one of: {values}")
            })
        })
        .collect()
}

fn exit_config_error(error: anyhow::Error) -> ! {
    error!(target: "cli", "Configuration error: {}", error);
    std::process::exit(2);
}

#[cfg(test)]
mod tests {
    use super::*;
    use extractor::ftl::diagnostics::{
        CodeLocation, ExtractionDiagnostic, ExtractionDiagnosticKind,
    };
    use tempfile::TempDir;

    #[test]
    fn check_all_runs_syntax_first() {
        let expanded = expand_check_kinds(vec![CheckKind::All]);

        assert_eq!(expanded.checks.first(), Some(&CheckKind::Syntax));
        assert_eq!(
            expanded.checks,
            vec![
                CheckKind::Syntax,
                CheckKind::References,
                CheckKind::Untranslated,
                CheckKind::Missing,
                CheckKind::Stale,
                CheckKind::Kwargs,
            ]
        );
    }

    #[test]
    fn check_custom_list_is_not_default_or_all() {
        let expanded = expand_check_kinds(vec![CheckKind::Missing, CheckKind::Syntax]);

        assert_eq!(expanded.checks, vec![CheckKind::Syntax, CheckKind::Missing]);
    }

    #[test]
    fn custom_checks_move_syntax_first() {
        let expanded = expand_check_kinds(vec![
            CheckKind::Kwargs,
            CheckKind::Missing,
            CheckKind::References,
            CheckKind::Stale,
            CheckKind::Syntax,
        ]);

        assert_eq!(
            expanded.checks,
            vec![
                CheckKind::Syntax,
                CheckKind::Kwargs,
                CheckKind::Missing,
                CheckKind::References,
                CheckKind::Stale,
            ]
        );
    }

    #[test]
    fn custom_checks_without_syntax_keep_order() {
        let expanded = expand_check_kinds(vec![CheckKind::Missing, CheckKind::Kwargs]);

        assert_eq!(expanded.checks, vec![CheckKind::Missing, CheckKind::Kwargs]);
    }

    #[test]
    fn combined_code_aware_checks_extract_once() {
        let temp = TempDir::new().unwrap();
        let locales_path = temp.path().join("locales");
        let code_path = temp.path().join("code");
        std::fs::create_dir_all(locales_path.join("uk")).unwrap();
        std::fs::create_dir_all(&code_path).unwrap();
        std::fs::write(locales_path.join("uk").join("_default.ftl"), "").unwrap();

        let expanded_checks = expand_check_kinds(vec![
            CheckKind::Missing,
            CheckKind::Stale,
            CheckKind::Kwargs,
        ]);
        let config = CheckRunConfig {
            locales_path,
            code_path: Some(code_path.clone()),
            locales: vec!["uk".to_string()],
            suggest_from: Vec::new(),
            i18n_keys: FastHashSet::default(),
            i18n_keys_prefix: FastHashSet::default(),
            exclude_dirs: FastHashSet::default(),
            ignore_attributes: FastHashSet::default(),
            ignore_kwargs: FastHashSet::default(),
            default_ftl_file: PathBuf::from(DEFAULT_FTL_FILENAME),
            cache: false,
            cache_path: None,
            clear_cache: false,
        };

        let mut extraction_calls = 0;
        let result = run_check_with_extractor(expanded_checks, config, |_| {
            extraction_calls += 1;
            Ok(ExtractedCode {
                keys: Vec::new(),
                diagnostics: vec![ExtractionDiagnostic {
                    kind: ExtractionDiagnosticKind::KeyPathConflict,
                    key: "hello".to_string(),
                    message: "Fluent key hello has different paths".to_string(),
                    locations: vec![CodeLocation {
                        path: code_path.join("app.py"),
                        line: 1,
                        column: 1,
                    }],
                }],
                py_files_count: 1,
            })
        })
        .unwrap();

        assert_eq!(extraction_calls, 1);
        assert_eq!(
            result
                .diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.kind == DiagnosticKind::Extraction)
                .count(),
            1
        );
    }
}
