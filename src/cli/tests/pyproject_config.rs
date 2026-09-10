use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tempfile::TempDir;

fn ftl() -> Command {
    Command::new(env!("CARGO_BIN_EXE_ftl"))
}

fn write(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, content).unwrap();
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "command failed\nstatus: {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn count_occurrences(haystack: &str, needle: &str) -> usize {
    haystack.match_indices(needle).count()
}

fn pyproject(temp: &TempDir) -> PathBuf {
    temp.path().join("pyproject.toml")
}

#[test]
fn config_sample_prints_all_command_sections() {
    let output = ftl().arg("config").arg("sample").output().unwrap();

    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("[tool.ftl-extract.extract]"));
    assert!(stdout.contains("[tool.ftl-extract.stub]"));
    assert!(stdout.contains("[tool.ftl-extract.check]"));
}

#[test]
fn config_sample_can_print_one_command_section() {
    let output = ftl()
        .arg("config")
        .arg("sample")
        .arg("--command")
        .arg("stub")
        .output()
        .unwrap();

    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("[tool.ftl-extract.extract]"));
    assert!(stdout.contains("[tool.ftl-extract.stub]"));
    assert!(!stdout.contains("[tool.ftl-extract.check]"));
}

#[test]
fn config_sample_help_does_not_show_config_option() {
    let output = ftl()
        .arg("config")
        .arg("sample")
        .arg("--help")
        .output()
        .unwrap();

    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--command <COMMAND>"));
    assert!(!stdout.contains("--config <CONFIG>"));
}

#[test]
fn extract_reads_command_config_from_pyproject() {
    let temp = TempDir::new().unwrap();
    write(
        &temp.path().join("code/app.py"),
        r#"i18n.get("hello", name=user.name)"#,
    );
    write(
        &pyproject(&temp),
        r#"
[tool.ftl-extract.extract]
code-path = "code"
locales-path = "locales"
languages = ["en", "uk"]
line-endings = "lf"
"#,
    );

    let output = ftl()
        .arg("--config")
        .arg(pyproject(&temp))
        .arg("extract")
        .output()
        .unwrap();

    assert_success(&output);
    for locale in ["en", "uk"] {
        let content = std::fs::read_to_string(
            temp.path()
                .join("locales")
                .join(locale)
                .join("_default.ftl"),
        )
        .unwrap();
        assert!(content.contains("hello = hello"));
        assert!(content.contains("{ $name }"));
    }
}

