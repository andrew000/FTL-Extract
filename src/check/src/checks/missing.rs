use crate::checks::{code_extraction_errors, extract_check_code, resolve_locales};
use crate::parser::read_locale_messages;
use crate::types::{
    CheckCodeAwareConfig, CheckCodeConfig, CheckMissingConfig, CheckMissingResult, MissingKey,
};
use anyhow::Result;
use extractor::ftl::diagnostics::ExtractedCode;
use extractor::ftl::utils::FastHashSet;
use std::path::{Path, PathBuf};

pub fn check_missing(config: CheckMissingConfig) -> Result<CheckMissingResult> {
    let locales = resolve_locales(&config.locales_path, &config.locales)?;
    let code_aware_config = CheckCodeAwareConfig {
        locales_path: config.locales_path,
        locales,
    };
    let extracted = extract_check_code(CheckCodeConfig {
        code_path: config.code_path,
        i18n_keys: config.i18n_keys,
        i18n_keys_prefix: config.i18n_keys_prefix,
        exclude_dirs: config.exclude_dirs,
        ignore_attributes: config.ignore_attributes,
        ignore_kwargs: config.ignore_kwargs,
        default_ftl_file: config.default_ftl_file,
    })?;

    let mut result = check_missing_with_extracted(code_aware_config, &extracted)?;
    result.extraction_errors = code_extraction_errors(&extracted);
    Ok(result)
}

pub fn check_missing_with_extracted(
    config: CheckCodeAwareConfig,
    extracted: &ExtractedCode,
) -> Result<CheckMissingResult> {
    let locales = resolve_locales(&config.locales_path, &config.locales)?;

    let mut missing_keys = Vec::new();
    for locale in &locales {
        let existing = existing_locale_keys(&config.locales_path, locale)?;
        for code_key in &extracted.keys {
            if !existing.contains(&(code_key.key.clone(), code_key.ftl_path.clone())) {
                missing_keys.push(MissingKey {
                    locale: locale.clone(),
                    key: code_key.key.clone(),
                    expected_file_path: PathBuf::from(locale).join(&code_key.ftl_path),
                    code_location: code_key.code_location.clone().map(Into::into),
                });
            }
        }
    }

    Ok(CheckMissingResult {
        checked_locales: locales,
        missing_keys,
        extraction_errors: Vec::new(),
    })
}

fn existing_locale_keys(
    locales_path: &Path,
    locale: &str,
) -> Result<FastHashSet<(String, PathBuf)>> {
    let locale_path = locales_path.join(locale);
    let entries = read_locale_messages(locales_path, locale)?;
    Ok(entries
        .into_iter()
        .filter_map(|entry| {
            let relative_path = entry
                .file_path
                .strip_prefix(&locale_path)
                .ok()?
                .to_path_buf();
            Some((entry.key, relative_path))
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use extractor::ftl::consts::{
        DEFAULT_EXCLUDE_DIRS, DEFAULT_FTL_FILENAME, DEFAULT_I18N_KEYS, DEFAULT_IGNORE_ATTRIBUTES,
        DEFAULT_IGNORE_KWARGS,
    };
    use std::fs;
    use tempfile::TempDir;

    fn write(path: &Path, content: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, content).unwrap();
    }

    fn config(temp: &TempDir, locales: Vec<String>) -> CheckMissingConfig {
        CheckMissingConfig {
            locales_path: temp.path().join("locales"),
            code_path: temp.path().join("code"),
            locales,
            i18n_keys: DEFAULT_I18N_KEYS.clone(),
            i18n_keys_prefix: FastHashSet::default(),
            exclude_dirs: DEFAULT_EXCLUDE_DIRS.clone(),
            ignore_attributes: DEFAULT_IGNORE_ATTRIBUTES.clone(),
            ignore_kwargs: DEFAULT_IGNORE_KWARGS.clone(),
            default_ftl_file: PathBuf::from(DEFAULT_FTL_FILENAME),
        }
    }

    #[test]
    fn test_check_missing_reports_missing_code_key() {
        let temp = TempDir::new().unwrap();
        write(
            &temp.path().join("code/app.py"),
            r#"def handler():
    i18n.get("hello")
"#,
        );
        write(
            &temp.path().join("locales/uk/_default.ftl"),
            "other = Other\n",
        );

        let result = check_missing(config(&temp, vec!["uk".to_string()])).unwrap();

        assert_eq!(result.missing_keys.len(), 1);
        assert_eq!(result.missing_keys[0].locale, "uk");
        assert_eq!(result.missing_keys[0].key, "hello");
        assert_eq!(
            result.missing_keys[0].expected_file_path,
            PathBuf::from("uk").join("_default.ftl")
        );
        assert_eq!(
            result.missing_keys[0].code_location.as_ref().unwrap().line,
            Some(2)
        );
    }

    #[test]
    fn test_check_missing_accepts_existing_key_in_expected_file() {
        let temp = TempDir::new().unwrap();
        write(&temp.path().join("code/app.py"), r#"i18n.get("hello")"#);
        write(
            &temp.path().join("locales/uk/_default.ftl"),
            "hello = Hello\n",
        );

        let result = check_missing(config(&temp, vec!["uk".to_string()])).unwrap();

        assert!(result.missing_keys.is_empty());
        assert!(result.extraction_errors.is_empty());
    }

    #[test]
    fn test_check_missing_checks_expected_path() {
        let temp = TempDir::new().unwrap();
        write(
            &temp.path().join("code/app.py"),
            r#"i18n.get("hello", _path="nested.ftl")"#,
        );
        write(
            &temp.path().join("locales/uk/_default.ftl"),
            "hello = Hello\n",
        );

        let result = check_missing(config(&temp, vec!["uk".to_string()])).unwrap();

        assert_eq!(result.missing_keys.len(), 1);
        assert_eq!(
            result.missing_keys[0].expected_file_path,
            PathBuf::from("uk").join("nested.ftl")
        );
    }

    #[test]
    fn test_check_missing_reports_extraction_conflicts() {
        let temp = TempDir::new().unwrap();
        write(
            &temp.path().join("code/app.py"),
            r#"i18n.get("hello", _path="one.ftl")
i18n.get("hello", _path="two.ftl")
"#,
        );
        write(&temp.path().join("locales/uk/one.ftl"), "hello = Hello\n");

        let result = check_missing(config(&temp, vec!["uk".to_string()])).unwrap();

        assert_eq!(result.extraction_errors.len(), 1);
        assert_eq!(result.extraction_errors[0].key, "hello");
        assert_eq!(result.extraction_errors[0].locations.len(), 2);
    }
}
