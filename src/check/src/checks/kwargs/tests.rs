use super::check_kwargs;
use crate::types::CheckKwargsConfig;
use common::FastHashSet;
use extractor::ftl::consts::{
    DEFAULT_EXCLUDE_DIRS, DEFAULT_FTL_FILENAME, DEFAULT_I18N_KEYS, DEFAULT_IGNORE_ATTRIBUTES,
    DEFAULT_IGNORE_KWARGS,
};
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

fn write(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, content).unwrap();
}

fn config(temp: &TempDir, locales: Vec<String>) -> CheckKwargsConfig {
    CheckKwargsConfig {
        locales_path: temp.path().join("locales"),
        code_path: temp.path().join("code"),
        locales,
        i18n_keys: DEFAULT_I18N_KEYS.clone(),
        i18n_keys_prefix: FastHashSet::default(),
        exclude_dirs: DEFAULT_EXCLUDE_DIRS.clone(),
        ignore_attributes: DEFAULT_IGNORE_ATTRIBUTES.clone(),
        ignore_kwargs: DEFAULT_IGNORE_KWARGS.clone(),
        default_ftl_file: PathBuf::from(DEFAULT_FTL_FILENAME),
        cache: false,
        cache_path: None,
        clear_cache: false,
    }
}

#[test]
fn test_check_kwargs_accepts_matching_variables() {
    let temp = TempDir::new().unwrap();
    write(
        &temp.path().join("code/app.py"),
        r#"i18n.get("hello", name=user.name)"#,
    );
    write(
        &temp.path().join("locales/uk/_default.ftl"),
        "hello = Hello { $name }\n",
    );

    let result = check_kwargs(config(&temp, vec!["uk".to_string()])).unwrap();

    assert!(result.mismatches.is_empty());
    assert!(result.extraction_errors.is_empty());
}

#[test]
fn test_check_kwargs_reports_missing_and_unused_variables() {
    let temp = TempDir::new().unwrap();
    write(
        &temp.path().join("code/app.py"),
        r#"def handler():
    i18n.get("hello", name=user.name)
"#,
    );
    write(
        &temp.path().join("locales/uk/_default.ftl"),
        "hello = Hello { $username }\n",
    );

    let result = check_kwargs(config(&temp, vec!["uk".to_string()])).unwrap();

    assert_eq!(result.mismatches.len(), 1);
    assert_eq!(result.mismatches[0].key, "hello");
    assert_eq!(result.mismatches[0].missing_kwargs, vec!["username"]);
    assert_eq!(result.mismatches[0].unused_kwargs, vec!["name"]);
    assert_eq!(result.mismatches[0].line, Some(1));
    assert_eq!(
        result.mismatches[0].code_location.as_ref().unwrap().line,
        Some(2)
    );
}

#[test]
fn test_check_kwargs_checks_expected_path() {
    let temp = TempDir::new().unwrap();
    write(
        &temp.path().join("code/app.py"),
        r#"i18n.get("hello", name=user.name, _path="nested.ftl")"#,
    );
    write(
        &temp.path().join("locales/uk/_default.ftl"),
        "hello = Hello { $username }\n",
    );
    write(
        &temp.path().join("locales/uk/nested.ftl"),
        "hello = Hello { $name }\n",
    );

    let result = check_kwargs(config(&temp, vec!["uk".to_string()])).unwrap();

    assert!(result.mismatches.is_empty());
}

#[test]
fn test_check_kwargs_includes_referenced_message_variables() {
    let temp = TempDir::new().unwrap();
    write(
        &temp.path().join("code/app.py"),
        r#"i18n.get("welcome", name=user.name)"#,
    );
    write(
        &temp.path().join("locales/uk/_default.ftl"),
        "welcome = { title }\ntitle = Welcome { $name }\n",
    );

    let result = check_kwargs(config(&temp, vec!["uk".to_string()])).unwrap();

    assert!(result.mismatches.is_empty());
}

#[test]
fn test_check_kwargs_term_string_argument_satisfies_term_variable() {
    let temp = TempDir::new().unwrap();
    write(&temp.path().join("code/app.py"), r#"i18n.get("title")"#);
    write(
        &temp.path().join("locales/uk/_default.ftl"),
        r#"-brand = { $case ->
    [genitive] Brand
   *[nominative] Brand
}

title = Welcome to { -brand(case: "genitive") }
"#,
    );

    let result = check_kwargs(config(&temp, vec!["uk".to_string()])).unwrap();

    assert!(result.mismatches.is_empty());
}

