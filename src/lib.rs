mod ftl;

use mimalloc::MiMalloc;
use pyo3::prelude::*;
use std::collections::HashSet;
use std::path::PathBuf;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

#[pyfunction]
fn fast_extract(
    code_path: PathBuf,
    output_path: PathBuf,
    language: Vec<String>,
    i18n_keys: HashSet<String>,
    i18n_keys_append: HashSet<String>,
    i18n_keys_prefix: HashSet<String>,
    exclude_dirs: HashSet<String>,
    exclude_dirs_append: HashSet<String>,
    ignore_attributes: HashSet<String>,
    append_ignore_attributes: HashSet<String>,
    ignore_kwargs: HashSet<String>,
    comment_junks: bool,
    default_ftl_file: PathBuf,
    comment_keys_mode: String,
    dry_run: bool,
) {
    let _ = ftl::ftl_extractor::extraxt(
        code_path.as_path(),
        output_path.as_path(),
        language,
        i18n_keys,
        i18n_keys_append,
        i18n_keys_prefix,
        exclude_dirs,
        exclude_dirs_append,
        ignore_attributes,
        append_ignore_attributes,
        ignore_kwargs,
        comment_junks,
        default_ftl_file.as_path(),
        comment_keys_mode,
        dry_run,
    );
}

#[pymodule]
fn ftl_extract(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(fast_extract, m)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::ftl::consts;
    use super::ftl::ftl_extractor::extraxt;
    use std::collections::HashSet;
    use std::path::PathBuf;

    #[test]
    fn test_extract() {
        let start_time = std::time::Instant::now();
        extraxt(
            &std::path::PathBuf::from(r"tests\files\bot"),
            &std::path::PathBuf::from(r"tests\files\bot\locales"),
            Vec::from(["en".to_string()]),
            consts::DEFAULT_I18N_KEYS.clone(),
            HashSet::from(["LF".to_string(), "cls_i18n".to_string()]),
            HashSet::from(["self".to_string(), "cls".to_string()]),
            consts::DEFAULT_EXCLUDE_DIRS.clone(),
            HashSet::from([]),
            consts::DEFAULT_IGNORE_ATTRIBUTES.clone(),
            HashSet::from(["core".to_string()]),
            HashSet::from(["when".to_string()]),
            true,
            &PathBuf::from(consts::DEFAULT_FTL_FILENAME),
            "warn".to_string(),
            true,
        )
        .unwrap();
        println!("Extracted fluent keys in {:?}", start_time.elapsed());
    }
}
