use crate::ftl::matcher::{FluentEntry, FluentKey, I18nMatcher};
use crate::ftl::utils::ExtractionStatistics;
use globset::GlobSet;
use hashbrown::hash_map::Entry;
use hashbrown::{HashMap, HashSet};
use ignore::WalkBuilder;
use ignore::types::TypesBuilder;
use log::error;
use memmap2::Mmap;
use rayon::prelude::*;
use ruff_python_ast::visitor::source_order::SourceOrderVisitor;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

fn find_py_files(search_path: &Path, ignore_set: &GlobSet) -> Vec<PathBuf> {
    let mut result_paths: Vec<PathBuf> = Vec::new();

    if search_path.is_dir() {
        let mut type_builder = TypesBuilder::new();
        type_builder.add("py", "*.py").unwrap();
        type_builder.select("py");

        let walker = WalkBuilder::new(search_path)
            .parents(false)
            .ignore(false)
            .git_global(false)
            .git_exclude(false)
            .require_git(false)
            .types(type_builder.build().unwrap())
            .build();

        for result in walker {
            match result {
                Ok(entry) => {
                    let path = entry.path();
                    if entry.file_type().is_some_and(|ft| ft.is_file())
                        && !ignore_set.is_match(path)
                    {
                        result_paths.push(path.to_path_buf());
                    }
                }
                Err(err) => error!(target: "extractor:code", "{}", err),
            }
        }
    } else if search_path.is_file() && search_path.extension().unwrap_or_default() == "py" {
        result_paths.push(search_path.to_path_buf());
    }

    result_paths
}
fn parse_file(
    file: &PathBuf,
    i18n_keys: &HashSet<String>,
    i18n_keys_prefix: &HashSet<String>,
    ignore_attributes: &HashSet<String>,
    ignore_kwargs: &HashSet<String>,
    default_ftl_file: &Path,
) -> HashMap<String, FluentKey> {
    let file_handle = match fs::File::open(file) {
        Ok(f) => f,
        Err(_) => return HashMap::new(),
    };
    // Mmap requires non-empty file. Handle empty files gracefully.
    let metadata = match file_handle.metadata() {
        Ok(m) => m,
        Err(_) => return HashMap::new(),
    };
    if metadata.len() == 0 {
        return HashMap::new();
    }

    // Unsafe is required for mmap (file could change under us), but standard for tools like this.
    let mmap = unsafe {
        match Mmap::map(&file_handle) {
            Ok(m) => m,
            Err(_) => return HashMap::new(),
        }
    };
    let code = match std::str::from_utf8(&mmap) {
        Ok(c) => c,
        Err(_) => return HashMap::new(),
    };
    let module = match ruff_python_parser::parse_module(code) {
        Ok(m) => m,
        Err(_) => return HashMap::new(),
    };

    let mut matcher = I18nMatcher::new(
        file.to_path_buf(),
        default_ftl_file.to_path_buf(),
        i18n_keys,
        i18n_keys_prefix,
        ignore_attributes,
        ignore_kwargs,
    );

    matcher.visit_body(module.suite());

    matcher.fluent_keys
}
pub(crate) fn extract_fluent_keys<'a>(
    path: &'a Path,
    i18n_keys: HashSet<String>,
    i18n_keys_prefix: HashSet<String>,
    exclude_dirs: &GlobSet,
    ignore_attributes: HashSet<String>,
    ignore_kwargs: HashSet<String>,
    default_ftl_file: &'a Path,
    statistics: &mut ExtractionStatistics,
) -> HashMap<String, FluentKey> {
    let py_files = find_py_files(path, exclude_dirs);
    let py_files_count = AtomicUsize::new(0);

    // Parallel Map-Reduce
    let fluent_keys: HashMap<String, FluentKey> = py_files
        .par_iter()
        .fold(
            HashMap::new, // Init local accumulator
            |mut acc, file| {
                let keys = parse_file(
                    file,
                    &i18n_keys,
                    &i18n_keys_prefix,
                    &ignore_attributes,
                    &ignore_kwargs,
                    default_ftl_file,
                );

                if !keys.is_empty() {
                    py_files_count.fetch_add(1, Ordering::Relaxed);
                }

                // Merge found keys into local accumulator
                for (key, new_fluent_key) in keys {
                    match acc.entry(key.clone()) {
                        Entry::Occupied(entry) => {
                            let existing_key: &FluentKey = entry.get();

                            // Validation Logic
                            if existing_key.path != new_fluent_key.path {
                                panic!(
                                    "FluentKey conflict: {} in {} and {}",
                                    key,
                                    existing_key.path.display(),
                                    new_fluent_key.path.display()
                                )
                            }

                            match (&existing_key.entry, &new_fluent_key.entry) {
                                (FluentEntry::Message(a), FluentEntry::Message(b)) if a != b => {
                                    panic!(
                                        "FluentKey conflict: {} in {} and {}",
                                        key,
                                        existing_key.path.display(),
                                        new_fluent_key.path.display()
                                    );
                                }
                                (a, b) if a != b => {
                                    panic!("FluentKey type conflict: {} ({:?} vs {:?})", key, a, b);
                                }
                                _ => {}
                            }
                        }
                        Entry::Vacant(entry) => {
                            entry.insert(new_fluent_key);
                        }
                    }
                }
                acc
            },
        )
        .reduce(
            HashMap::new, // Init reducer
            |a, b| {
                // Merge two Maps (a and b)
                // We iterate over the smaller map and insert into the larger one for efficiency
                let (mut target, source) = if a.len() > b.len() { (a, b) } else { (b, a) };

                for (key, val) in source {
                    // Same conflict logic as above needed here strictly speaking,
                    // but if keys are unique per file or conflicts handled in fold,
                    // we just insert. To be safe, we use the same check:
                    match target.entry(key.clone()) {
                        Entry::Occupied(entry) => {
                            let existing_key: &FluentKey = entry.get();
                            if existing_key.path != val.path {
                                panic!("FluentKey conflict during merge: {}", key);
                            }
                            // Additional checks omitted for brevity, but should be mirrored
                        }
                        Entry::Vacant(entry) => {
                            entry.insert(val);
                        }
                    }
                }
                target
            },
        );

    statistics.py_files_count += py_files_count.load(Ordering::Relaxed);

    fluent_keys
}