#[test]
fn test_check_kwargs_term_number_argument_satisfies_term_variable() {
    let temp = TempDir::new().unwrap();
    write(&temp.path().join("code/app.py"), r#"i18n.get("title")"#);
    write(
        &temp.path().join("locales/uk/_default.ftl"),
        r#"-brand = { $case ->
    [1] Brand
   *[0] Brand
}

title = Welcome to { -brand(case: 1) }
"#,
    );

    let result = check_kwargs(config(&temp, vec!["uk".to_string()])).unwrap();

    assert!(result.mismatches.is_empty());
}

#[test]
fn test_check_kwargs_term_without_argument_does_not_require_term_variable() {
    // `$case` is the term's parameter. Unbound, it falls back to the default variant; the
    // caller's kwargs are never consulted, so nothing is required from code.
    let temp = TempDir::new().unwrap();
    write(&temp.path().join("code/app.py"), r#"i18n.get("title")"#);
    write(
        &temp.path().join("locales/uk/_default.ftl"),
        r#"-brand = { $case ->
    [genitive] Brand
   *[nominative] Brand
}

title = Welcome to { -brand }
"#,
    );

    let result = check_kwargs(config(&temp, vec!["uk".to_string()])).unwrap();

    assert!(result.mismatches.is_empty());
}

