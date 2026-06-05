mod definitions;
mod locale;
mod traversal;

#[cfg(test)]
mod tests;

use crate::checks::run_locale_check;
use crate::parser::CheckLocaleCache;
use crate::types::{CheckReferencesConfig, CheckReferencesResult};
use anyhow::Result;
use definitions::collect_definitions;
use locale::read_locale_resources;
use traversal::collect_missing_references;

pub fn check_references(config: CheckReferencesConfig) -> Result<CheckReferencesResult> {
    run_locale_check(config, check_references_with_cache)
}

pub fn check_references_with_cache(cache: &CheckLocaleCache) -> Result<CheckReferencesResult> {
    let mut missing_references = Vec::new();
    for locale in cache.checked_locales() {
        let locale_resources = read_locale_resources(cache, locale);
        let definitions = collect_definitions(&locale_resources);

        for resource in &locale_resources {
            collect_missing_references(locale, resource, &definitions, &mut missing_references);
        }
    }

    missing_references.sort_by(|a, b| {
        a.locale
            .cmp(&b.locale)
            .then_with(|| a.file_path.cmp(&b.file_path))
            .then_with(|| a.line.cmp(&b.line))
            .then_with(|| a.reference.cmp(&b.reference))
    });

    Ok(CheckReferencesResult {
        checked_locales: cache.checked_locales().to_vec(),
        missing_references,
    })
}
