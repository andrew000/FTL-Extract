mod locale;

#[cfg(test)]
mod tests;

use crate::checks::{run_code_aware_check, run_code_aware_check_with_extracted};
use crate::parser::CheckLocaleCache;
use crate::types::{CheckCodeAwareConfig, CheckKwargsConfig, CheckKwargsResult, KwargsMismatch};
use anyhow::Result;
use common::FastHashSet;
use extractor::ftl::diagnostics::{ExtractedCode, ExtractedFluentKey};
use locale::read_locale_messages_with_ast;
use log::debug;

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
    // A call with `**kwargs` can pass any variable, so such a key cannot be checked. It is
    // skipped for every locale and mentioned once in verbose mode.
    let mut unverifiable: Vec<&ExtractedFluentKey> = extracted
        .keys
        .iter()
        .filter(|code_key| code_key.kwargs_unknown.is_some())
        .collect();
    unverifiable.sort_by(|a, b| a.key.cmp(&b.key));
    for code_key in &unverifiable {
        if let Some(location) = &code_key.kwargs_unknown {
            debug!(
                target: "check::kwargs",
                "key \"{}\" is called with **kwargs at {location}; its variables cannot be verified",
                code_key.key
            );
        }
    }

    let mut mismatches = Vec::new();
    for locale in cache.checked_locales() {
        let locale_messages = read_locale_messages_with_ast(cache, locale);

        for code_key in &extracted.keys {
            if code_key.kwargs_unknown.is_some() {
                continue;
            }
            let Some(locale_message) = locale_messages
                .by_expected_path
                .get(&(code_key.key.clone(), code_key.ftl_path.clone()))
            else {
                continue;
            };

            // The message opted out of this check with `# ftl-extract: ignore kwargs`.
            if locale_message.ignores_kwargs {
                debug!(
                    target: "check::kwargs",
                    "key \"{}\" is skipped in {locale}: marker ignores kwargs",
                    code_key.key
                );
                continue;
            }

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
