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
    pub untranslated: Option<UntranslatedPyprojectConfig>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ExtractPyprojectConfig {
    pub code_path: Option<PathBuf>,
    pub output_path: Option<PathBuf>,
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
    pub ftl_path: Option<PathBuf>,
    pub output_path: Option<PathBuf>,
    pub export_tree: Option<bool>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct UntranslatedPyprojectConfig {
    pub locales_path: Option<PathBuf>,
    pub languages: Option<Vec<String>>,
    pub suggest_from: Option<Vec<String>>,
    pub fail_on_untranslated: Option<bool>,
    pub output: Option<PathBuf>,
    pub output_format: Option<String>,
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
    Untranslated,
}

pub fn render_config_sample(command: Option<ConfigSampleCommand>) -> &'static str {
    match command {
        Some(ConfigSampleCommand::Extract) => EXTRACT_SAMPLE,
        Some(ConfigSampleCommand::Stub) => STUB_SAMPLE,
        Some(ConfigSampleCommand::Untranslated) => UNTRANSLATED_SAMPLE,
        None => FULL_SAMPLE.as_str(),
    }
}

static FULL_SAMPLE: LazyLock<String> =
    LazyLock::new(|| [EXTRACT_SAMPLE, STUB_SAMPLE, UNTRANSLATED_SAMPLE].join("\n"));

const EXTRACT_SAMPLE: &str = r#"[tool.ftl-extract.extract]
code-path = "app/bot"
output-path = "app/bot/locales"
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
ftl-path = "app/bot/locales/en"
output-path = "app/bot/stub.pyi"
export-tree = false
"#;

const UNTRANSLATED_SAMPLE: &str = r#"[tool.ftl-extract.untranslated]
locales-path = "app/bot/locales"
languages = ["uk"]
suggest-from = ["en"]
fail-on-untranslated = true
output = "reports/untranslated"
output-format = "json"
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
output-path = "locales"
languages = ["en", "uk"]
comment-keys-mode = "warn"

[tool.ftl-extract.stub]
ftl-path = "locales/en"
output-path = "app/stub.pyi"

[tool.ftl-extract.untranslated]
locales-path = "locales"
output-format = "json"
"#,
        )
        .unwrap();

        let loaded = load_pyproject_config(Some(pyproject)).unwrap().unwrap();

        assert_eq!(loaded.base_dir, temp.path());
        let extract = loaded.config.extract.unwrap();
        assert_eq!(extract.code_path, Some(PathBuf::from("app")));
        assert_eq!(extract.output_path, Some(PathBuf::from("locales")));
        assert_eq!(
            extract.languages,
            Some(vec!["en".to_string(), "uk".to_string()])
        );
        assert_eq!(extract.comment_keys_mode, Some("warn".to_string()));
        assert_eq!(
            loaded.config.stub.unwrap().output_path,
            Some(PathBuf::from("app/stub.pyi"))
        );
        assert_eq!(
            loaded.config.untranslated.unwrap().output_format,
            Some("json".to_string())
        );
    }

    #[test]
    fn resolve_relative_config_paths_from_config_directory() {
        let base = Path::new("project");

        assert_eq!(
            resolve_config_path(Some(PathBuf::from("locales")), base),
            Some(PathBuf::from("project").join("locales"))
        );
    }

    #[test]
    fn render_full_config_sample_contains_all_command_sections() {
        let sample = render_config_sample(None);

        assert!(sample.contains("[tool.ftl-extract.extract]"));
        assert!(sample.contains("[tool.ftl-extract.stub]"));
        assert!(sample.contains("[tool.ftl-extract.untranslated]"));
    }

    #[test]
    fn render_command_config_sample_contains_only_selected_section() {
        let sample = render_config_sample(Some(ConfigSampleCommand::Extract));

        assert!(sample.contains("[tool.ftl-extract.extract]"));
        assert!(!sample.contains("[tool.ftl-extract.stub]"));
        assert!(!sample.contains("[tool.ftl-extract.untranslated]"));
    }
}
