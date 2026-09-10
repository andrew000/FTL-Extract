use crate::args::CheckReportFormat;
use crate::config::resolve_config_path;
use anyhow::{Context, Result, bail};
use check::{DiagnosticKind, Severity, SeverityOverrides};
use clap::ValueEnum;
use log::error;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub(crate) fn write_output_file(path: &Path, content: String) -> std::io::Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, content)
}

pub(crate) fn normalize_output_path(path: PathBuf, format: &CheckReportFormat) -> PathBuf {
    if path.extension().is_some() {
        return path;
    }

    let suffix = match format {
        CheckReportFormat::Terminal => "txt",
        CheckReportFormat::Json => "json",
    };

    path.with_extension(suffix)
}

pub(crate) fn cli_or_config_vec<T>(cli: Vec<T>, config: Option<Vec<T>>, default: Vec<T>) -> Vec<T> {
    if !cli.is_empty() {
        cli
    } else if let Some(config) = config {
        config
    } else {
        default
    }
}

pub(crate) fn cli_or_config_path(
    cli: Option<PathBuf>,
    config: Option<PathBuf>,
    base_dir: &Path,
) -> Option<PathBuf> {
    cli.or_else(|| resolve_config_path(config, base_dir))
}

pub(crate) fn resolve_required_path(
    cli: Option<PathBuf>,
    config: Option<PathBuf>,
    base_dir: &Path,
    error: &'static str,
) -> Result<PathBuf> {
    cli_or_config_path(cli, config, base_dir).context(error)
}

pub(crate) fn cli_or_config_enum<T>(
    cli: Option<T>,
    config: Option<String>,
    field: &str,
) -> Result<Option<T>>
where
    T: ValueEnum,
{
    if cli.is_some() {
        return Ok(cli);
    }

    let Some(config) = config else {
        return Ok(None);
    };

    T::from_str(&config, true).map(Some).map_err(|_| {
        let values = T::value_variants()
            .iter()
            .filter_map(|variant| variant.to_possible_value())
            .map(|value| value.get_name().to_string())
            .collect::<Vec<_>>()
            .join(", ");
        anyhow::anyhow!("Invalid `{field}` value `{config}`. Expected one of: {values}")
    })
}

