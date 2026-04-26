use crate::ftl::cache::{
    CacheFile, CacheUpdate, cache_file_path, cache_options, cached_file_to_keys, file_cache_key,
    file_modified_ns, keys_to_cached_file, load_cache, save_cache,
};
use crate::ftl::matcher::{FluentEntry, FluentKey, I18nMatcher};
use crate::ftl::utils::{ExtractionStatistics, FastHashMap, FastHashSet};
use globset::GlobSet;
use ignore::WalkBuilder;
use ignore::types::TypesBuilder;
use log::error;
use memchr::memmem;
use memmap2::Mmap;
use rayon::prelude::*;
use ruff_python_ast::visitor::source_order::SourceOrderVisitor;
use std::collections::hash_map::Entry;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Clone, Debug)]
struct PyFile {
    path: PathBuf,
    size: u64,
    modified_ns: u128,
}

fn find_py_files(search_path: &Path, ignore_set: &GlobSet) -> Vec<PyFile> {
    let mut result_paths: Vec<PyFile> = Vec::new();

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
                        && let Ok(metadata) = entry.metadata()
                    {
                        result_paths.push(PyFile {
                            path: path.to_path_buf(),
                            size: metadata.len(),
                            modified_ns: file_modified_ns(&metadata),
                        });
                    }
                }
                Err(err) => error!(target: "extractor:code", "{}", err),
            }
        }
    } else if search_path.is_file()
        && search_path.extension().unwrap_or_default() == "py"
        && let Ok(metadata) = fs::metadata(search_path)
    {
        result_paths.push(PyFile {
            path: search_path.to_path_buf(),
            size: metadata.len(),
            modified_ns: file_modified_ns(&metadata),
        });
    }

    result_paths
}
fn parse_file(
    file: &Path,
    file_size: u64,
    i18n_keys: &FastHashSet<String>,
    i18n_keys_prefix: &FastHashSet<String>,
    ignore_attributes: &FastHashSet<String>,
    ignore_kwargs: &FastHashSet<String>,
    default_ftl_file: &Path,
) -> FastHashMap<String, FluentKey> {
    if file_size == 0 {
        return FastHashMap::default();
    }

    let file_handle = match fs::File::open(file) {
        Ok(f) => f,
        Err(_) => return FastHashMap::default(),
    };

    // Unsafe is required for mmap (file could change under us), but standard for tools like this.
    let mmap = unsafe {
        match Mmap::map(&file_handle) {
            Ok(m) => m,
            Err(_) => return FastHashMap::default(),
        }
    };

    // Quick check: does the file contain any of the i18n keys or prefixes?
    let has_key = i18n_keys
        .iter()
        .chain(i18n_keys_prefix.iter())
        .any(|key| memmem::find(&mmap, key.as_bytes()).is_some());

    if !has_key {
        return FastHashMap::default();
    }

    let code = match std::str::from_utf8(&mmap) {
        Ok(c) => c,
        Err(_) => {
            error!(target: "extractor:code", "Bad UTF-8 in {}", file.display());
            return FastHashMap::default();
        }
    };
    let module = match ruff_python_parser::parse_module(code) {
        Ok(m) => m,
        Err(_) => return FastHashMap::default(),
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

fn extract_from_file(
    file: &PyFile,
    i18n_keys: &FastHashSet<String>,
    i18n_keys_prefix: &FastHashSet<String>,
    ignore_attributes: &FastHashSet<String>,
    ignore_kwargs: &FastHashSet<String>,
    default_ftl_file: &Path,
    cache: Option<&CacheFile>,
) -> (FastHashMap<String, FluentKey>, Option<CacheUpdate>) {
    let cache_key = file_cache_key(&file.path);
    let cached_file = cache.and_then(|cache| cache.files.get(&cache_key));

    if let Some(cached) = cached_file
        .filter(|cached| cached.size == file.size && cached.modified_ns == file.modified_ns)
    {
        return (cached_file_to_keys(cached), None);
    }

    let keys = parse_file(
        &file.path,
        file.size,
        i18n_keys,
        i18n_keys_prefix,
        ignore_attributes,
        ignore_kwargs,
        default_ftl_file,
    );

    if keys.is_empty() {
        let update = cached_file
            .is_some()
            .then_some(CacheUpdate::Remove(cache_key));
        return (keys, update);
    }

    let cached_file = keys_to_cached_file(file.size, file.modified_ns, &keys);

    (keys, Some(CacheUpdate::Upsert(cache_key, cached_file)))
}

fn merge_fluent_keys(target: &mut FastHashMap<String, FluentKey>, key: String, val: FluentKey) {
    match target.entry(key) {
        Entry::Occupied(entry) => {
            let existing_key: &FluentKey = entry.get();
            if existing_key.path != val.path {
                panic!("FluentKey conflict during merge: {}", entry.key());
            }
        }
        Entry::Vacant(entry) => {
            entry.insert(val);
        }
    }
}

pub(crate) fn extract_fluent_keys<'a>(
    path: &'a Path,
    i18n_keys: FastHashSet<String>,
    i18n_keys_prefix: FastHashSet<String>,
    exclude_dirs: &GlobSet,
    ignore_attributes: FastHashSet<String>,
    ignore_kwargs: FastHashSet<String>,
    default_ftl_file: &'a Path,
    use_cache: bool,
    cache_path: Option<&Path>,
    clear_cache: bool,
    statistics: &mut ExtractionStatistics,
) -> FastHashMap<String, FluentKey> {
    let py_files = find_py_files(path, exclude_dirs);
    let py_files_count = AtomicUsize::new(0);
    let options = cache_options(
        &i18n_keys,
        &i18n_keys_prefix,
        &ignore_attributes,
        &ignore_kwargs,
        default_ftl_file,
    );
    let cache_file_path = cache_file_path(cache_path);
    let cache = use_cache.then(|| load_cache(&cache_file_path, &options, clear_cache));

    // Parallel Map-Reduce
    let (fluent_keys, cache_updates): (FastHashMap<String, FluentKey>, Vec<CacheUpdate>) = py_files
        .par_iter()
        .fold(
            || (FastHashMap::default(), Vec::new()),
            |(mut acc, mut updates), file| {
                let (keys, cache_update) = extract_from_file(
                    file,
                    &i18n_keys,
                    &i18n_keys_prefix,
                    &ignore_attributes,
                    &ignore_kwargs,
                    default_ftl_file,
                    cache.as_ref(),
                );

                if !keys.is_empty() {
                    py_files_count.fetch_add(1, Ordering::Relaxed);
                }

                if let Some(update) = cache_update {
                    updates.push(update);
                }

                // Merge found keys into local accumulator
                for (key, new_fluent_key) in keys {
                    match acc.entry(key) {
                        Entry::Occupied(entry) => {
                            let existing_key: &FluentKey = entry.get();

                            // Validation Logic
                            if existing_key.path != new_fluent_key.path {
                                panic!(
                                    "FluentKey conflict: {} in {} and {}",
                                    entry.key(),
                                    existing_key.path.display(),
                                    new_fluent_key.path.display()
                                )
                            }

                            match (&existing_key.entry.as_ref(), &new_fluent_key.entry.as_ref()) {
                                (FluentEntry::Message(a), FluentEntry::Message(b)) if a != b => {
                                    panic!(
                                        "FluentKey conflict: {} in {} and {}",
                                        entry.key(),
                                        existing_key.path.display(),
                                        new_fluent_key.path.display()
                                    );
                                }
                                (a, b) if a != b => {
                                    panic!(
                                        "FluentKey type conflict: {} ({:?} vs {:?})",
                                        entry.key(),
                                        a,
                                        b
                                    );
                                }
                                _ => {}
                            }
                        }
                        Entry::Vacant(entry) => {
                            entry.insert(new_fluent_key);
                        }
                    }
                }
                (acc, updates)
            },
        )
        .reduce(
            || (FastHashMap::default(), Vec::new()),
            |a, b| {
                // Merge two Maps (a and b)
                // We iterate over the smaller map and insert into the larger one for efficiency
                let ((mut target, mut target_updates), (source, source_updates)) =
                    if a.0.len() > b.0.len() {
                        (a, b)
                    } else {
                        (b, a)
                    };

                for (key, val) in source {
                    // Same conflict logic as above needed here strictly speaking,
                    // but if keys are unique per file or conflicts handled in fold,
                    // we just insert. To be safe, we use the same check:
                    merge_fluent_keys(&mut target, key, val);
                }
                target_updates.extend(source_updates);
                (target, target_updates)
            },
        );

    statistics.py_files_count += py_files_count.load(Ordering::Relaxed);

    if let Some(mut cache) = cache
        && !cache_updates.is_empty()
    {
        for update in cache_updates {
            match update {
                CacheUpdate::Upsert(path, cached_file) => {
                    cache.files.insert(path, cached_file);
                }
                CacheUpdate::Remove(path) => {
                    cache.files.remove(&path);
                }
            }
        }
        save_cache(&cache_file_path, &cache);
    }

    fluent_keys
}

pub(crate) fn sort_fluent_keys_by_path(
    fluent_keys: FastHashMap<String, FluentKey>,
) -> FastHashMap<Arc<PathBuf>, Vec<FluentKey>> {
    if fluent_keys.is_empty() {
        return FastHashMap::default();
    }

    let mut sorted_fluent_keys: FastHashMap<Arc<PathBuf>, Vec<FluentKey>> = FastHashMap::default();

    for fluent_key in fluent_keys.into_values() {
        sorted_fluent_keys
            .entry(fluent_key.path.clone())
            .or_default()
            .push(fluent_key);
    }

    sorted_fluent_keys
}

#[cfg(test)]
mod tests {
    use crate::ftl::consts;
    use crate::ftl::matcher::{FluentEntry, FluentKey};
    use crate::ftl::utils::{FastHashMap, FastHashSet};
    use globset::GlobSet;
    use pretty_assertions::assert_eq;
    use std::path::PathBuf;
    use std::sync::Arc;
    use tempfile::TempDir;

    const DEFAULT_PY: &str = r#"
from .stub import I18nContext

i18n = I18nContext()

i18n.text()
i18n.text.kwargs(kwarg1="value1", kwarg2="value2")
i18n.text.args.term()
i18n.text.args.term.args(kwarg1="value1", kwarg2="value2")
i18n.text.message_reference()
i18n.text.message_reference.args(kwarg1="value1", kwarg2="value2")
i18n.text.selector(selector=1)
i18n.text.selector.selectors(selector=1)
i18n.text.selector.kwargs(selector=1, kwarg1="value1", kwarg2="value2")
i18n.text.selector.reference.selector.kwargs.terms(
    selector=1,
    kwarg1="value1",
    kwarg2="value2",
)
"#;

    const CLASSLIKE_PY: &str = r#"
from typing import Any


class I18nContext:
    def get(self, *_, **__) -> None: ...

    def __getattr__(self, item: str) -> Any: ...

    def __call__(self, *_, **__) -> None: ...


class Mock:
    cls_i18n: I18nContext

    def __init__(self, i18n: I18nContext) -> None:
        self.i18n = i18n

    def self_i18n(self) -> None:
        self.i18n.self.key(some_kwarg="...", _path="classlike.ftl")
        self.i18n.get("self-get-key", some_kwarg="...", _path="classlike.ftl")

    @classmethod
    def cls_i18n(cls) -> None:
        cls.cls_i18n.cls.key(some_kwarg="...", _path="classlike.ftl")
        cls.cls_i18n.get("cls-get-key", some_kwarg="...", _path="classlike.ftl")
"#;

    fn write_python_fixture(dir: &std::path::Path) {
        std::fs::write(dir.join("__init__.py"), "").unwrap();
        std::fs::write(dir.join("default.py"), DEFAULT_PY).unwrap();
        std::fs::write(dir.join("classlike.py"), CLASSLIKE_PY).unwrap();
    }

    #[test]
    fn test_find_py_files_dir() {
        let temp = TempDir::new().unwrap();
        let code_path = temp.path().join("py");
        std::fs::create_dir_all(&code_path).unwrap();
        write_python_fixture(&code_path);

        let ignore_set = GlobSet::empty();
        let py_files = super::find_py_files(&code_path, &ignore_set);
        assert_eq!(py_files.len(), 3);
    }

    #[test]
    fn test_find_py_files_file() {
        let temp = TempDir::new().unwrap();
        let code_dir = temp.path().join("py");
        std::fs::create_dir_all(&code_dir).unwrap();
        write_python_fixture(&code_dir);

        let code_path = code_dir.join("default.py");
        let ignore_set = GlobSet::empty();
        let py_files = super::find_py_files(&code_path, &ignore_set);
        assert_eq!(py_files.len(), 1);
        assert_eq!(py_files[0].path, code_path);
    }

    #[test]
    fn test_extract_fluent_keys() {
        let temp = TempDir::new().unwrap();
        let code_path = temp.path().join("py");
        std::fs::create_dir_all(&code_path).unwrap();
        write_python_fixture(&code_path);

        let mut key_prefixes = consts::DEFAULT_I18N_KEYS.clone();
        key_prefixes.insert("self".to_string());
        key_prefixes.insert("cls".to_string());
        let mut statistics = super::ExtractionStatistics::new();

        let fluent_keys = super::extract_fluent_keys(
            &code_path,
            key_prefixes.clone(),
            FastHashSet::default(),
            &globset::GlobSet::empty(),
            FastHashSet::default(),
            FastHashSet::default(),
            &PathBuf::from("locales/en.ftl"),
            false,
            None,
            false,
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
    fn test_extract_fluent_keys_matcher_edges() {
        let temp = TempDir::new().unwrap();
        let code_path = temp.path().join("app.py");
        std::fs::write(
            &code_path,
            r#"
L("named-key", kwarg="value", when=True)
i18n.get()
i18n.get(dynamic_key)
i18n.get("folder-key", _path="nested")
i18n.get("file-key", _path="custom.ftl")
i18n.set_locale("uk")
self.i18n.get("prefixed-get")
self.i18n.menu.open()
unknown("ignored")
"#,
        )
        .unwrap();

        let mut i18n_keys = FastHashSet::default();
        i18n_keys.insert("i18n".to_string());
        i18n_keys.insert("L".to_string());

        let mut prefixes = FastHashSet::default();
        prefixes.insert("self".to_string());

        let mut ignore_attributes = FastHashSet::default();
        ignore_attributes.insert("set_locale".to_string());

        let mut ignore_kwargs = FastHashSet::default();
        ignore_kwargs.insert("when".to_string());

        let mut statistics = super::ExtractionStatistics::new();
        let fluent_keys = super::extract_fluent_keys(
            &code_path,
            i18n_keys,
            prefixes,
            &globset::GlobSet::empty(),
            ignore_attributes,
            ignore_kwargs,
            &PathBuf::from("_default.ftl"),
            false,
            None,
            false,
            &mut statistics,
        );

        assert_eq!(statistics.py_files_count, 1);
        assert!(fluent_keys.contains_key("named-key"));
        assert!(fluent_keys.contains_key("folder-key"));
        assert!(fluent_keys.contains_key("file-key"));
        assert!(fluent_keys.contains_key("prefixed-get"));
        assert!(fluent_keys.contains_key("menu-open"));
        assert!(!fluent_keys.contains_key("set-locale"));

        assert_eq!(
            fluent_keys["folder-key"].path.as_ref(),
            &PathBuf::from("nested").join("_default.ftl")
        );
        assert_eq!(
            fluent_keys["file-key"].path.as_ref(),
            &PathBuf::from("custom.ftl")
        );

        let FluentEntry::Message(message) = fluent_keys["named-key"].entry.as_ref() else {
            panic!("expected message")
        };
        let elements = &message.value.as_ref().unwrap().elements;
        assert_eq!(elements.len(), 2);
    }

    #[test]
    fn test_parse_file_empty_no_key_invalid_utf8_and_invalid_python() {
        let temp = TempDir::new().unwrap();
        let empty = temp.path().join("empty.py");
        let no_key = temp.path().join("no_key.py");
        let invalid_utf8 = temp.path().join("invalid_utf8.py");
        let invalid_python = temp.path().join("invalid_python.py");
        std::fs::write(&empty, "").unwrap();
        std::fs::write(&no_key, "print('hello')").unwrap();
        std::fs::write(&invalid_utf8, b"i18n.get('\xFF')").unwrap();
        std::fs::write(&invalid_python, "i18n.get(").unwrap();

        let mut i18n_keys = FastHashSet::default();
        i18n_keys.insert("i18n".to_string());

        for path in [&empty, &no_key, &invalid_utf8, &invalid_python] {
            let keys = super::parse_file(
                path,
                std::fs::metadata(path).unwrap().len(),
                &i18n_keys,
                &FastHashSet::default(),
                &FastHashSet::default(),
                &FastHashSet::default(),
                &PathBuf::from("_default.ftl"),
            );
            assert!(keys.is_empty());
        }
    }

    #[test]
    fn test_extract_from_file_cache_hit_and_stale_remove() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("app.py");
        std::fs::write(&path, r#"i18n.get("hello")"#).unwrap();
        let metadata = std::fs::metadata(&path).unwrap();
        let py_file = super::PyFile {
            path: path.clone(),
            size: metadata.len(),
            modified_ns: super::file_modified_ns(&metadata),
        };

        let mut i18n_keys = FastHashSet::default();
        i18n_keys.insert("i18n".to_string());

        let (keys, update) = super::extract_from_file(
            &py_file,
            &i18n_keys,
            &FastHashSet::default(),
            &FastHashSet::default(),
            &FastHashSet::default(),
            &PathBuf::from("_default.ftl"),
            None,
        );
        assert!(keys.contains_key("hello"));
        assert!(matches!(update, Some(super::CacheUpdate::Upsert(_, _))));

        let mut cache = super::CacheFile {
            schema_version: 1,
            options: super::cache_options(
                &i18n_keys,
                &FastHashSet::default(),
                &FastHashSet::default(),
                &FastHashSet::default(),
                &PathBuf::from("_default.ftl"),
            ),
            files: FastHashMap::default(),
        };
        let Some(super::CacheUpdate::Upsert(cache_key, cached_file)) = update else {
            panic!("expected cache upsert")
        };
        cache.files.insert(cache_key, cached_file);

        let (cached_keys, cached_update) = super::extract_from_file(
            &py_file,
            &i18n_keys,
            &FastHashSet::default(),
            &FastHashSet::default(),
            &FastHashSet::default(),
            &PathBuf::from("_default.ftl"),
            Some(&cache),
        );
        assert!(cached_keys.contains_key("hello"));
        assert!(cached_update.is_none());

        std::fs::write(&path, "print('hello')").unwrap();
        let metadata = std::fs::metadata(&path).unwrap();
        let changed = super::PyFile {
            path,
            size: metadata.len(),
            modified_ns: super::file_modified_ns(&metadata),
        };
        let (empty_keys, remove_update) = super::extract_from_file(
            &changed,
            &i18n_keys,
            &FastHashSet::default(),
            &FastHashSet::default(),
            &FastHashSet::default(),
            &PathBuf::from("_default.ftl"),
            Some(&cache),
        );
        assert!(empty_keys.is_empty());
        assert!(matches!(remove_update, Some(super::CacheUpdate::Remove(_))));
    }

    #[test]
    fn test_sort_fluent_keys_by_path() {
        let mut fluent_keys: FastHashMap<String, FluentKey> = FastHashMap::default();

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
                FastHashSet::default(),
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
                FastHashSet::default(),
            ),
        );

        let sorted = super::sort_fluent_keys_by_path(fluent_keys.clone());
        assert_eq!(sorted.len(), 2);
        assert!(sorted.contains_key(&ftl_path1.clone()));
        assert!(sorted.contains_key(&ftl_path2.clone()));
    }
}
