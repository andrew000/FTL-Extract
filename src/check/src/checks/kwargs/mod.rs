mod locale;

#[cfg(test)]
mod tests;

use crate::checks::{run_code_aware_check, run_code_aware_check_with_extracted};
use crate::parser::CheckLocaleCache;
use crate::types::{CheckCodeAwareConfig, CheckKwargsConfig, CheckKwargsResult, KwargsMismatch};
use anyhow::Result;
use common::FastHashSet;
use extractor::ftl::diagnostics::ExtractedCode;
use locale::read_locale_messages_with_ast;

pub fn check_kwargs(config: CheckKwargsConfig) -> Result<CheckKwargsResult> {
    run_code_aware_check(config, check_kwargs_with_cache)
}

pub fn check_kwargs_with_extracted(
    config: CheckCodeAwareConfig,
    extracted: &ExtractedCode,
) -> Result<CheckKwargsResult> {
    run_code_aware_check_with_extracted(config, extracted, check_kwargs_with_cache)
}

pub fn check_kwargs_with_cache(
    cache: &CheckLocaleCache,
    extracted: &ExtractedCode,
) -> Result<CheckKwargsResult> {
    let mut mismatches = Vec::new();
    for locale in cache.checked_locales() {
        let locale_messages = read_locale_messages_with_ast(cache, locale);

        for code_key in &extracted.keys {
            let Some(locale_message) = locale_messages
                .by_expected_path
                .get(&(code_key.key.clone(), code_key.ftl_path.clone()))
            else {
                continue;
            };

            let code_kwargs = code_key.kwargs.iter().cloned().collect::<FastHashSet<_>>();
            let ftl_kwargs = locale_messages
                .message_kwargs(&locale_message.key)
                .cloned()
                .unwrap_or_default();

            let mut missing_kwargs = ftl_kwargs
                .difference(&code_kwargs)
                .cloned()
                .collect::<Vec<_>>();
            let mut unused_kwargs = code_kwargs
                .difference(&ftl_kwargs)
                .cloned()
                .collect::<Vec<_>>();
            missing_kwargs.sort();
            unused_kwargs.sort();

            if missing_kwargs.is_empty() && unused_kwargs.is_empty() {
                continue;
            }

            mismatches.push(KwargsMismatch {
                locale: locale.clone(),
                key: code_key.key.clone(),
                file_path: locale_message.path.clone(),
                line: locale_message.line,
                code_location: code_key.code_location.clone().map(Into::into),
                missing_kwargs,
                unused_kwargs,
            });
        }
    }

    mismatches.sort_by(|a, b| {
        a.locale
            .cmp(&b.locale)
            .then_with(|| a.file_path.cmp(&b.file_path))
            .then_with(|| a.line.cmp(&b.line))
            .then_with(|| a.key.cmp(&b.key))
    });

    Ok(CheckKwargsResult {
        checked_locales: cache.checked_locales().to_vec(),
        mismatches,
        extraction_errors: Vec::new(),
    })
}
