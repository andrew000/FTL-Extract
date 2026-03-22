use crate::parser::{discover_locales, read_locale_messages};
use crate::types::{
    CheckUntranslatedConfig, CheckUntranslatedResult, TranslationSuggestion, UntranslatedKey,
};
use anyhow::{Result, bail};

pub fn check_untranslated(config: CheckUntranslatedConfig) -> Result<CheckUntranslatedResult> {
    let available_locales = discover_locales(&config.locales_path)?;

    let locales = if config.locales.is_empty() {
        available_locales.clone()
    } else {
        config.locales
    };

    for locale in &locales {
        if !available_locales.iter().any(|existing| existing == locale) {
            bail!(
                "Locale `{}` does not exist in `{}`",
                locale,
                config.locales_path.display()
            );
        }
    }

    for locale in &config.suggest_from {
        if !available_locales.iter().any(|existing| existing == locale) {
            bail!(
                "Suggest locale `{}` does not exist in `{}`",
                locale,
                config.locales_path.display()
            );
        }
    }

    let mut checked_entries = Vec::new();
    for locale in &locales {
        checked_entries.extend(read_locale_messages(&config.locales_path, locale)?);
    }

    let mut suggestion_only_entries = Vec::new();
    for locale in &config.suggest_from {
        if !locales.iter().any(|checked| checked == locale) {
            suggestion_only_entries.extend(read_locale_messages(&config.locales_path, locale)?);
        }
    }

    let checked_messages = checked_entries
        .iter()
        .filter_map(|entry| entry.value.as_ref().map(|value| (entry, value)))
        .filter(|(_, value)| !value.is_empty())
        .collect::<Vec<_>>();

    let all_messages = checked_entries
        .iter()
        .chain(suggestion_only_entries.iter())
        .filter_map(|entry| entry.value.as_ref().map(|value| (entry, value)))
        .filter(|(_, value)| !value.is_empty())
        .collect::<Vec<_>>();

    let mut untranslated = checked_messages
        .iter()
        .filter(|(entry, value)| {
            !entry.ignore_untranslated && is_placeholder_translation(&entry.key, value)
        })
        .map(|(entry, value)| UntranslatedKey {
            locale: entry.locale.clone(),
            file_path: entry
                .file_path
                .strip_prefix(&config.locales_path)
                .unwrap_or(&entry.file_path)
                .to_path_buf(),
            key: entry.key.clone(),
            value: (*value).clone(),
            line: entry.line,
            suggestions: Vec::new(),
        })
        .collect::<Vec<_>>();

    if !config.suggest_from.is_empty() {
        for item in &mut untranslated {
            item.suggestions = config
                .suggest_from
                .iter()
                .filter_map(|locale| {
                    all_messages
                        .iter()
                        .find(|(entry, value)| {
                            entry.locale == *locale
                                && entry.key == item.key
                                && !is_placeholder_translation(&entry.key, value)
                        })
                        .map(|(_, value)| TranslationSuggestion {
                            locale: locale.clone(),
                            value: (*value).clone(),
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

    let fully_translated_locales = locales
        .iter()
        .filter(|locale| untranslated.iter().all(|item| &item.locale != *locale))
        .cloned()
        .collect::<Vec<_>>();

    Ok(CheckUntranslatedResult {
        checked_locales: locales,
        fully_translated_locales,
        untranslated,
    })
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
            suggest_from: vec!["ru".to_string()],
        });

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Suggest locale `ru` does not exist")
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
}
