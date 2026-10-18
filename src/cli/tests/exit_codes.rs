//! Exit codes and messages of the error paths of `ftl`: configuration errors exit `2`, a failed
//! `extract` or `stub` run, an `extract --dry-run` that would change a file, a broken
//! `pyproject.toml` and an unwritable report exit `1`, and a failed `check` run exits `2`.

use std::path::Path;
use std::process::{Command, Output};
use tempfile::TempDir;

fn ftl(dir: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ftl"));
    // Run inside the temporary directory, so no `pyproject.toml` of this repository is found.
    command.current_dir(dir);
    command
}

fn write(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, content).unwrap();
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// Runs `ftl` with `args` in a fresh directory holding `pyproject.toml` with `config`, a code
/// file and an `en` locale, and returns the output.
fn run_with_config(config: &str, args: &[&str]) -> Output {
    let temp = TempDir::new().unwrap();
    write(&temp.path().join("code/app.py"), r#"i18n.get("hello")"#);
    write(
        &temp.path().join("locales/en/_default.ftl"),
        "hello = Hello\n",
    );
    write(&temp.path().join("pyproject.toml"), config);
    ftl(temp.path()).args(args).output().unwrap()
}

fn assert_exit(output: &Output, code: i32, message: &str) {
    let stderr = stderr(output);
    assert_eq!(output.status.code(), Some(code), "{stderr}");
    assert!(
        stderr.contains(message),
        "expected {message:?} in:\n{stderr}"
    );
}

#[test]
fn extract_without_paths_is_a_configuration_error() {
    let output = run_with_config("[tool.ftl-extract.extract]\n", &["extract"]);
    assert_exit(&output, 2, "Missing code path");

    let output = run_with_config(
        "[tool.ftl-extract.extract]\ncode-path = \"code\"\n",
        &["extract"],
    );
    assert_exit(&output, 2, "Missing locales path");
}

#[test]
fn extract_dry_run_exits_1_only_when_a_file_would_change() {
    let config = "[tool.ftl-extract.extract]\ncode-path = \"code\"\nlocales-path = \"locales\"\n";

    // `en/_default.ftl` already holds the only key.
    let output = run_with_config(config, &["extract", "--dry-run"]);
    assert_exit(&output, 0, "FTL files changed: 0");

    // A second locale would be created.
    let output = run_with_config(config, &["extract", "--dry-run", "-l", "en", "-l", "uk"]);
    assert_exit(&output, 1, "Dry run: 1 .ftl file(s) would change");
    assert!(stderr(&output).contains("[DRY-RUN] Would write"));
}

#[test]
fn extract_rejects_unknown_enum_values_in_pyproject() {
    let base = "[tool.ftl-extract.extract]\ncode-path = \"code\"\nlocales-path = \"locales\"\n";

    let output = run_with_config(
        &format!("{base}comment-keys-mode = \"delete\"\n"),
        &["extract"],
    );
    assert_exit(&output, 2, "Invalid `comment-keys-mode` value `delete`");

    let output = run_with_config(&format!("{base}line-endings = \"nl\"\n"), &["extract"]);
    assert_exit(&output, 2, "Invalid `line-endings` value `nl`");
}

#[test]
fn a_pyproject_that_does_not_parse_exits_1() {
    let output = run_with_config("[tool.ftl-extract.extract\n", &["extract"]);
    assert_exit(&output, 1, "Error loading config");
}

#[test]
fn extract_verbose_without_any_pyproject() {
    // No `pyproject.toml` in the directory or above it: the search walks up to the root and
    // the paths come from the command line.
    let temp = TempDir::new().unwrap();
    write(&temp.path().join("code/app.py"), r#"i18n.get("hello")"#);
    std::fs::create_dir_all(temp.path().join("locales/en")).unwrap();

    let output = ftl(temp.path())
        .args(["extract", "code", "locales", "--verbose"])
        .output()
        .unwrap();

    let stderr = stderr(&output);
    assert_eq!(output.status.code(), Some(0), "{stderr}");
    assert!(stderr.contains("[DEBUG"), "{stderr}");
    assert!(stderr.contains("Done in"), "{stderr}");
}

#[test]
fn stub_without_paths_is_a_configuration_error() {
    let output = run_with_config("[tool.ftl-extract.stub]\n", &["stub"]);
    assert_exit(&output, 2, "Missing locales path");

    let output = run_with_config(
        "[tool.ftl-extract.stub]\nlocales-path = \"locales/en\"\n",
        &["stub"],
    );
    assert_exit(&output, 2, "Missing stub path");
}

#[test]
fn stub_that_cannot_be_written_exits_1() {
    // `blocker` is a file, so the stub cannot be written below it.
    let temp = TempDir::new().unwrap();
    write(
        &temp.path().join("locales/en/_default.ftl"),
        "hello = Hello\n",
    );
    write(&temp.path().join("blocker"), "");

    let output = ftl(temp.path())
        .args(["stub", "locales/en", "blocker/stub.pyi", "--verbose"])
        .output()
        .unwrap();

    assert_exit(&output, 1, "Error during stub generation");
}

#[test]
fn check_configuration_errors_exit_2() {
    let output = run_with_config("[tool.ftl-extract.check]\n", &["check"]);
    assert_exit(&output, 2, "Missing locales path");

    let base = "[tool.ftl-extract.check]\nlocales-path = \"locales\"\n";
    for (line, message) in [
        (
            "checks = [\"everything\"]",
            "Invalid `checks` value `everything`",
        ),
        (
            "report-format = \"xml\"",
            "Invalid `report-format` value `xml`",
        ),
        ("fail-on = [\"never\"]", "Invalid `fail-on` value `never`"),
    ] {
        let output = run_with_config(&format!("{base}{line}\n"), &["check"]);
        assert_exit(&output, 2, message);
    }
}

#[test]
fn check_with_an_unknown_suggestion_locale_exits_2() {
    let output = run_with_config(
        "[tool.ftl-extract.check]\nlocales-path = \"locales\"\nchecks = [\"untranslated\"]\nsuggest-from = [\"xx\"]\n",
        &["check"],
    );
    assert_exit(&output, 2, "Error during check");
}

#[test]
fn check_report_that_cannot_be_written_exits_1() {
    let temp = TempDir::new().unwrap();
    write(
        &temp.path().join("locales/en/_default.ftl"),
        "hello = Hello\n",
    );
    write(&temp.path().join("blocker"), "");

    let output = ftl(temp.path())
        .args([
            "check",
            "locales",
            "--check",
            "syntax",
            "--report-path",
            "blocker/report",
        ])
        .output()
        .unwrap();

    assert_exit(&output, 1, "Failed to write report file");
}

#[test]
fn check_writes_a_terminal_report_next_to_the_working_directory() {
    let temp = TempDir::new().unwrap();
    write(
        &temp.path().join("locales/en/_default.ftl"),
        "hello = Hello\n",
    );

    let output = ftl(temp.path())
        .args([
            "check",
            "locales",
            "--check",
            "syntax",
            "--report-format",
            "terminal",
            "--report-path",
            "report",
        ])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let report = std::fs::read_to_string(temp.path().join("report.txt")).unwrap();
    assert!(report.contains("syntax: passed"), "{report}");
}

#[test]
fn no_command_prints_a_hint_and_succeeds() {
    let temp = TempDir::new().unwrap();
    let output = ftl(temp.path()).output().unwrap();

    assert_exit(&output, 0, "No command provided");
}

#[test]
fn check_rejects_unknown_locales() {
    // A checked locale that does not exist.
    let output = run_with_config(
        "[tool.ftl-extract.check]\nlocales-path = \"locales\"\nlanguages = [\"xx\"]\nchecks = [\"syntax\"]\n",
        &["check"],
    );
    assert_exit(&output, 2, "Locale `xx` does not exist");

    // A suggestion locale that does not exist, loaded after `syntax` loaded the checked ones.
    let output = run_with_config(
        "[tool.ftl-extract.check]\nlocales-path = \"locales\"\nlanguages = [\"en\"]\nchecks = [\"syntax\", \"untranslated\"]\nsuggest-from = [\"xx\"]\n",
        &["check"],
    );
    assert_exit(&output, 2, "Locale `xx` does not exist");
}

#[test]
fn check_suggestion_locale_that_is_already_checked() {
    // `en` is both checked and the suggestion locale: `syntax` already loaded it, so
    // `untranslated` has nothing more to load.
    let output = run_with_config(
        "[tool.ftl-extract.check]\nlocales-path = \"locales\"\nlanguages = [\"en\"]\nchecks = [\"syntax\", \"untranslated\"]\nsuggest-from = [\"en\"]\n",
        &["check"],
    );
    assert_exit(&output, 0, "Done in");
}

#[test]
fn code_aware_check_rejects_an_unknown_locale() {
    let output = run_with_config(
        "[tool.ftl-extract.check]\nlocales-path = \"locales\"\ncode-path = \"code\"\nlanguages = [\"xx\"]\nchecks = [\"missing\"]\n",
        &["check"],
    );
    assert_exit(&output, 2, "Locale `xx` does not exist");
}
