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

use crate::parser::discover_locales;
use crate::types::{CheckCodeConfig, CodeExtractionError};
use anyhow::{Result, bail};
use extractor::ftl::code_extractor::extract_code_with_diagnostics_cached;
use extractor::ftl::diagnostics::ExtractedCode;
use globset::{Glob, GlobSetBuilder};
use std::path::Path;

pub fn extract_check_code(config: CheckCodeConfig) -> Result<ExtractedCode> {
    let ignore_set = build_ignore_set(&config.exclude_dirs)?;
    Ok(extract_code_with_diagnostics_cached(
        &config.code_path,
        config.i18n_keys,
        config.i18n_keys_prefix,
        &ignore_set,
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

pub(super) fn build_ignore_set(
    exclude_dirs: &extractor::ftl::utils::FastHashSet<String>,
) -> Result<globset::GlobSet> {
    let mut builder = GlobSetBuilder::new();
    for exclude in exclude_dirs {
        builder.add(Glob::new(exclude.as_str())?);
    }
    Ok(builder.build()?)
}
