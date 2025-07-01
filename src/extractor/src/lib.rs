pub mod ftl;

use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

#[cfg(test)]
mod tests {
    use super::ftl::consts;
    use super::ftl::ftl_extractor::extract;
    use crate::ftl::consts::CommentsKeyModes;
    use hashbrown::HashSet;
    use std::path::PathBuf;

    #[test]
    fn test_extract() {
        let start_time = std::time::Instant::now();
        let statistics = extract(
            &std::path::PathBuf::from(r"tests\files\py"),
            &std::path::PathBuf::from(r"tests\files\locales"),
            Vec::from(["en".to_string()]),
            consts::DEFAULT_I18N_KEYS.clone(),
            HashSet::from(["LF".to_string(), "cls_i18n".to_string()]),
            HashSet::from(["self".to_string(), "cls".to_string()]),
            consts::DEFAULT_EXCLUDE_DIRS.clone(),
            HashSet::from([r".\tests\files\py\should_be_excluded\*".to_string()]),
            consts::DEFAULT_IGNORE_ATTRIBUTES.clone(),
            HashSet::from(["core".to_string()]),
            HashSet::from(["when".to_string()]),
            true,
            &PathBuf::from(consts::DEFAULT_FTL_FILENAME),
            CommentsKeyModes::Comment,
            true,
        )
        .unwrap();

        println!("Extraction statistics:");
        println!("  - Py files count: {}", statistics.py_files_count);
        println!("  - FTL files count: {:?}", statistics.ftl_files_count);
        println!(
            "  - FTL keys in code: {:?}",
            statistics.ftl_in_code_keys_count
        );
        println!(
            "  - FTL keys stored: {:?}",
            statistics.ftl_stored_keys_count
        );
        println!("  - FTL keys updated: {:?}", statistics.ftl_keys_updated);
        println!("  - FTL keys added: {:?}", statistics.ftl_keys_added);
        println!(
            "  - FTL keys commented: {:?}",
            statistics.ftl_keys_commented
        );

        println!("Extracted fluent keys in {:?}", start_time.elapsed());
    }
}
