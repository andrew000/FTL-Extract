use super::check_references;
use crate::types::CheckReferencesConfig;
use anyhow::Result;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_check_references_reports_missing_message_and_term() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let locales = temp_dir.path().join("locales");
    fs::create_dir_all(locales.join("en"))?;
    fs::write(
        locales.join("en").join("_default.ftl"),
        "welcome = { missing-message }\nbrand = { -missing-term }\n",
    )?;

    let result = check_references(CheckReferencesConfig {
        locales_path: locales,
        locales: vec!["en".to_string()],
    })?;

    assert_eq!(result.missing_references.len(), 2);
    assert!(
        result
            .missing_references
            .iter()
            .any(|item| item.reference == "missing-message")
    );
    assert!(
        result
            .missing_references
            .iter()
            .any(|item| item.reference == "-missing-term")
    );
    Ok(())
}

#[test]
fn test_check_references_accepts_existing_references() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let locales = temp_dir.path().join("locales");
    fs::create_dir_all(locales.join("en"))?;
    fs::write(
        locales.join("en").join("_default.ftl"),
        "welcome = { title } { -brand }\ntitle = Welcome\n-brand = Brand\n",
    )?;

    let result = check_references(CheckReferencesConfig {
        locales_path: locales,
        locales: vec!["en".to_string()],
    })?;

    assert!(result.missing_references.is_empty());
    Ok(())
}

#[test]
fn test_check_references_reports_missing_term_argument_reference() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let locales = temp_dir.path().join("locales");
    fs::create_dir_all(locales.join("en"))?;
    fs::write(
        locales.join("en").join("_default.ftl"),
        "-brand = Brand { $case }\nwelcome = { -brand(case: missing-message) }\n",
    )?;

    let result = check_references(CheckReferencesConfig {
        locales_path: locales,
        locales: vec!["en".to_string()],
    })?;

    assert_eq!(result.missing_references.len(), 1);
    assert_eq!(result.missing_references[0].reference, "missing-message");
    Ok(())
}

#[test]
fn test_check_references_reports_missing_message_attribute() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let locales = temp_dir.path().join("locales");
    fs::create_dir_all(locales.join("en"))?;
    fs::write(
        locales.join("en").join("_default.ftl"),
        "title = Title\nwelcome = { title.missing }\n",
    )?;

    let result = check_references(CheckReferencesConfig {
        locales_path: locales,
        locales: vec!["en".to_string()],
    })?;

    assert_eq!(result.missing_references.len(), 1);
    assert!(
        result
            .missing_references
            .iter()
            .any(|item| item.reference == "title.missing")
    );
    Ok(())
}

#[test]
fn test_check_references_accepts_existing_message_attribute() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let locales = temp_dir.path().join("locales");
    fs::create_dir_all(locales.join("en"))?;
    fs::write(
        locales.join("en").join("_default.ftl"),
        "title = Title\n    .short = T\nwelcome = { title.short }\n",
    )?;

    let result = check_references(CheckReferencesConfig {
        locales_path: locales,
        locales: vec!["en".to_string()],
    })?;

    assert!(result.missing_references.is_empty());
    Ok(())
}

#[test]
fn test_check_references_reports_missing_references_in_select_variants() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let locales = temp_dir.path().join("locales");
    fs::create_dir_all(locales.join("en"))?;
    fs::write(
        locales.join("en").join("_default.ftl"),
        r#"choice = { $count ->
    [one] { missing-one }
   *[other] { -missing-term }
}
"#,
    )?;

    let result = check_references(CheckReferencesConfig {
        locales_path: locales,
        locales: vec!["en".to_string()],
    })?;

    assert_eq!(result.missing_references.len(), 2);
    assert!(
        result
            .missing_references
            .iter()
            .any(|item| item.reference == "missing-one")
    );
    assert!(
        result
            .missing_references
            .iter()
            .any(|item| item.reference == "-missing-term")
    );
    Ok(())
}

#[test]
fn test_check_references_reports_real_source_line() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let locales = temp_dir.path().join("locales");
    fs::create_dir_all(locales.join("en"))?;
    fs::write(
        locales.join("en").join("_default.ftl"),
        "# comment\n\nwelcome = { missing }\n",
    )?;

    let result = check_references(CheckReferencesConfig {
        locales_path: locales,
        locales: vec!["en".to_string()],
    })?;

    assert_eq!(result.missing_references[0].line, Some(3));
    Ok(())
}

#[test]
fn test_check_references_reports_missing_references_in_nested_places() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let locales = temp_dir.path().join("locales");
    fs::create_dir_all(locales.join("en"))?;
    fs::write(
        locales.join("en").join("_default.ftl"),
        concat!(
            "-brand = Brand\n",
            "    .short = { missing-in-term-attribute }\n",
            "welcome = { NUMBER(missing-positional, style: \"percent\") } { { missing-in-placeable } }\n",
            "    { -brand(missing-term-positional) }\n",
            "pick = { -brand.missing-attribute ->\n",
            "       *[other] Other\n",
            "    }\n",
        ),
    )?;

    let result = check_references(CheckReferencesConfig {
        locales_path: locales,
        locales: vec!["en".to_string()],
    })?;

    let mut found = result
        .missing_references
        .iter()
        .map(|item| (item.key.clone().unwrap_or_default(), item.reference.clone()))
        .collect::<Vec<_>>();
    found.sort();
    let expected = [
        ("-brand", "missing-in-term-attribute"),
        ("pick", "-brand.missing-attribute"),
        ("welcome", "missing-in-placeable"),
        ("welcome", "missing-positional"),
        ("welcome", "missing-term-positional"),
    ]
    .map(|(key, reference)| (key.to_string(), reference.to_string()));
    assert_eq!(found, expected);
    Ok(())
}

#[test]
fn test_check_references_reports_the_line_after_lines_that_look_like_entries() -> Result<()> {
    // Lines of a multiline value that start like a term or a message but have no `=` must
    // not be taken for entry starts when the line of `broken` is looked up.
    let temp_dir = TempDir::new()?;
    let locales = temp_dir.path().join("locales");
    fs::create_dir_all(locales.join("en"))?;
    fs::write(
        locales.join("en").join("_default.ftl"),
        concat!(
            "long =\n",
            "    first line\n",
            "    -not a term\n",
            "    plain words\n",
            "broken = { missing }\n",
        ),
    )?;

    let result = check_references(CheckReferencesConfig {
        locales_path: locales,
        locales: vec!["en".to_string()],
    })?;

    assert_eq!(result.missing_references.len(), 1);
    assert_eq!(result.missing_references[0].line, Some(5));
    Ok(())
}