#[test]
fn test_check_kwargs_term_local_binding_does_not_hide_message_requirement_outside_term() {
    let temp = TempDir::new().unwrap();
    write(&temp.path().join("code/app.py"), r#"i18n.get("title")"#);
    write(
        &temp.path().join("locales/uk/_default.ftl"),
        r#"-brand = { subtitle }
subtitle = Brand { $case }
wrapper = { subtitle }
title = { -brand(case: "genitive") } { wrapper }
"#,
    );

    let result = check_kwargs(config(&temp, vec!["uk".to_string()])).unwrap();

    assert_eq!(result.mismatches.len(), 1);
    assert_eq!(result.mismatches[0].missing_kwargs, vec!["case"]);
    assert!(result.mismatches[0].unused_kwargs.is_empty());
}

#[test]
fn test_check_kwargs_message_reference_still_propagates_variables() {
    let temp = TempDir::new().unwrap();
    write(
        &temp.path().join("code/app.py"),
        r#"i18n.get("welcome", name=user.name)"#,
    );
    write(
        &temp.path().join("locales/uk/_default.ftl"),
        "welcome = { title }\ntitle = Welcome { $name }\n",
    );

    let result = check_kwargs(config(&temp, vec!["uk".to_string()])).unwrap();

    assert!(result.mismatches.is_empty());
}

#[test]
fn test_check_kwargs_referenced_message_reports_missing_variables() {
    let temp = TempDir::new().unwrap();
    write(&temp.path().join("code/app.py"), r#"i18n.get("welcome")"#);
    write(
        &temp.path().join("locales/uk/_default.ftl"),
        "welcome = { title }\ntitle = Welcome { $name }\n",
    );

    let result = check_kwargs(config(&temp, vec!["uk".to_string()])).unwrap();

    assert_eq!(result.mismatches.len(), 1);
    assert_eq!(result.mismatches[0].key, "welcome");
    assert_eq!(result.mismatches[0].missing_kwargs, vec!["name"]);
    assert!(result.mismatches[0].unused_kwargs.is_empty());
}

#[test]
fn test_check_kwargs_referenced_message_uses_term_literal_argument() {
    let temp = TempDir::new().unwrap();
    write(&temp.path().join("code/app.py"), r#"i18n.get("title")"#);
    write(
        &temp.path().join("locales/uk/_default.ftl"),
        r#"title = { subtitle }
subtitle = Welcome to { -brand(case: "genitive") }
-brand = { $case ->
    [genitive] Brand
   *[nominative] Brand
}
"#,
    );

    let result = check_kwargs(config(&temp, vec!["uk".to_string()])).unwrap();
    assert!(result.mismatches.is_empty());
}

#[test]
fn test_check_kwargs_function_literal_named_argument_does_not_require_kwarg() {
    let temp = TempDir::new().unwrap();
    write(
        &temp.path().join("code/app.py"),
        r#"i18n.get("updated", created_at=created_at)"#,
    );
    write(
        &temp.path().join("locales/uk/_default.ftl"),
        r#"updated = Updated at { DATETIME($created_at, month: "long") }
"#,
    );

    let result = check_kwargs(config(&temp, vec!["uk".to_string()])).unwrap();
    assert!(result.mismatches.is_empty());

    write(&temp.path().join("code/app.py"), r#"i18n.get("updated")"#);
    let result = check_kwargs(config(&temp, vec!["uk".to_string()])).unwrap();
    assert_eq!(result.mismatches.len(), 1);
    assert_eq!(result.mismatches[0].missing_kwargs, vec!["created_at"]);
    assert!(
        !result.mismatches[0]
            .missing_kwargs
            .contains(&"month".to_string())
    );
    assert!(result.mismatches[0].unused_kwargs.is_empty());
}

#[test]
fn test_check_kwargs_missing_and_unused_variables_are_sorted() {
    let temp = TempDir::new().unwrap();
    write(
        &temp.path().join("code/app.py"),
        r#"i18n.get("hello", d=4, c=3)"#,
    );
    write(
        &temp.path().join("locales/uk/_default.ftl"),
        "hello = Hello { $b } { $a }\n",
    );

    let result = check_kwargs(config(&temp, vec!["uk".to_string()])).unwrap();

    assert_eq!(result.mismatches.len(), 1);
    assert_eq!(result.mismatches[0].missing_kwargs, vec!["a", "b"]);
    assert_eq!(result.mismatches[0].unused_kwargs, vec!["c", "d"]);
}

#[test]
fn test_check_kwargs_recursive_terms_with_literal_local_bindings_terminate() {
    let temp = TempDir::new().unwrap();
    write(&temp.path().join("code/app.py"), r#"i18n.get("title")"#);
    write(
        &temp.path().join("locales/uk/_default.ftl"),
        r#"title = { -outer(case: "genitive") }
-outer = { -inner(case: "genitive") }
-inner = { -outer(case: "genitive") } { $case }
"#,
    );

    let result = check_kwargs(config(&temp, vec!["uk".to_string()])).unwrap();

    assert!(result.mismatches.is_empty());
}

#[test]
fn test_check_kwargs_reports_extraction_conflicts() {
    let temp = TempDir::new().unwrap();
    write(
        &temp.path().join("code/app.py"),
        r#"i18n.get("hello", _path="one.ftl")
i18n.get("hello", _path="two.ftl")
"#,
    );
    write(&temp.path().join("locales/uk/one.ftl"), "hello = Hello\n");

    let result = check_kwargs(config(&temp, vec!["uk".to_string()])).unwrap();

    assert_eq!(result.extraction_errors.len(), 1);
    assert_eq!(result.extraction_errors[0].key.as_deref(), Some("hello"));
}

#[test]
fn test_check_kwargs_unbound_term_variable_is_not_required_from_code() {
    // Variables inside a term resolve against the term's call arguments only; the caller's
    // kwargs are never consulted, so an unbound `$suffix` is a term problem, not a code one.
    let temp = TempDir::new().unwrap();
    write(&temp.path().join("code/app.py"), r#"i18n.get("about")"#);
    write(
        &temp.path().join("locales/uk/_default.ftl"),
        "-brand = Bot { $suffix }\nabout = About { -brand(case: \"gen\") }\n",
    );

    let result = check_kwargs(config(&temp, vec!["uk".to_string()])).unwrap();

    assert!(result.mismatches.is_empty());
}

#[test]
fn test_check_kwargs_kwarg_matching_a_term_variable_is_unused() {
    let temp = TempDir::new().unwrap();
    write(
        &temp.path().join("code/app.py"),
        r#"i18n.get("about", suffix="x")"#,
    );
    write(
        &temp.path().join("locales/uk/_default.ftl"),
        "-brand = Bot { $suffix }\nabout = About { -brand }\n",
    );

    let result = check_kwargs(config(&temp, vec!["uk".to_string()])).unwrap();

    assert_eq!(result.mismatches.len(), 1);
    assert!(result.mismatches[0].missing_kwargs.is_empty());
    assert_eq!(result.mismatches[0].unused_kwargs, vec!["suffix"]);
}

#[test]
fn test_check_kwargs_function_positional_argument_requires_kwarg() {
    let temp = TempDir::new().unwrap();
    write(&temp.path().join("code/app.py"), r#"i18n.get("items")"#);
    write(
        &temp.path().join("locales/uk/_default.ftl"),
        "items =\n    { NUMBER($count) ->\n        [one] One item\n       *[other] Many items\n    }\n",
    );

    let result = check_kwargs(config(&temp, vec!["uk".to_string()])).unwrap();

    assert_eq!(result.mismatches.len(), 1);
    assert_eq!(result.mismatches[0].missing_kwargs, vec!["count"]);
}

#[test]
fn test_check_kwargs_attribute_reference_follows_only_that_attribute() {
    let temp = TempDir::new().unwrap();
    write(
        &temp.path().join("code/app.py"),
        r#"i18n.get("a", x=1)
i18n.get("b", x=1, v=2)
"#,
    );
    write(
        &temp.path().join("locales/uk/_default.ftl"),
        "b = B { $v }\n    .title = Title { $x }\n    .other = { $y }\na = See { b.title }\n",
    );

    let result = check_kwargs(config(&temp, vec!["uk".to_string()])).unwrap();

    // `a` needs only `x`; `b` needs only its value's `v`, so the `x` passed to `b` is unused.
    assert_eq!(result.mismatches.len(), 1);
    assert_eq!(result.mismatches[0].key, "b");
    assert!(result.mismatches[0].missing_kwargs.is_empty());
    assert_eq!(result.mismatches[0].unused_kwargs, vec!["x"]);
}

#[test]
fn test_check_kwargs_message_reference_does_not_pull_attributes() {
    let temp = TempDir::new().unwrap();
    write(
        &temp.path().join("code/app.py"),
        r#"i18n.get("a")
i18n.get("b")
"#,
    );
    write(
        &temp.path().join("locales/uk/_default.ftl"),
        "b = B\n    .title = Title { $x }\na = See { b }\n",
    );

    let result = check_kwargs(config(&temp, vec!["uk".to_string()])).unwrap();

    assert!(result.mismatches.is_empty());
}

#[test]
fn test_check_kwargs_own_attributes_are_not_required() {
    // `i18n.get("b")` renders only the value of `b`, so `$x` in `.title` is never read.
    let temp = TempDir::new().unwrap();
    write(&temp.path().join("code/app.py"), r#"i18n.get("b")"#);
    write(
        &temp.path().join("locales/uk/_default.ftl"),
        "b = B\n    .title = Title { $x }\n",
    );

    let result = check_kwargs(config(&temp, vec!["uk".to_string()])).unwrap();

    assert!(result.mismatches.is_empty());
}

#[test]
fn test_check_kwargs_kwarg_matching_only_an_own_attribute_is_unused() {
    let temp = TempDir::new().unwrap();
    write(&temp.path().join("code/app.py"), r#"i18n.get("b", x=1)"#);
    write(
        &temp.path().join("locales/uk/_default.ftl"),
        "b = B\n    .title = Title { $x }\n",
    );

    let result = check_kwargs(config(&temp, vec!["uk".to_string()])).unwrap();

    assert_eq!(result.mismatches.len(), 1);
    assert_eq!(result.mismatches[0].key, "b");
    assert!(result.mismatches[0].missing_kwargs.is_empty());
    assert_eq!(result.mismatches[0].unused_kwargs, vec!["x"]);
}

#[test]
fn test_check_kwargs_skips_keys_called_with_double_star_kwargs() {
    // `**data` can pass `name`, so the key cannot be verified and is not a mismatch.
    let temp = TempDir::new().unwrap();
    write(
        &temp.path().join("code/app.py"),
        r#"i18n.get("welcome", **data)
i18n.get("bye", name=user.name)
"#,
    );
    write(
        &temp.path().join("locales/uk/_default.ftl"),
        "welcome = Welcome, { $name }!\nbye = Bye { $username }\n",
    );

    let result = check_kwargs(config(&temp, vec!["uk".to_string()])).unwrap();

    // Only the verifiable key is reported.
    assert_eq!(result.mismatches.len(), 1);
    assert_eq!(result.mismatches[0].key, "bye");
}
