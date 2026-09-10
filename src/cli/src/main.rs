mod args;
mod check_runner;
mod config;
mod options;

use crate::args::{CheckReportFormat, Cli, Commands, ConfigCommands, FailSeverity};
use crate::check_runner::{CheckRunConfig, expand_check_kinds, run_check};
use crate::config::{load_pyproject_config, render_config_sample};
use crate::options::{
    cli_or_config_enum, cli_or_config_enum_vec, cli_or_config_path, cli_or_config_vec,
    exit_config_error, normalize_output_path, resolve_required_path, write_output_file,
};
use check::{has_failing_diagnostics, render_check_json, render_check_terminal};
use clap::Parser;
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

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

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
            locales_path,
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
            allow_parse_errors,
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
            let locales_path = match resolve_required_path(
                locales_path,
                pyproject.locales_path,
                base_dir,
                "Missing locales path. Pass locales path as an argument or set tool.ftl-extract.extract.locales-path",
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
            info!(target: "cli", "Locales path: {}", locales_path.display());

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
                locales_path,
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
                allow_parse_errors: allow_parse_errors
                    || pyproject.allow_parse_errors.unwrap_or(false),
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
            locales_path,
            stub_path,
            export_tree,
        }) => {
            let config_source = project_config.as_ref();
            let pyproject = config_source
                .and_then(|loaded| loaded.config.stub.clone())
                .unwrap_or_default();
            let base_dir = config_source
                .map(|loaded| loaded.base_dir.as_path())
                .unwrap_or_else(|| Path::new("."));
            let locales_path = match resolve_required_path(
                locales_path,
                pyproject.locales_path,
                base_dir,
                "Missing locales path. Pass it as an argument or set tool.ftl-extract.stub.locales-path",
            ) {
                Ok(path) => path,
                Err(e) => exit_config_error(e),
            };
            let stub_path = match resolve_required_path(
                stub_path,
                pyproject.stub_path,
                base_dir,
                "Missing stub path. Pass it as an argument or set tool.ftl-extract.stub.stub-path",
            ) {
                Ok(path) => path,
                Err(e) => exit_config_error(e),
            };

            info!(target: "cli", "Locales path: {}", locales_path.display());
            info!(target: "cli", "Stub path: {}", stub_path.display());

            let config = StubConfig {
                locales_path,
                stub_path,
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
            report_path,
            report_format,
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
            let report_format =
                match cli_or_config_enum(report_format, pyproject.report_format, "report-format") {
                    Ok(report_format) => report_format,
                    Err(e) => exit_config_error(e),
                };
            let report_format = report_format.unwrap_or(CheckReportFormat::Json);
            let report = cli_or_config_path(report_path, pyproject.report_path, base_dir);
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

                    if let Some(report_path) = report {
                        let report_path = normalize_output_path(report_path, &report_format);
                        let report_content = match report_format {
                            CheckReportFormat::Terminal => render_check_terminal(&result),
                            CheckReportFormat::Json => render_check_json(&result),
                        };

                        if let Err(e) = write_output_file(&report_path, report_content) {
                            error!(
                                target: "cli",
                                "Failed to write report file `{}`: {}",
                                report_path.display(),
                                e
                            );
                            std::process::exit(1);
                        }

                        info!(target: "cli", "Saved report to {}", report_path.display());
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
