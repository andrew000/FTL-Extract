use crate::checks::resolve_locales_with_available;
use crate::parser::{CheckLocaleCache, discover_locales};
use crate::types::{
    CheckUntranslatedConfig, CheckUntranslatedResult, TranslationSuggestion, UntranslatedKey,
};
use anyhow::{Result, bail};
use common::FastHashMap;

pub fn check_untranslated(config: CheckUntranslatedConfig) -> Result<CheckUntranslatedResult> {
    let available_locales = discover_locales(&config.locales_path)?;

    resolve_locales_with_available(&config.locales_path, &available_locales, &config.locales)?;

    for locale in &config.suggest_from {
        if !available_locales.iter().any(|existing| existing == locale) {
            bail!(
                "Suggest locale `{}` does not exist in `{}`",
                locale,
                config.locales_path.display()
            );
        }
    }

    let cache =
        CheckLocaleCache::load(&config.locales_path, &config.locales, &config.suggest_from)?;

    check_untranslated_with_cache(&cache, &config.suggest_from)
}

pub fn check_untranslated_with_cache(
    cache: &CheckLocaleCache,
    suggest_from: &[String],
) -> Result<CheckUntranslatedResult> {
    let mut checked_entries = Vec::new();
    for locale in cache.checked_locales() {
        checked_entries.extend(cache.messages(locale)?);
    }

    let mut suggestion_only_entries = Vec::new();
    for locale in suggest_from {
        if !cache
            .checked_locales()
            .iter()
            .any(|checked| checked == locale)
        {
            suggestion_only_entries.extend(cache.messages(locale)?);
        }
    }

    let checked_messages = checked_entries
        .iter()
        .filter_map(|entry| entry.value.as_ref().map(|value| (entry, value)))
        .filter(|(_, value)| !value.is_empty())
        .collect::<Vec<_>>();

    let suggestion_values = suggestion_values_by_locale_and_key(
        checked_entries.iter().chain(suggestion_only_entries.iter()),
    );

    let mut untranslated = checked_messages
        .iter()
        .filter(|(entry, value)| {
            !entry.ignored.untranslated && is_placeholder_translation(&entry.key, value)
        })
        .map(|(entry, value)| UntranslatedKey {
            locale: entry.locale.clone(),
            file_path: entry
                .file_path
                .strip_prefix(cache.locales_path())
                .unwrap_or(&entry.file_path)
                .to_path_buf(),
            key: entry.key.clone(),
            value: (*value).clone(),
            line: entry.line,
            suggestions: Vec::new(),
        })
        .collect::<Vec<_>>();

    if !suggest_from.is_empty() {
        for item in &mut untranslated {
            item.suggestions = suggest_from
                .iter()
                .filter_map(|locale| {
                    suggestion_values
                        .get(&(locale.clone(), item.key.clone()))
                        .map(|value| TranslationSuggestion {
                            locale: locale.clone(),
                            value: value.clone(),
                        })
                })
                .collect();
        }
    }

    untranslated.sort_by(|a, b| {
        a.locale
            .cmp(&b.locale)
            .then_with(|| a.file_path.cmp(&b.file_path))
            .then_with(|| a.key.cmp(&b.key))
    });

    let fully_translated_locales = cache
        .checked_locales()
        .iter()
        .filter(|locale| untranslated.iter().all(|item| &item.locale != *locale))
        .cloned()
        .collect::<Vec<_>>();

    Ok(CheckUntranslatedResult {
        checked_locales: cache.checked_locales().to_vec(),
        fully_translated_locales,
        untranslated,
    })
}

fn suggestion_values_by_locale_and_key<'a>(
    entries: impl Iterator<Item = &'a crate::types::MessageEntry>,
) -> FastHashMap<(String, String), String> {
    entries
        .filter_map(|entry| {
            let value = entry.value.as_ref()?;
            if value.is_empty() || is_placeholder_translation(&entry.key, value) {
                return None;
            }
            Some(((entry.locale.clone(), entry.key.clone()), value.clone()))
        })
        .collect()
}

