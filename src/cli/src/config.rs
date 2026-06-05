use anyhow::{Context, Result, bail};
use clap::ValueEnum;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ProjectConfig {
    pub tool: Option<ToolConfig>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ToolConfig {
    #[serde(rename = "ftl-extract")]
    pub ftl_extract: Option<FtlExtractConfig>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct FtlExtractConfig {
    pub extract: Option<ExtractPyprojectConfig>,
    pub stub: Option<StubPyprojectConfig>,
    pub check: Option<CheckPyprojectConfig>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ExtractPyprojectConfig {
    pub code_path: Option<PathBuf>,
    pub locales_path: Option<PathBuf>,
    pub languages: Option<Vec<String>>,
    pub i18n_keys: Option<Vec<String>>,
    pub i18n_keys_append: Option<Vec<String>>,
    pub i18n_keys_prefix: Option<Vec<String>>,
    pub exclude_dirs: Option<Vec<String>>,
    pub exclude_dirs_append: Option<Vec<String>>,
    pub ignore_attributes: Option<Vec<String>>,
    pub ignore_attributes_append: Option<Vec<String>>,
    pub ignore_kwargs: Option<Vec<String>>,
    pub comment_junks: Option<bool>,
    pub default_ftl_file: Option<PathBuf>,
    pub comment_keys_mode: Option<String>,
    pub line_endings: Option<String>,
    pub dry_run: Option<bool>,
    pub cache: Option<bool>,
    pub cache_path: Option<PathBuf>,
    pub clear_cache: Option<bool>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct StubPyprojectConfig {
    pub locales_path: Option<PathBuf>,
    pub stub_path: Option<PathBuf>,
    pub export_tree: Option<bool>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct CheckPyprojectConfig {
    pub locales_path: Option<PathBuf>,
    pub code_path: Option<PathBuf>,
    pub languages: Option<Vec<String>>,
    pub checks: Option<Vec<String>>,
    pub suggest_from: Option<Vec<String>>,
    pub fail_on: Option<Vec<String>>,
    pub report_path: Option<PathBuf>,
    pub report_format: Option<String>,
}

pub fn load_pyproject_config(path: Option<PathBuf>) -> Result<Option<LoadedProjectConfig>> {
    let Some(path) = path.or_else(find_pyproject) else {
        return Ok(None);
    };

    if !path.exists() {
        bail!("Config file `{}` does not exist", path.display());
    }

    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read config file `{}`", path.display()))?;
    let config: ProjectConfig = toml::from_str(&content)
        .with_context(|| format!("Failed to parse config file `{}`", path.display()))?;

    Ok(Some(LoadedProjectConfig {
        config: config
            .tool
            .and_then(|tool| tool.ftl_extract)
            .unwrap_or_default(),
        base_dir: path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from(".")),
    }))
}

#[derive(Debug, Clone)]
pub struct LoadedProjectConfig {
    pub config: FtlExtractConfig,
    pub base_dir: PathBuf,
}

pub fn resolve_config_path(path: Option<PathBuf>, base_dir: &Path) -> Option<PathBuf> {
    path.map(|path| {
        if path.is_relative() {
            base_dir.join(path)
        } else {
            path
        }
    })
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, ValueEnum)]
pub enum ConfigSampleCommand {
    Extract,
    Stub,
    Check,
}

pub fn render_config_sample(command: Option<ConfigSampleCommand>) -> &'static str {
    match command {
        Some(ConfigSampleCommand::Extract) => EXTRACT_SAMPLE,
        Some(ConfigSampleCommand::Stub) => STUB_SAMPLE,
        Some(ConfigSampleCommand::Check) => CHECK_SAMPLE,
        None => FULL_SAMPLE.as_str(),
    }
}

static FULL_SAMPLE: LazyLock<String> =
    LazyLock::new(|| [EXTRACT_SAMPLE, STUB_SAMPLE, CHECK_SAMPLE].join("\n"));

const EXTRACT_SAMPLE: &str = r#"[tool.ftl-extract.extract]
code-path = "app/bot"
locales-path = "app/bot/locales"
languages = ["en", "uk"]
i18n-keys-append = ["LF", "LazyProxy"]
ignore-attributes-append = ["core"]
exclude-dirs-append = ["./tests/*"]
ignore-kwargs = ["when"]
comment-junks = true
comment-keys-mode = "comment"
line-endings = "lf"
cache = true
"#;

const STUB_SAMPLE: &str = r#"[tool.ftl-extract.stub]
locales-path = "app/bot/locales/en"
stub-path = "app/bot/stub.pyi"
export-tree = false
"#;

const CHECK_SAMPLE: &str = r#"[tool.ftl-extract.check]
locales-path = "app/bot/locales"
code-path = "app/bot"
languages = ["uk", "pl"]
checks = ["all"]
suggest-from = ["en"]
fail-on = ["error"]
report-path = "reports/ftl-check"
report-format = "json"

# Check presets:
# checks = ["all"]
# checks = ["untranslated"] # Does not require code-path.
# checks = ["syntax"]       # Does not require code-path.
# checks = ["references"]   # Does not require code-path.
# checks = ["missing"]      # Requires code-path.
# checks = ["stale"]        # Requires code-path.
# checks = ["kwargs"]       # Requires code-path.
# checks = ["syntax", "references", "missing", "kwargs"]
"#;

fn find_pyproject() -> Option<PathBuf> {
    let mut current = std::env::current_dir().ok()?;

    loop {
        let candidate = current.join("pyproject.toml");
        if candidate.exists() {
            return Some(candidate);
        }

        if !current.pop() {
            return None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn load_explicit_pyproject_command_sections() {
        let temp = TempDir::new().unwrap();
        let pyproject = temp.path().join("pyproject.toml");
        std::fs::write(
            &pyproject,
            r#"
[tool.ftl-extract.extract]
code-path = "app"
locales-path = "locales"
languages = ["en", "uk"]
comment-keys-mode = "warn"

[tool.ftl-extract.stub]
locales-path = "locales/en"
stub-path = "app/stub.pyi"

[tool.ftl-extract.check]
locales-path = "locales"
code-path = "app"
checks = ["all"]
report-format = "json"
"#,
        )
        .unwrap();

        let loaded = load_pyproject_config(Some(pyproject)).unwrap().unwrap();

        assert_eq!(loaded.base_dir, temp.path());
        let extract = loaded.config.extract.unwrap();
        assert_eq!(extract.code_path, Some(PathBuf::from("app")));
        assert_eq!(extract.locales_path, Some(PathBuf::from("locales")));
        assert_eq!(
            extract.languages,
            Some(vec!["en".to_string(), "uk".to_string()])
        );
        assert_eq!(extract.comment_keys_mode, Some("warn".to_string()));
        assert_eq!(
            loaded.config.stub.unwrap().stub_path,
            Some(PathBuf::from("app/stub.pyi"))
        );
        let check = loaded.config.check.unwrap();
        assert_eq!(check.code_path, Some(PathBuf::from("app")));
        assert_eq!(check.report_format, Some("json".to_string()));
    }

    #[test]
    fn load_missing_pyproject_config_errors() {
        let temp = TempDir::new().unwrap();
        let missing = temp.path().join("missing-pyproject.toml");

        let error = load_pyproject_config(Some(missing)).unwrap_err();

        assert!(error.to_string().contains("does not exist"));
    }

    #[test]
    fn resolve_relative_config_paths_from_config_directory() {
        let base = Path::new("project");

        assert_eq!(
            resolve_config_path(Some(PathBuf::from("locales")), base),
            Some(PathBuf::from("project").join("locales"))
        );
        let absolute = std::env::current_dir().unwrap().join("locales");
        assert_eq!(
            resolve_config_path(Some(absolute.clone()), base),
            Some(absolute)
        );
    }

    #[test]
    fn render_full_config_sample_contains_all_command_sections() {
        let sample = render_config_sample(None);

        assert!(sample.contains("[tool.ftl-extract.extract]"));
        assert!(sample.contains("[tool.ftl-extract.stub]"));
        assert!(sample.contains("[tool.ftl-extract.check]"));
    }

    #[test]
    fn render_command_config_sample_contains_only_selected_section() {
        let sample = render_config_sample(Some(ConfigSampleCommand::Extract));

        assert!(sample.contains("[tool.ftl-extract.extract]"));
        assert!(!sample.contains("[tool.ftl-extract.stub]"));
        assert!(!sample.contains("[tool.ftl-extract.check]"));

        let sample = render_config_sample(Some(ConfigSampleCommand::Stub));

        assert!(!sample.contains("[tool.ftl-extract.extract]"));
        assert!(sample.contains("[tool.ftl-extract.stub]"));
        assert!(!sample.contains("[tool.ftl-extract.check]"));

        let sample = render_config_sample(Some(ConfigSampleCommand::Check));

        assert!(!sample.contains("[tool.ftl-extract.extract]"));
        assert!(!sample.contains("[tool.ftl-extract.stub]"));
        assert!(sample.contains("[tool.ftl-extract.check]"));
    }
}