pub(crate) fn cli_or_config_enum_vec<T>(
    cli: Vec<T>,
    config: Option<Vec<String>>,
    field: &str,
    default: Vec<T>,
) -> Result<Vec<T>>
where
    T: ValueEnum,
{
    if !cli.is_empty() {
        return Ok(cli);
    }

    let Some(config) = config else {
        return Ok(default);
    };

    config
        .into_iter()
        .map(|value| {
            T::from_str(&value, true).map_err(|_| {
                let values = T::value_variants()
                    .iter()
                    .filter_map(|variant| variant.to_possible_value())
                    .map(|value| value.get_name().to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                anyhow::anyhow!("Invalid `{field}` value `{value}`. Expected one of: {values}")
            })
        })
        .collect()
}

/// Merges per-check severity overrides from `pyproject.toml` (`severity = { stale = "error" }`)
/// and the CLI (`--severity stale=error`). CLI values win over config values.
pub(crate) fn severity_overrides(
    cli: &[String],
    config: Option<BTreeMap<String, String>>,
) -> Result<SeverityOverrides> {
    let mut overrides = SeverityOverrides::default();

    for (kind, severity) in config.into_iter().flatten() {
        overrides.set(
            parse_severity_kind(&kind)?,
            parse_severity_level(&severity)?,
        );
    }
    for entry in cli {
        let Some((kind, severity)) = entry.split_once('=') else {
            bail!(
                "Invalid `--severity` value `{entry}`. Expected `<check>=<severity>`, for example `stale=error`"
            );
        };
        overrides.set(parse_severity_kind(kind)?, parse_severity_level(severity)?);
    }

    Ok(overrides)
}

fn parse_severity_kind(value: &str) -> Result<DiagnosticKind> {
    DiagnosticKind::parse(value).with_context(|| {
        let kinds = DiagnosticKind::ALL
            .iter()
            .map(|kind| kind.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        format!("Invalid `severity` check `{value}`. Expected one of: {kinds}")
    })
}

fn parse_severity_level(value: &str) -> Result<Severity> {
    Severity::parse(value).with_context(|| {
        let levels = Severity::ALL
            .iter()
            .map(|severity| severity.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        format!("Invalid `severity` level `{value}`. Expected one of: {levels}")
    })
}

pub(crate) fn exit_config_error(error: anyhow::Error) -> ! {
    error!(target: "cli", "Configuration error: {}", error);
    std::process::exit(2);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::args::FailSeverity;
    use tempfile::TempDir;

    #[test]
    fn write_output_file_creates_parent_directories() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("reports").join("ftl-check.json");

        write_output_file(&path, "report".to_string()).unwrap();

        assert_eq!(std::fs::read_to_string(path).unwrap(), "report");
    }

    #[test]
    fn normalize_output_path_adds_extension_for_selected_format() {
        assert_eq!(
            normalize_output_path(PathBuf::from("report"), &CheckReportFormat::Json),
            PathBuf::from("report.json")
        );
        assert_eq!(
            normalize_output_path(PathBuf::from("report"), &CheckReportFormat::Terminal),
            PathBuf::from("report.txt")
        );
    }

    #[test]
    fn normalize_output_path_preserves_existing_extension() {
        assert_eq!(
            normalize_output_path(PathBuf::from("report.out"), &CheckReportFormat::Json),
            PathBuf::from("report.out")
        );
    }

    #[test]
    fn cli_or_config_vec_prefers_cli_then_config_then_default() {
        assert_eq!(
            cli_or_config_vec(vec!["cli"], Some(vec!["config"]), vec!["default"]),
            vec!["cli"]
        );
        assert_eq!(
            cli_or_config_vec(Vec::<&str>::new(), Some(vec!["config"]), vec!["default"]),
            vec!["config"]
        );
        assert_eq!(
            cli_or_config_vec(Vec::<&str>::new(), None, vec!["default"]),
            vec!["default"]
        );
    }

    #[test]
    fn cli_or_config_path_resolves_config_relative_to_base() {
        let base = Path::new("project");

        assert_eq!(
            cli_or_config_path(
                Some(PathBuf::from("cli")),
                Some(PathBuf::from("config")),
                base
            ),
            Some(PathBuf::from("cli"))
        );
        assert_eq!(
            cli_or_config_path(None, Some(PathBuf::from("config")), base),
            Some(PathBuf::from("project").join("config"))
        );
    }

    #[test]
    fn resolve_required_path_reports_missing_path() {
        let error = resolve_required_path(None, None, Path::new("."), "missing path").unwrap_err();

        assert_eq!(error.to_string(), "missing path");
    }

    #[test]
    fn cli_or_config_enum_parses_config_and_reports_invalid_values() {
        assert_eq!(
            cli_or_config_enum::<CheckReportFormat>(
                None,
                Some("terminal".to_string()),
                "report-format"
            )
            .unwrap(),
            Some(CheckReportFormat::Terminal)
        );

        let error =
            cli_or_config_enum::<CheckReportFormat>(None, Some("xml".to_string()), "report-format")
                .unwrap_err();
        assert!(error.to_string().contains("Invalid `report-format` value"));
    }

    #[test]
    fn cli_or_config_enum_vec_prefers_cli_and_reports_invalid_values() {
        assert_eq!(
            cli_or_config_enum_vec(
                vec![FailSeverity::Warn],
                Some(vec!["error".to_string()]),
                "fail-on",
                vec![FailSeverity::Error]
            )
            .unwrap(),
            vec![FailSeverity::Warn]
        );
        assert_eq!(
            cli_or_config_enum_vec::<FailSeverity>(
                Vec::new(),
                None,
                "fail-on",
                vec![FailSeverity::Error]
            )
            .unwrap(),
            vec![FailSeverity::Error]
        );

        let error = cli_or_config_enum_vec::<FailSeverity>(
            Vec::new(),
            Some(vec!["fatal".to_string()]),
            "fail-on",
            Vec::new(),
        )
        .unwrap_err();
        assert!(error.to_string().contains("Invalid `fail-on` value"));
    }

    #[test]
    fn severity_overrides_merge_config_and_cli_with_cli_winning() {
        let config = BTreeMap::from([
            ("stale".to_string(), "error".to_string()),
            ("missing".to_string(), "warn".to_string()),
        ]);

        let overrides = severity_overrides(
            &["stale=warn".to_string(), "Syntax=WARN".to_string()],
            Some(config),
        )
        .unwrap();

        assert_eq!(overrides.get(DiagnosticKind::Stale), Some(Severity::Warn));
        assert_eq!(overrides.get(DiagnosticKind::Missing), Some(Severity::Warn));
        assert_eq!(overrides.get(DiagnosticKind::Syntax), Some(Severity::Warn));
        assert_eq!(overrides.get(DiagnosticKind::Kwargs), None);
        assert_eq!(
            overrides.severity_for(DiagnosticKind::Kwargs),
            Severity::Error
        );
    }

    #[test]
    fn severity_overrides_report_invalid_values() {
        let error = severity_overrides(&["stale".to_string()], None).unwrap_err();
        assert!(error.to_string().contains("Expected `<check>=<severity>`"));

        let error = severity_overrides(&["bogus=error".to_string()], None).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("Invalid `severity` check `bogus`")
        );

        let error = severity_overrides(&["stale=fatal".to_string()], None).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("Invalid `severity` level `fatal`")
        );

        let config = BTreeMap::from([("stale".to_string(), "loud".to_string())]);
        let error = severity_overrides(&[], Some(config)).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("Invalid `severity` level `loud`")
        );

        assert!(severity_overrides(&[], None).unwrap().is_empty());
    }
}
