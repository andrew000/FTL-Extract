use crate::checks::resolve_locales;
use crate::parser::{ftl_files_for_locale, parse_ftl_syntax_errors};
use crate::types::{CheckSyntaxConfig, CheckSyntaxResult, SyntaxError};
use anyhow::Result;

pub fn check_syntax(config: CheckSyntaxConfig) -> Result<CheckSyntaxResult> {
    let locales = resolve_locales(&config.locales_path, &config.locales)?;

    let mut errors = Vec::new();
    for locale in &locales {
        for file_path in ftl_files_for_locale(&config.locales_path, locale)? {
            for error in parse_ftl_syntax_errors(&file_path)? {
                errors.push(SyntaxError {
                    locale: locale.clone(),
                    file_path: file_path
                        .strip_prefix(&config.locales_path)
                        .unwrap_or(&file_path)
                        .to_path_buf(),
                    line: error.line,
                    column: error.column,
                    message: error.message,
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
        checked_locales: locales,
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
