use crate::args::CheckReportFormat;
use crate::config::resolve_config_path;
use anyhow::{Context, Result};
use clap::ValueEnum;
use log::error;
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

pub(crate) fn exit_config_error(error: anyhow::Error) -> ! {
    error!(target: "cli", "Configuration error: {}", error);
    std::process::exit(2);
}
