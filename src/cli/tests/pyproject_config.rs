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
    assert!(stdout.contains("[tool.ftl-extract.untranslated]"));
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
    assert!(!stdout.contains("[tool.ftl-extract.untranslated]"));
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
output-path = "locales"
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
output-path = "locales"
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
ftl-path = "locales/en"
output-path = "code/stub.pyi"
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
fn untranslated_reads_command_config_from_pyproject() {
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
[tool.ftl-extract.untranslated]
locales-path = "locales"
languages = ["uk"]
suggest-from = ["en"]
output = "reports/untranslated"
output-format = "json"
"#,
    );

    let output = ftl()
        .arg("--config")
        .arg(pyproject(&temp))
        .arg("untranslated")
        .output()
        .unwrap();

    assert_success(&output);
    let report = std::fs::read_to_string(temp.path().join("reports/untranslated.json")).unwrap();
    assert!(report.contains(r#""locale": "uk""#));
    assert!(report.contains(r#""key": "hello""#));
    assert!(report.contains(r#""locale": "en""#));
}
