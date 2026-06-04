use crate::checks::run_locale_check;
use crate::parser::CheckLocaleCache;
use crate::types::{CheckSyntaxConfig, CheckSyntaxResult, SyntaxError};
use anyhow::Result;

pub fn check_syntax(config: CheckSyntaxConfig) -> Result<CheckSyntaxResult> {
    run_locale_check(config, check_syntax_with_cache)
}

pub fn check_syntax_with_cache(cache: &CheckLocaleCache) -> Result<CheckSyntaxResult> {
    let mut errors = Vec::new();
    for locale in cache.checked_locales() {
        for file in cache.files(locale) {
            for error in &file.syntax_errors {
                errors.push(SyntaxError {
                    locale: locale.clone(),
                    file_path: file.relative_to_locales.clone(),
                    line: error.line,
                    column: error.column,
                    message: error.message.clone(),
                });
            }
        }
    }

    errors.sort_by(|a, b| {
        a.locale
            .cmp(&b.locale)
            .then_with(|| a.file_path.cmp(&b.file_path))
            .then_with(|| a.line.cmp(&b.line))
            .then_with(|| a.column.cmp(&b.column))
    });

    Ok(CheckSyntaxResult {
        checked_locales: cache.checked_locales().to_vec(),
        errors,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_check_syntax_reports_invalid_ftl() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let locales = temp_dir.path().join("locales");
        fs::create_dir_all(locales.join("en"))?;

        fs::write(
            locales.join("en").join("_default.ftl"),
            "valid = Valid\nbroken = {\n",
        )?;

        let result = check_syntax(CheckSyntaxConfig {
            locales_path: locales,
            locales: vec!["en".to_string()],
        })?;

        assert!(!result.errors.is_empty());
        let item = &result.errors[0];
        assert_eq!(item.locale, "en");
        assert_eq!(item.file_path, std::path::PathBuf::from("en/_default.ftl"));
        assert!(item.line.is_some());
        assert!(!item.message.is_empty());
        Ok(())
    }

    #[test]
    fn test_check_syntax_ignores_valid_ftl() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let locales = temp_dir.path().join("locales");
        fs::create_dir_all(locales.join("en"))?;

        fs::write(locales.join("en").join("_default.ftl"), "valid = Valid\n")?;

        let result = check_syntax(CheckSyntaxConfig {
            locales_path: locales,
            locales: vec!["en".to_string()],
        })?;

        assert!(result.errors.is_empty());
        Ok(())
    }
}
