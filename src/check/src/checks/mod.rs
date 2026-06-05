mod kwargs;
mod missing;
mod references;
mod stale;
mod syntax;
mod untranslated;

pub use kwargs::check_kwargs;
pub use kwargs::check_kwargs_with_cache;
pub use kwargs::check_kwargs_with_extracted;
pub use missing::check_missing;
pub use missing::check_missing_with_cache;
pub use missing::check_missing_with_extracted;
pub use references::check_references;
pub use references::check_references_with_cache;
pub use stale::check_stale;
pub use stale::check_stale_with_cache;
pub use stale::check_stale_with_extracted;
pub use syntax::check_syntax;
pub use syntax::check_syntax_with_cache;
pub use untranslated::check_untranslated;
pub use untranslated::check_untranslated_with_cache;

use crate::parser::{CheckLocaleCache, discover_locales};
use crate::types::{
    CheckCodeAwareCheckConfig, CheckCodeAwareConfig, CheckCodeConfig, CheckKwargsResult,
    CheckLocaleConfig, CheckMissingResult, CheckStaleResult, CodeExtractionError,
};
use anyhow::{Result, bail};
use extractor::ftl::code_extractor::{build_exclude_matcher, extract_code_with_diagnostics_cached};
use extractor::ftl::diagnostics::ExtractedCode;
use std::path::{Path, PathBuf};

pub fn extract_check_code(config: CheckCodeConfig) -> Result<ExtractedCode> {
    let exclude_matcher = build_exclude_matcher(&config.code_path, &config.exclude_dirs)?;
    Ok(extract_code_with_diagnostics_cached(
        &config.code_path,
        config.i18n_keys,
        config.i18n_keys_prefix,
        &exclude_matcher,
        config.ignore_attributes,
        config.ignore_kwargs,
        &config.default_ftl_file,
        config.cache,
        config.cache_path.as_deref(),
        config.clear_cache,
    ))
}

pub fn code_extraction_errors(extracted: &ExtractedCode) -> Vec<CodeExtractionError> {
    extracted
        .diagnostics
        .iter()
        .cloned()
        .map(Into::into)
        .collect()
}

pub(super) fn run_locale_check<R>(
    config: CheckLocaleConfig,
    check: impl FnOnce(&CheckLocaleCache) -> Result<R>,
) -> Result<R> {
    let cache = CheckLocaleCache::load(&config.locales_path, &config.locales, &[])?;
    check(&cache)
}

pub(super) fn run_code_aware_check<C, R>(
    config: C,
    check: impl FnOnce(&CheckLocaleCache, &ExtractedCode) -> Result<R>,
) -> Result<R>
where
    C: IntoCodeAwareCheckInputs,
    R: CheckResultWithExtractionErrors,
{
    let inputs = config.into_code_aware_check_inputs();
    let cache = CheckLocaleCache::load(&inputs.locales_path, &inputs.locales, &[])?;
    let extracted = extract_check_code(inputs.code)?;

    let mut result = check(&cache, &extracted)?;
    result.set_extraction_errors(code_extraction_errors(&extracted));
    Ok(result)
}

pub(super) fn run_code_aware_check_with_extracted<R>(
    config: CheckCodeAwareConfig,
    extracted: &ExtractedCode,
    check: impl FnOnce(&CheckLocaleCache, &ExtractedCode) -> Result<R>,
) -> Result<R> {
    let cache = CheckLocaleCache::load(&config.locales_path, &config.locales, &[])?;
    check(&cache, extracted)
}

pub fn validate_check_locales(locales_path: &Path, locales: &[String]) -> Result<()> {
    resolve_locales(locales_path, locales).map(|_| ())
}

pub(super) fn resolve_locales(locales_path: &Path, locales: &[String]) -> Result<Vec<String>> {
    let available_locales = discover_locales(locales_path)?;
    resolve_locales_with_available(locales_path, &available_locales, locales)
}

pub(super) fn resolve_locales_with_available(
    locales_path: &Path,
    available_locales: &[String],
    locales: &[String],
) -> Result<Vec<String>> {
    if locales.is_empty() {
        return Ok(available_locales.to_vec());
    }

    validate_locales(locales_path, available_locales, locales)?;
    Ok(locales.to_vec())
}

pub(super) fn validate_locales(
    locales_path: &Path,
    available_locales: &[String],
    locales: &[String],
) -> Result<()> {
    for locale in locales {
        if !available_locales.iter().any(|existing| existing == locale) {
            bail!(
                "Locale `{}` does not exist in `{}`",
                locale,
                locales_path.display()
            );
        }
    }

    Ok(())
}

pub(super) struct CodeAwareCheckInputs {
    locales_path: PathBuf,
    locales: Vec<String>,
    code: CheckCodeConfig,
}

pub(super) trait IntoCodeAwareCheckInputs {
    fn into_code_aware_check_inputs(self) -> CodeAwareCheckInputs;
}

pub(super) trait CheckResultWithExtractionErrors {
    fn set_extraction_errors(&mut self, extraction_errors: Vec<CodeExtractionError>);
}

impl IntoCodeAwareCheckInputs for CheckCodeAwareCheckConfig {
    fn into_code_aware_check_inputs(self) -> CodeAwareCheckInputs {
        CodeAwareCheckInputs {
            locales_path: self.locales_path,
            locales: self.locales,
            code: CheckCodeConfig {
                code_path: self.code_path,
                i18n_keys: self.i18n_keys,
                i18n_keys_prefix: self.i18n_keys_prefix,
                exclude_dirs: self.exclude_dirs,
                ignore_attributes: self.ignore_attributes,
                ignore_kwargs: self.ignore_kwargs,
                default_ftl_file: self.default_ftl_file,
                cache: self.cache,
                cache_path: self.cache_path,
                clear_cache: self.clear_cache,
            },
        }
    }
}

macro_rules! impl_check_result_with_extraction_errors {
    ($result:ty) => {
        impl CheckResultWithExtractionErrors for $result {
            fn set_extraction_errors(&mut self, extraction_errors: Vec<CodeExtractionError>) {
                self.extraction_errors = extraction_errors;
            }
        }
    };
}

impl_check_result_with_extraction_errors!(CheckKwargsResult);
impl_check_result_with_extraction_errors!(CheckMissingResult);
impl_check_result_with_extraction_errors!(CheckStaleResult);