#[test]
fn extract_cli_arguments_override_pyproject_config() {
    let temp = TempDir::new().unwrap();
    write(&temp.path().join("code/app.py"), r#"i18n.get("hello")"#);
    write(
        &pyproject(&temp),
        r#"
[tool.ftl-extract.extract]
code-path = "code"
locales-path = "locales"
languages = ["en"]
"#,
    );

    let output = ftl()
        .arg("--config")
        .arg(pyproject(&temp))
        .arg("extract")
        .arg("--language")
        .arg("uk")
        .output()
        .unwrap();

    assert_success(&output);
    assert!(temp.path().join("locales/uk/_default.ftl").exists());
    assert!(!temp.path().join("locales/en/_default.ftl").exists());
}

#[test]
fn stub_reads_command_config_from_pyproject() {
    let temp = TempDir::new().unwrap();
    write(
        &temp.path().join("locales/en/_default.ftl"),
        "hello-user = Hello { $name }\n",
    );
    std::fs::create_dir_all(temp.path().join("code")).unwrap();
    write(
        &pyproject(&temp),
        r#"
[tool.ftl-extract.stub]
locales-path = "locales/en"
stub-path = "code/stub.pyi"
"#,
    );

    let output = ftl()
        .arg("--config")
        .arg(pyproject(&temp))
        .arg("stub")
        .output()
        .unwrap();

    assert_success(&output);
    let stub = std::fs::read_to_string(temp.path().join("code/stub.pyi")).unwrap();
    assert!(stub.contains("class __Hello"));
    assert!(stub.contains("def user"));
    assert!(stub.contains("name: Any"));
}

#[test]
fn check_untranslated_reads_command_config_from_pyproject() {
    let temp = TempDir::new().unwrap();
    write(
        &temp.path().join("locales/en/_default.ftl"),
        "hello = Hello\n",
    );
    write(
        &temp.path().join("locales/uk/_default.ftl"),
        "hello = hello\n",
    );
    write(
        &pyproject(&temp),
        r#"
[tool.ftl-extract.check]
locales-path = "locales"
languages = ["uk"]
checks = ["untranslated"]
suggest-from = ["en"]
fail-on = []
report-path = "reports/ftl-check"
report-format = "json"
"#,
    );

    let output = ftl()
        .arg("--config")
        .arg(pyproject(&temp))
        .arg("check")
        .output()
        .unwrap();

    assert_success(&output);
    let report = std::fs::read_to_string(temp.path().join("reports/ftl-check.json")).unwrap();
    assert!(report.contains(r#""locale": "uk""#));
    assert!(report.contains(r#""key": "hello""#));
    assert!(report.contains(r#""locale": "en""#));
}

#[test]
fn check_untranslated_defaults_to_all_locales_from_cli() {
    let temp = TempDir::new().unwrap();
    write(
        &temp.path().join("locales/en/_default.ftl"),
        "hello = Hello\n",
    );
    write(
        &temp.path().join("locales/uk/_default.ftl"),
        "hello = hello\n",
    );

    let output = ftl()
        .arg("check")
        .arg(temp.path().join("locales"))
        .arg("--check")
        .arg("untranslated")
        .arg("--suggest-from")
        .arg("en")
        .arg("--fail-on")
        .arg("warn")
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("key `hello` is untranslated in locale `uk`"));
    assert!(stdout.contains("suggestion[en]: hello = Hello"));
}

#[test]
fn check_syntax_reads_command_config_from_pyproject() {
    let temp = TempDir::new().unwrap();
    write(
        &temp.path().join("locales/en/_default.ftl"),
        "valid = Valid\nbroken = {\n",
    );
    write(
        &pyproject(&temp),
        r#"
[tool.ftl-extract.check]
locales-path = "locales"
languages = ["en"]
checks = ["syntax"]
fail-on = []
report-path = "reports/ftl-check"
report-format = "json"
"#,
    );

    let output = ftl()
        .arg("--config")
        .arg(pyproject(&temp))
        .arg("check")
        .output()
        .unwrap();

    assert_success(&output);
    let report = std::fs::read_to_string(temp.path().join("reports/ftl-check.json")).unwrap();
    assert!(report.contains(r#""kind": "syntax""#));
    assert!(report.contains(r#""locale": "en""#));
    assert!(report.contains("_default.ftl"));
}

#[test]
fn check_references_reads_command_config_from_pyproject() {
    let temp = TempDir::new().unwrap();
    write(
        &temp.path().join("locales/en/_default.ftl"),
        "welcome = { missing-message }\n",
    );
    write(
        &pyproject(&temp),
        r#"
[tool.ftl-extract.check]
locales-path = "locales"
languages = ["en"]
checks = ["references"]
fail-on = []
report-path = "reports/ftl-check"
report-format = "json"
"#,
    );

    let output = ftl()
        .arg("--config")
        .arg(pyproject(&temp))
        .arg("check")
        .output()
        .unwrap();

    assert_success(&output);
    let report = std::fs::read_to_string(temp.path().join("reports/ftl-check.json")).unwrap();
    assert!(report.contains(r#""kind": "references""#));
    assert!(report.contains(r#""locale": "en""#));
    assert!(report.contains("missing-message"));
}

#[test]
fn check_missing_reads_command_config_from_pyproject() {
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
    write(
        &pyproject(&temp),
        r#"
[tool.ftl-extract.check]
locales-path = "locales"
code-path = "code"
languages = ["uk"]
checks = ["missing"]
fail-on = []
report-path = "reports/ftl-check"
report-format = "json"
"#,
    );

    let output = ftl()
        .arg("--config")
        .arg(pyproject(&temp))
        .arg("check")
        .output()
        .unwrap();

    assert_success(&output);
    let report = std::fs::read_to_string(temp.path().join("reports/ftl-check.json")).unwrap();
    assert!(report.contains(r#""kind": "missing""#));
    assert!(report.contains(r#""locale": "uk""#));
    assert!(report.contains(r#""key": "hello""#));
    assert!(report.contains(r#""code_location""#));
    assert!(report.contains("app.py"));
}

#[test]
fn check_missing_requires_code_path() {
    let temp = TempDir::new().unwrap();
    write(
        &temp.path().join("locales/uk/_default.ftl"),
        "hello = Hello\n",
    );

    let output = ftl()
        .arg("check")
        .arg(temp.path().join("locales"))
        .arg("--check")
        .arg("missing")
        .arg("--language")
        .arg("uk")
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("Missing code path"));
}

#[test]
fn check_stale_reads_command_config_from_pyproject() {
    let temp = TempDir::new().unwrap();
    write(&temp.path().join("code/app.py"), r#"i18n.get("hello")"#);
    write(
        &temp.path().join("locales/uk/_default.ftl"),
        "hello = Hello\nold = Old\n",
    );
    write(
        &pyproject(&temp),
        r#"
[tool.ftl-extract.check]
locales-path = "locales"
code-path = "code"
languages = ["uk"]
checks = ["stale"]
fail-on = []
report-path = "reports/ftl-check"
report-format = "json"
"#,
    );

    let output = ftl()
        .arg("--config")
        .arg(pyproject(&temp))
        .arg("check")
        .output()
        .unwrap();

    assert_success(&output);
    let report = std::fs::read_to_string(temp.path().join("reports/ftl-check.json")).unwrap();
    assert!(report.contains(r#""kind": "stale""#));
    assert!(report.contains(r#""locale": "uk""#));
    assert!(report.contains(r#""key": "old""#));
    assert!(report.contains("_default.ftl"));
}

#[test]
fn check_stale_requires_code_path() {
    let temp = TempDir::new().unwrap();
    write(
        &temp.path().join("locales/uk/_default.ftl"),
        "hello = Hello\n",
    );

    let output = ftl()
        .arg("check")
        .arg(temp.path().join("locales"))
        .arg("--check")
        .arg("stale")
        .arg("--language")
        .arg("uk")
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("Missing code path"));
}

#[test]
fn check_kwargs_reads_command_config_from_pyproject() {
    let temp = TempDir::new().unwrap();
    write(
        &temp.path().join("code/app.py"),
        r#"i18n.get("hello", name=user.name)"#,
    );
    write(
        &temp.path().join("locales/uk/_default.ftl"),
        "hello = Hello { $username }\n",
    );
    write(
        &pyproject(&temp),
        r#"
[tool.ftl-extract.check]
locales-path = "locales"
code-path = "code"
languages = ["uk"]
checks = ["kwargs"]
fail-on = []
report-path = "reports/ftl-check"
report-format = "json"
"#,
    );

    let output = ftl()
        .arg("--config")
        .arg(pyproject(&temp))
        .arg("check")
        .output()
        .unwrap();

    assert_success(&output);
    let report = std::fs::read_to_string(temp.path().join("reports/ftl-check.json")).unwrap();
    assert!(report.contains(r#""kind": "kwargs""#));
    assert!(report.contains(r#""key": "hello""#));
    assert!(report.contains(r#""username""#));
    assert!(report.contains(r#""name""#));
    assert!(report.contains(r#""code_location""#));
}

#[test]
fn check_kwargs_requires_code_path() {
    let temp = TempDir::new().unwrap();
    write(
        &temp.path().join("locales/uk/_default.ftl"),
        "hello = Hello { $name }\n",
    );

    let output = ftl()
        .arg("check")
        .arg(temp.path().join("locales"))
        .arg("--check")
        .arg("kwargs")
        .arg("--language")
        .arg("uk")
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("Missing code path"));
}

#[test]
fn check_defaults_to_all_checks_from_pyproject() {
    let temp = TempDir::new().unwrap();
    write(
        &temp.path().join("code/app.py"),
        r#"i18n.get("hello", name=user.name)"#,
    );
    write(
        &temp.path().join("locales/uk/_default.ftl"),
        "hello = Hello { $name }\n",
    );
    write(
        &pyproject(&temp),
        r#"
[tool.ftl-extract.check]
locales-path = "locales"
code-path = "code"
languages = ["uk"]
fail-on = []
report-path = "reports/ftl-check"
report-format = "json"
"#,
    );

    let output = ftl()
        .arg("--config")
        .arg(pyproject(&temp))
        .arg("check")
        .output()
        .unwrap();

    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("FTL check passed"));
    assert!(stdout.contains("kwargs: passed"));
    assert!(stdout.contains("missing: passed"));
    assert!(stdout.contains("references: passed"));
    assert!(stdout.contains("stale: passed"));
    assert!(stdout.contains("syntax: passed"));
    assert!(stdout.contains("untranslated: passed"));

    let report = std::fs::read_to_string(temp.path().join("reports/ftl-check.json")).unwrap();
    assert!(report.contains(r#""kind": "kwargs""#));
    assert!(report.contains(r#""kind": "missing""#));
    assert!(report.contains(r#""kind": "references""#));
    assert!(report.contains(r#""kind": "stale""#));
    assert!(report.contains(r#""kind": "syntax""#));
    assert!(report.contains(r#""kind": "untranslated""#));
}

#[test]
fn check_all_shortcut_runs_every_check() {
    let temp = TempDir::new().unwrap();
    write(
        &temp.path().join("code/app.py"),
        r#"i18n.get("hello", name=user.name)"#,
    );
    write(
        &temp.path().join("locales/uk/_default.ftl"),
        "hello = Hello { $name }\n",
    );

    let output = ftl()
        .arg("check")
        .arg(temp.path().join("locales"))
        .arg("--code-path")
        .arg(temp.path().join("code"))
        .arg("--check")
        .arg("all")
        .arg("--language")
        .arg("uk")
        .output()
        .unwrap();

    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("kwargs: passed"));
    assert!(stdout.contains("untranslated: passed"));
}

#[test]
fn check_all_stops_after_syntax_errors() {
    let temp = TempDir::new().unwrap();
    write(
        &temp.path().join("locales/en/_default.ftl"),
        "valid = Valid\nbroken = {\n",
    );

    let output = ftl()
        .arg("check")
        .arg(temp.path().join("locales"))
        .arg("--check")
        .arg("all")
        .arg("--language")
        .arg("en")
        .arg("--suggest-from")
        .arg("missing-suggest")
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stdout.contains("error[syntax]"));
    assert!(stdout.contains("syntax: failed"));
    assert!(!stdout.contains("references:"));
    assert!(!stdout.contains("untranslated:"));
    assert!(!stdout.contains("missing:"));
    assert!(!stdout.contains("stale:"));
    assert!(!stdout.contains("kwargs:"));
    assert!(!stderr.contains("Error during check"));
    assert!(!stderr.contains("Missing code path"));
    assert!(!stderr.contains("Suggestion locale `missing-suggest`"));
}

#[test]
fn custom_checks_move_syntax_first_from_pyproject() {
    let temp = TempDir::new().unwrap();
    write(
        &temp.path().join("locales/uk/_default.ftl"),
        "valid = Valid\nbroken = {\n",
    );
    write(
        &pyproject(&temp),
        r#"
[tool.ftl-extract.check]
locales-path = "locales"
languages = ["uk"]
checks = ["kwargs", "missing", "references", "stale", "syntax"]
"#,
    );

    let output = ftl()
        .arg("--config")
        .arg(pyproject(&temp))
        .arg("check")
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stdout.contains("error[syntax]"));
    assert!(stdout.contains("syntax: failed"));
    assert!(!stdout.contains("kwargs:"));
    assert!(!stdout.contains("missing:"));
    assert!(!stderr.contains("Error during check"));
    assert!(!stderr.contains("Missing code path"));
}

#[test]
fn custom_checks_move_syntax_first_from_cli() {
    let temp = TempDir::new().unwrap();
    write(
        &temp.path().join("locales/uk/_default.ftl"),
        "valid = Valid\nbroken = {\n",
    );

    let output = ftl()
        .arg("check")
        .arg(temp.path().join("locales"))
        .arg("--check")
        .arg("kwargs")
        .arg("--check")
        .arg("missing")
        .arg("--check")
        .arg("syntax")
        .arg("--language")
        .arg("uk")
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stdout.contains("error[syntax]"));
    assert!(stdout.contains("syntax: failed"));
    assert!(!stdout.contains("kwargs:"));
    assert!(!stdout.contains("missing:"));
    assert!(!stderr.contains("Error during check"));
    assert!(!stderr.contains("Missing code path"));
}

#[test]
fn custom_checks_stop_after_syntax_errors() {
    let temp = TempDir::new().unwrap();
    write(&temp.path().join("code/app.py"), r#"i18n.get("hello")"#);
    write(
        &temp.path().join("locales/uk/_default.ftl"),
        "valid = Valid\nbroken = {\n",
    );
    write(
        &pyproject(&temp),
        r#"
[tool.ftl-extract.check]
locales-path = "locales"
code-path = "code"
languages = ["uk"]
checks = ["kwargs", "missing", "syntax"]
fail-on = []
report-path = "reports/ftl-check"
report-format = "json"
"#,
    );

    let output = ftl()
        .arg("--config")
        .arg(pyproject(&temp))
        .arg("check")
        .output()
        .unwrap();

    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let report = std::fs::read_to_string(temp.path().join("reports/ftl-check.json")).unwrap();
    assert!(stdout.contains("error[syntax]"));
    assert!(!stdout.contains("kwargs:"));
    assert!(!stdout.contains("missing:"));
    assert!(report.contains(r#""kind": "syntax""#));
    assert!(!report.contains(r#""kind": "kwargs""#));
    assert!(!report.contains(r#""kind": "missing""#));
    assert!(!stderr.contains("Error during check"));
}

#[test]
fn check_all_with_valid_syntax_runs_remaining_checks() {
    let temp = TempDir::new().unwrap();
    write(
        &temp.path().join("code/app.py"),
        r#"i18n.get("hello", name=user.name)"#,
    );
    write(
        &temp.path().join("locales/uk/_default.ftl"),
        "hello = Hello { $name }\n",
    );

    let output = ftl()
        .arg("check")
        .arg(temp.path().join("locales"))
        .arg("--code-path")
        .arg(temp.path().join("code"))
        .arg("--check")
        .arg("all")
        .arg("--language")
        .arg("uk")
        .output()
        .unwrap();

    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("syntax: passed"));
    assert!(stdout.contains("references: passed"));
    assert!(stdout.contains("untranslated: passed"));
    assert!(stdout.contains("missing: passed"));
    assert!(stdout.contains("stale: passed"));
    assert!(stdout.contains("kwargs: passed"));
}

#[test]
fn check_all_deduplicates_extraction_diagnostics() {
    let temp = TempDir::new().unwrap();
    write(
        &temp.path().join("code/app.py"),
        r#"i18n.get("hello", _path="one.ftl")
i18n.get("hello", _path="two.ftl")
"#,
    );
    write(&temp.path().join("locales/uk/one.ftl"), "hello = Hello\n");
    write(
        &pyproject(&temp),
        r#"
[tool.ftl-extract.check]
locales-path = "locales"
code-path = "code"
languages = ["uk"]
checks = ["all"]
fail-on = []
report-path = "reports/ftl-check"
report-format = "json"
"#,
    );

    let output = ftl()
        .arg("--config")
        .arg(pyproject(&temp))
        .arg("check")
        .output()
        .unwrap();

    assert_success(&output);
    let report = std::fs::read_to_string(temp.path().join("reports/ftl-check.json")).unwrap();
    assert_eq!(
        count_occurrences(&report, "Fluent key hello has different paths"),
        1
    );
}

#[test]
fn check_multiple_code_aware_checks_extract_once_or_report_once() {
    let temp = TempDir::new().unwrap();
    write(
        &temp.path().join("code/app.py"),
        r#"i18n.get("hello", _path="one.ftl")
i18n.get("hello", _path="two.ftl")
"#,
    );
    write(&temp.path().join("locales/uk/one.ftl"), "hello = Hello\n");
    write(
        &pyproject(&temp),
        r#"
[tool.ftl-extract.check]
locales-path = "locales"
code-path = "code"
languages = ["uk"]
checks = ["missing", "stale", "kwargs"]
fail-on = []
report-path = "reports/ftl-check"
report-format = "json"
"#,
    );

    let output = ftl()
        .arg("--config")
        .arg(pyproject(&temp))
        .arg("check")
        .output()
        .unwrap();

    assert_success(&output);
    let report = std::fs::read_to_string(temp.path().join("reports/ftl-check.json")).unwrap();
    assert_eq!(
        count_occurrences(&report, "Fluent key hello has different paths"),
        1
    );
}

#[test]
fn extract_refuses_to_write_when_python_file_is_broken() {
    let temp = TempDir::new().unwrap();
    write(&temp.path().join("code/good.py"), r#"i18n.get("hello")"#);
    write(&temp.path().join("code/broken.py"), "i18n.get(\"old\"\n");
    write(&temp.path().join("locales/en/_default.ftl"), "old = Old\n");

    let output = ftl()
        .arg("extract")
        .arg(temp.path().join("code"))
        .arg(temp.path().join("locales"))
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Extraction aborted"), "{stderr}");
    assert!(stderr.contains("[parse-error]"), "{stderr}");
    assert!(stderr.contains("broken.py"), "{stderr}");
    assert!(stderr.contains("--allow-parse-errors"), "{stderr}");
    assert_eq!(
        std::fs::read_to_string(temp.path().join("locales/en/_default.ftl")).unwrap(),
        "old = Old\n"
    );
}

#[test]
fn extract_allow_parse_errors_flag_continues_past_broken_files() {
    let temp = TempDir::new().unwrap();
    write(&temp.path().join("code/good.py"), r#"i18n.get("hello")"#);
    write(&temp.path().join("code/broken.py"), "i18n.get(");

    let output = ftl()
        .arg("extract")
        .arg(temp.path().join("code"))
        .arg(temp.path().join("locales"))
        .arg("--allow-parse-errors")
        .output()
        .unwrap();

    assert_success(&output);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Skipping Python file"), "{stderr}");
    assert!(stderr.contains("broken.py"), "{stderr}");
    let content = std::fs::read_to_string(temp.path().join("locales/en/_default.ftl")).unwrap();
    assert!(content.contains("hello = hello"));
}

#[test]
fn extract_allow_parse_errors_reads_from_pyproject() {
    let temp = TempDir::new().unwrap();
    write(&temp.path().join("code/good.py"), r#"i18n.get("hello")"#);
    write(&temp.path().join("code/broken.py"), "i18n.get(");
    write(
        &pyproject(&temp),
        r#"
[tool.ftl-extract.extract]
code-path = "code"
locales-path = "locales"
allow-parse-errors = true
"#,
    );

    let output = ftl()
        .arg("--config")
        .arg(pyproject(&temp))
        .arg("extract")
        .output()
        .unwrap();

    assert_success(&output);
    assert!(temp.path().join("locales/en/_default.ftl").exists());
}

#[test]
fn check_reports_python_parse_errors_as_extraction_diagnostics() {
    let temp = TempDir::new().unwrap();
    write(&temp.path().join("code/good.py"), r#"i18n.get("hello")"#);
    write(&temp.path().join("code/broken.py"), "i18n.get(\"old\"\n");
    write(
        &temp.path().join("locales/uk/_default.ftl"),
        "hello = Hello\nold = Old\n",
    );

    let output = ftl()
        .arg("check")
        .arg(temp.path().join("locales"))
        .arg("--code-path")
        .arg(temp.path().join("code"))
        .arg("--check")
        .arg("stale")
        .arg("--report-path")
        .arg(temp.path().join("reports/ftl-check"))
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("error[extraction]"), "{stdout}");
    assert!(stdout.contains("Failed to parse"), "{stdout}");
    assert!(stdout.contains("broken.py"), "{stdout}");

    let report = std::fs::read_to_string(temp.path().join("reports/ftl-check.json")).unwrap();
    assert!(report.contains(r#""kind": "extraction""#));
    assert!(report.contains(r#""key": null"#));
    assert!(report.contains("broken.py"));
}

#[test]
fn extract_conflicting_paths_exit_with_both_call_sites() {
    let temp = TempDir::new().unwrap();
    write(
        &temp.path().join("code/a.py"),
        r#"i18n.get("hello", _path="one.ftl")"#,
    );
    write(
        &temp.path().join("code/b.py"),
        "\n\ni18n.get(\"hello\", _path=\"two.ftl\")\n",
    );

    let output = ftl()
        .arg("extract")
        .arg(temp.path().join("code"))
        .arg(temp.path().join("locales"))
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("[key-path-conflict]"), "{stderr}");
    assert!(stderr.contains("a.py:1:1"), "{stderr}");
    assert!(stderr.contains("b.py:3:1"), "{stderr}");
    assert!(!stderr.contains("panicked"), "{stderr}");
    assert!(!temp.path().join("locales").exists());
}

#[test]
fn extract_reports_dangling_ftl_reference_instead_of_panicking() {
    let temp = TempDir::new().unwrap();
    write(&temp.path().join("code/app.py"), r#"i18n.get("hello")"#);
    write(
        &temp.path().join("locales/en/_default.ftl"),
        "hello = Hello { missing-message }\n",
    );

    let output = ftl()
        .arg("extract")
        .arg(temp.path().join("code"))
        .arg(temp.path().join("locales"))
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("references unknown message `missing-message`"),
        "{stderr}"
    );
    assert!(!stderr.contains("panicked"), "{stderr}");
    assert_eq!(
        std::fs::read_to_string(temp.path().join("locales/en/_default.ftl")).unwrap(),
        "hello = Hello { missing-message }\n"
    );
}
