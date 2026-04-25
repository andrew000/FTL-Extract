use crate::ftl::matcher::{FluentEntry, FluentKey, I18nMatcher};
use crate::ftl::utils::{ExtractionStatistics, FastHashMap, FastHashSet};
use bincode_next::{Decode, Encode};
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
use std::time::UNIX_EPOCH;

const CACHE_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
struct CacheOptions {
    i18n_keys: Vec<String>,
    i18n_keys_prefix: Vec<String>,
    ignore_attributes: Vec<String>,
    ignore_kwargs: Vec<String>,
    default_ftl_file: PathBuf,
}

#[derive(Clone, Debug, Encode, Decode)]
struct CacheFile {
    schema_version: u32,
    options: CacheOptions,
    files: FastHashMap<String, CachedPyFile>,
}

#[derive(Clone, Debug, Encode, Decode)]
struct CachedPyFile {
    size: u64,
    modified_ns: u128,
    keys: Vec<CachedFluentKey>,
}

#[derive(Clone, Debug, Encode, Decode)]
struct CachedFluentKey {
    key: String,
    code_path: PathBuf,
    ftl_path: PathBuf,
    kwargs: Vec<String>,
}

#[derive(Clone, Debug)]
struct PyFile {
    path: PathBuf,
    size: u64,
    modified_ns: u128,
}

enum CacheUpdate {
    Upsert(String, CachedPyFile),
    Remove(String),
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

fn sorted_values(values: &FastHashSet<String>) -> Vec<String> {
    let mut values = values.iter().cloned().collect::<Vec<_>>();
    values.sort_unstable();
    values
}

fn cache_options(
    i18n_keys: &FastHashSet<String>,
    i18n_keys_prefix: &FastHashSet<String>,
    ignore_attributes: &FastHashSet<String>,
    ignore_kwargs: &FastHashSet<String>,
    default_ftl_file: &Path,
) -> CacheOptions {
    CacheOptions {
        i18n_keys: sorted_values(i18n_keys),
        i18n_keys_prefix: sorted_values(i18n_keys_prefix),
        ignore_attributes: sorted_values(ignore_attributes),
        ignore_kwargs: sorted_values(ignore_kwargs),
        default_ftl_file: default_ftl_file.to_path_buf(),
    }
}

fn cache_file_path(cache_path: Option<&Path>) -> PathBuf {
    let file_name = format!(
        "extract-{}-v{CACHE_SCHEMA_VERSION}.bin",
        env!("CARGO_PKG_VERSION")
    );

    if let Some(path) = cache_path {
        if path.extension().is_some() {
            path.to_path_buf()
        } else {
            path.join(file_name)
        }
    } else {
        PathBuf::from(".ftl-extract-cache").join(file_name)
    }
}

fn file_cache_key(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn file_modified_ns(metadata: &fs::Metadata) -> u128 {
    metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |duration| duration.as_nanos())
}

fn load_cache(path: &Path, options: &CacheOptions, clear_cache: bool) -> CacheFile {
    if clear_cache {
        let _ = fs::remove_file(path);
    }

    let Ok(content) = fs::read(path) else {
        return CacheFile {
            schema_version: CACHE_SCHEMA_VERSION,
            options: options.clone(),
            files: FastHashMap::default(),
        };
    };

    let config = bincode_next::config::standard();
    match bincode_next::decode_from_slice::<CacheFile, _>(&content, config) {
        Ok((cache, len))
            if len == content.len()
                && cache.schema_version == CACHE_SCHEMA_VERSION
                && cache.options == *options =>
        {
            cache
        }
        _ => CacheFile {
            schema_version: CACHE_SCHEMA_VERSION,
            options: options.clone(),
            files: FastHashMap::default(),
        },
    }
}

fn save_cache(path: &Path, cache: &CacheFile) {
    if let Some(parent) = path.parent()
        && let Err(err) = fs::create_dir_all(parent)
    {
        error!(target: "extractor:code", "Failed to create cache directory {}: {}", parent.display(), err);
        return;
    }

    let config = bincode_next::config::standard();
    match bincode_next::encode_to_vec(cache, config) {
        Ok(content) => {
            if let Err(err) = fs::write(path, content) {
                error!(target: "extractor:code", "Failed to write cache {}: {}", path.display(), err);
            }
        }
        Err(err) => {
            error!(target: "extractor:code", "Failed to serialize extraction cache: {}", err)
        }
    }
}

fn cached_key_to_fluent_key(cached: CachedFluentKey) -> FluentKey {
    let mut elements = vec![fluent_syntax::ast::PatternElement::TextElement {
        value: cached.key.clone(),
    }];

    for kwarg in &cached.kwargs {
        elements.push(fluent_syntax::ast::PatternElement::Placeable {
            expression: fluent_syntax::ast::Expression::Inline(
                fluent_syntax::ast::InlineExpression::VariableReference {
                    id: fluent_syntax::ast::Identifier {
                        name: kwarg.clone(),
                    },
                },
            ),
        });
    }

    FluentKey::new(
        Arc::new(cached.code_path),
        cached.key.clone(),
        FluentEntry::Message(fluent_syntax::ast::Message {
            id: fluent_syntax::ast::Identifier { name: cached.key },
            value: Some(fluent_syntax::ast::Pattern { elements }),
            attributes: vec![],
            comment: None,
        }),
        Arc::new(cached.ftl_path),
        None,
        None,
        FastHashSet::default(),
    )
}

fn fluent_key_to_cached_key(fluent_key: FluentKey) -> CachedFluentKey {
    let kwargs = match fluent_key.entry.as_ref() {
        FluentEntry::Message(message) => message
            .value
            .as_ref()
            .map(|pattern| {
                pattern
                    .elements
                    .iter()
                    .filter_map(|element| match element {
                        fluent_syntax::ast::PatternElement::Placeable {
                            expression:
                                fluent_syntax::ast::Expression::Inline(
                                    fluent_syntax::ast::InlineExpression::VariableReference { id },
                                ),
                        } => Some(id.name.clone()),
                        _ => None,
                    })
                    .collect()
            })
            .unwrap_or_default(),
        _ => Vec::new(),
    };

    CachedFluentKey {
        key: fluent_key.key,
        code_path: fluent_key.code_path.as_ref().clone(),
        ftl_path: fluent_key.path.as_ref().clone(),
        kwargs,
    }
}

fn cached_file_to_keys(cached: &CachedPyFile) -> FastHashMap<String, FluentKey> {
    cached
        .keys
        .iter()
        .cloned()
        .map(cached_key_to_fluent_key)
        .map(|key| (key.key.clone(), key))
        .collect()
}

fn keys_to_cached_file(file: &PyFile, keys: &FastHashMap<String, FluentKey>) -> CachedPyFile {
    CachedPyFile {
        size: file.size,
        modified_ns: file.modified_ns,
        keys: keys
            .values()
            .cloned()
            .map(fluent_key_to_cached_key)
            .collect(),
    }
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

    let cached_file = keys_to_cached_file(file, &keys);

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
        assert_eq!(py_files[0].path, code_path);
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
