use crate::args::{CheckKind, FailSeverity};
use anyhow::{Context, Result};
use check::{
    CheckCodeConfig, CheckLocaleCache, CheckResult, Diagnostic, DiagnosticKind, Severity,
    check_kwargs_with_cache, check_missing_with_cache, check_references_with_cache,
    check_stale_with_cache, check_syntax_with_cache, check_untranslated_with_cache,
    code_extraction_errors, extract_check_code, validate_check_locales,
};
use extractor::ftl::diagnostics::ExtractedCode;
use extractor::ftl::utils::FastHashSet;
use std::path::PathBuf;

#[derive(Clone)]
pub(crate) struct CheckRunConfig {
    pub(crate) locales_path: PathBuf,
    pub(crate) code_path: Option<PathBuf>,
    pub(crate) locales: Vec<String>,
    pub(crate) suggest_from: Vec<String>,
    pub(crate) i18n_keys: FastHashSet<String>,
    pub(crate) i18n_keys_prefix: FastHashSet<String>,
    pub(crate) exclude_dirs: FastHashSet<String>,
    pub(crate) ignore_attributes: FastHashSet<String>,
    pub(crate) ignore_kwargs: FastHashSet<String>,
    pub(crate) default_ftl_file: PathBuf,
    pub(crate) cache: bool,
    pub(crate) cache_path: Option<PathBuf>,
    pub(crate) clear_cache: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct ExpandedChecks {
    pub(crate) checks: Vec<CheckKind>,
}

pub(crate) fn run_check(
    expanded_checks: ExpandedChecks,
    config: CheckRunConfig,
) -> Result<CheckResult> {
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

pub(crate) fn expand_check_kinds(checks: Vec<CheckKind>) -> ExpandedChecks {
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

#[cfg(test)]
mod tests {
    use super::*;
    use extractor::ftl::consts::DEFAULT_FTL_FILENAME;
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