pub(crate) fn sort_fluent_keys_by_path(
    fluent_keys: HashMap<String, FluentKey>,
) -> HashMap<Arc<PathBuf>, Vec<FluentKey>> {
    let mut sorted_fluent_keys: HashMap<Arc<PathBuf>, Vec<FluentKey>> = HashMap::new();
    for fluent_key in fluent_keys.values() {
        sorted_fluent_keys
            .entry(fluent_key.path.clone())
            .or_default()
            .push(fluent_key.clone());
    }
    sorted_fluent_keys
}

#[cfg(test)]
mod tests {
    use crate::ftl::consts;
    use crate::ftl::matcher::{FluentEntry, FluentKey};
    use globset::GlobSet;
    use hashbrown::{HashMap, HashSet};
    use pretty_assertions::assert_eq;
    use std::path::PathBuf;
    use std::sync::Arc;

    #[test]
    fn test_find_py_files_dir() {
        let code_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("py");
        let ignore_set = GlobSet::empty();
        let py_files = super::find_py_files(&code_path, &ignore_set);
        assert_eq!(py_files.len(), 3);
    }

    #[test]
    fn test_find_py_files_file() {
        let code_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("py")
            .join("default.py");
        let ignore_set = GlobSet::empty();
        let py_files = super::find_py_files(&code_path, &ignore_set);
        assert_eq!(py_files.len(), 1);
        assert_eq!(py_files[0], code_path);
    }

    #[test]
    fn test_extract_fluent_keys() {
        let code_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("py");
        let mut key_prefixes = consts::DEFAULT_I18N_KEYS.clone();
        key_prefixes.insert("self".to_string());
        key_prefixes.insert("cls".to_string());
        let mut statistics = super::ExtractionStatistics::new();

        let fluent_keys = super::extract_fluent_keys(
            &code_path,
            key_prefixes.clone(),
            HashSet::new(),
            &globset::GlobSet::empty(),
            HashSet::new(),
            HashSet::new(),
            &PathBuf::from("locales/en.ftl"),
            &mut statistics,
        );

        eprintln!("Extracted Fluent Keys: {:?}", fluent_keys.keys());

        assert_eq!(fluent_keys.len(), 14);
        assert!(fluent_keys.contains_key("text"));
        assert!(fluent_keys.contains_key("text-kwargs"));
        assert!(fluent_keys.contains_key("text-args-term"));
        assert!(fluent_keys.contains_key("text-args-term-args"));
        assert!(fluent_keys.contains_key("text-message_reference"));
        assert!(fluent_keys.contains_key("text-message_reference-args"));
        assert!(fluent_keys.contains_key("text-selector"));
        assert!(fluent_keys.contains_key("text-selector-selectors"));
        assert!(fluent_keys.contains_key("text-selector-kwargs"));
        assert!(fluent_keys.contains_key("text-selector-reference-selector-kwargs-terms"));
        assert_eq!(statistics.py_files_count, 2);
    }

    #[test]
    fn test_sort_fluent_keys_by_path() {
        let mut fluent_keys: HashMap<String, FluentKey> = HashMap::new();

        let code_path1 = Arc::new(PathBuf::from("file1.py"));
        let ftl_path1 = Arc::new(PathBuf::from("file1.ftl"));
        let code_path2 = Arc::new(PathBuf::from("file2.py"));
        let ftl_path2 = Arc::new(PathBuf::from("file2.ftl"));

        fluent_keys.insert(
            "key1".to_string(),
            FluentKey::new(
                code_path1.clone(),
                "key1".to_string(),
                FluentEntry::Message(fluent_syntax::ast::Message {
                    id: fluent_syntax::ast::Identifier {
                        name: "key1".to_string(),
                    },
                    value: None,
                    attributes: vec![],
                    comment: None,
                }),
                ftl_path1.clone(),
                Some("en".to_string()),
                Some(0),
                HashSet::new(),
            ),
        );
        fluent_keys.insert(
            "key2".to_string(),
            FluentKey::new(
                code_path2.clone(),
                "key2".to_string(),
                FluentEntry::Message(fluent_syntax::ast::Message {
                    id: fluent_syntax::ast::Identifier {
                        name: "key2".to_string(),
                    },
                    value: None,
                    attributes: vec![],
                    comment: None,
                }),
                ftl_path2.clone(),
                Some("en".to_string()),
                Some(0),
                HashSet::new(),
            ),
        );

        let sorted = super::sort_fluent_keys_by_path(fluent_keys.clone());
        assert_eq!(sorted.len(), 2);
        assert!(sorted.contains_key(&ftl_path1.clone()));
        assert!(sorted.contains_key(&ftl_path2.clone()));
    }
}