fn is_placeholder_translation(key: &str, value: &str) -> bool {
    key == value
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_placeholder_detection() {
        assert!(is_placeholder_translation("hello-world", "hello-world"));
        assert!(!is_placeholder_translation("hello-world", "Hello world"));
    }

    #[test]
    fn test_check_untranslated_defaults_to_all_locales() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let locales = temp_dir.path().join("locales");
        fs::create_dir_all(locales.join("en"))?;
        fs::create_dir_all(locales.join("uk"))?;
        fs::write(
            locales.join("en").join("_default.ftl"),
            "welcome = Welcome\n",
        )?;
        fs::write(
            locales.join("uk").join("_default.ftl"),
            "welcome = welcome\n",
        )?;

        let result = check_untranslated(CheckUntranslatedConfig {
            locales_path: locales,
            locales: vec![],
            suggest_from: vec![],
        })?;

        assert_eq!(
            result.checked_locales,
            vec!["en".to_string(), "uk".to_string()]
        );
        assert_eq!(result.untranslated.len(), 1);
        assert_eq!(result.untranslated[0].locale, "uk");
        Ok(())
    }

    #[test]
    fn test_check_untranslated_with_suggestions() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let locales = temp_dir.path().join("locales");
        fs::create_dir_all(locales.join("en"))?;
        fs::create_dir_all(locales.join("uk"))?;

        fs::write(
            locales.join("en").join("_default.ftl"),
            "welcome = Welcome\nlogout = Logout\n",
        )?;
        fs::write(
            locales.join("uk").join("_default.ftl"),
            "welcome = welcome\nlogout = Вихід\n",
        )?;

        let result = check_untranslated(CheckUntranslatedConfig {
            locales_path: locales,
            locales: vec!["uk".to_string(), "en".to_string()],
            suggest_from: vec!["en".to_string()],
        })?;

        assert_eq!(result.untranslated.len(), 1);
        let item = &result.untranslated[0];
        assert_eq!(item.locale, "uk");
        assert_eq!(item.key, "welcome");
        assert_eq!(item.value, "welcome");
        assert_eq!(item.suggestions.len(), 1);
        assert_eq!(item.suggestions[0].locale, "en");
        assert_eq!(item.suggestions[0].value, "Welcome");
        assert_eq!(result.fully_translated_locales, vec!["en".to_string()]);
        Ok(())
    }

    #[test]
    fn test_ignore_untranslated_marker_skips_placeholder() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let locales = temp_dir.path().join("locales");
        fs::create_dir_all(locales.join("en"))?;

        fs::write(
            locales.join("en").join("_default.ftl"),
            "# ftl-extract: ignore-untranslated\nbalance = balance\nnormal = normal\n",
        )?;

        let result = check_untranslated(CheckUntranslatedConfig {
            locales_path: locales,
            locales: vec!["en".to_string()],
            suggest_from: vec![],
        })?;

        assert_eq!(result.untranslated.len(), 1);
        assert_eq!(result.untranslated[0].key, "normal");
        Ok(())
    }

    #[test]
    fn test_missing_suggest_locale_returns_error() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let locales = temp_dir.path().join("locales");
        fs::create_dir_all(locales.join("en"))?;

        fs::write(
            locales.join("en").join("_default.ftl"),
            "welcome = Welcome\n",
        )?;

        let result = check_untranslated(CheckUntranslatedConfig {
            locales_path: locales,
            locales: vec!["en".to_string()],
            suggest_from: vec!["pl".to_string()],
        });

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Suggest locale `pl` does not exist")
        );
        Ok(())
    }

    #[test]
    fn test_suggest_locale_can_be_outside_checked_locales() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let locales = temp_dir.path().join("locales");
        fs::create_dir_all(locales.join("en"))?;
        fs::create_dir_all(locales.join("uk"))?;

        fs::write(
            locales.join("en").join("_default.ftl"),
            "welcome = welcome\n",
        )?;
        fs::write(
            locales.join("uk").join("_default.ftl"),
            "welcome = Ласкаво просимо\n",
        )?;

        let result = check_untranslated(CheckUntranslatedConfig {
            locales_path: locales,
            locales: vec!["en".to_string()],
            suggest_from: vec!["uk".to_string()],
        })?;

        assert_eq!(result.untranslated.len(), 1);
        let item = &result.untranslated[0];
        assert_eq!(item.locale, "en");
        assert_eq!(item.key, "welcome");
        assert_eq!(item.suggestions.len(), 1);
        assert_eq!(item.suggestions[0].locale, "uk");
        Ok(())
    }

    #[test]
    fn test_generalised_ignore_marker_skips_placeholder() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let locales = temp_dir.path().join("locales");
        fs::create_dir_all(locales.join("en"))?;

        fs::write(
            locales.join("en").join("_default.ftl"),
            "# ftl-extract: ignore untranslated\nbalance = balance\n# ftl-extract: ignore all\nbrand = brand\n# ftl-extract: ignore stale\nstale-only = stale-only\nnormal = normal\n",
        )?;

        let result = check_untranslated(CheckUntranslatedConfig {
            locales_path: locales,
            locales: vec!["en".to_string()],
            suggest_from: vec![],
        })?;

        let keys = result
            .untranslated
            .iter()
            .map(|item| item.key.as_str())
            .collect::<Vec<_>>();
        assert_eq!(keys, vec!["normal", "stale-only"]);
        Ok(())
    }
}
