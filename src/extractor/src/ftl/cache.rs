use crate::ftl::code_extractor::kwargs_from_key;
use crate::ftl::diagnostics::CodeLocation;
use crate::ftl::matcher::{FluentEntry, FluentKey, code_message};
use crate::ftl::utils::{FastHashMap, FastHashSet};
use bincode_next::{Decode, Encode};
use log::error;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::UNIX_EPOCH;

// Bump whenever `CacheOptions` or the cached key layout changes; older files are then ignored.
pub(super) const CACHE_SCHEMA_VERSION: u32 = 3;

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub(super) struct CacheOptions {
    i18n_keys: Vec<String>,
    i18n_keys_prefix: Vec<String>,
    ignore_attributes: Vec<String>,
    ignore_kwargs: Vec<String>,
    exclude_dirs: Vec<String>,
    default_ftl_file: PathBuf,
}

#[derive(Clone, Debug, Encode, Decode)]
pub(super) struct CacheFile {
    pub(super) schema_version: u32,
    pub(super) options: CacheOptions,
    pub(super) files: FastHashMap<String, CachedPyFile>,
}

#[derive(Clone, Debug, Encode, Decode)]
pub(super) struct CachedPyFile {
    pub(super) size: u64,
    pub(super) modified_ns: u128,
    keys: Vec<CachedFluentKey>,
}

#[derive(Clone, Debug, Encode, Decode)]
struct CachedFluentKey {
    key: String,
    code_path: PathBuf,
    ftl_path: PathBuf,
    kwargs: Vec<String>,
    source_line: Option<usize>,
    source_column: Option<usize>,
}

pub(super) enum CacheUpdate {
    Upsert(String, CachedPyFile),
    Remove(String),
}

fn sorted_values(values: &FastHashSet<String>) -> Vec<String> {
    let mut values = values.iter().cloned().collect::<Vec<_>>();
    values.sort_unstable();
    values
}

pub(super) fn cache_options(
    i18n_keys: &FastHashSet<String>,
    i18n_keys_prefix: &FastHashSet<String>,
    ignore_attributes: &FastHashSet<String>,
    ignore_kwargs: &FastHashSet<String>,
    exclude_dirs: &FastHashSet<String>,
    default_ftl_file: &Path,
) -> CacheOptions {
    CacheOptions {
        i18n_keys: sorted_values(i18n_keys),
        i18n_keys_prefix: sorted_values(i18n_keys_prefix),
        ignore_attributes: sorted_values(ignore_attributes),
        ignore_kwargs: sorted_values(ignore_kwargs),
        exclude_dirs: sorted_values(exclude_dirs),
        default_ftl_file: default_ftl_file.to_path_buf(),
    }
}

pub(super) fn cache_file_path(cache_path: Option<&Path>) -> PathBuf {
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

pub(super) fn file_cache_key(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

pub(super) fn file_modified_ns(metadata: &fs::Metadata) -> u128 {
    metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |duration| duration.as_nanos())
}

pub(super) fn load_cache(path: &Path, options: &CacheOptions, clear_cache: bool) -> CacheFile {
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

pub(super) fn save_cache(path: &Path, cache: &CacheFile) {
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
    // `code_message` sorts the kwargs, so an entry cached in call order by an older build
    // yields the same message as a fresh parse.
    let mut fluent_key = FluentKey::new(
        Arc::new(cached.code_path),
        cached.key.clone(),
        FluentEntry::Message(code_message(cached.key, cached.kwargs)),
        Arc::new(cached.ftl_path),
        None,
        None,
        FastHashSet::default(),
    );

    if let (Some(line), Some(column)) = (cached.source_line, cached.source_column) {
        fluent_key.source_location = Some(CodeLocation {
            path: fluent_key.code_path.as_ref().clone(),
            line,
            column,
        });
    }

    fluent_key
}

fn fluent_key_to_cached_key(fluent_key: FluentKey) -> CachedFluentKey {
    let kwargs = kwargs_from_key(&fluent_key);
    let source_line = fluent_key
        .source_location
        .as_ref()
        .map(|location| location.line);
    let source_column = fluent_key
        .source_location
        .as_ref()
        .map(|location| location.column);

    CachedFluentKey {
        key: fluent_key.key,
        code_path: fluent_key.code_path.as_ref().clone(),
        ftl_path: fluent_key.path.as_ref().clone(),
        kwargs,
        source_line,
        source_column,
    }
}

pub(super) fn cached_file_to_keys(cached: &CachedPyFile) -> FastHashMap<String, FluentKey> {
    cached
        .keys
        .iter()
        .cloned()
        .map(cached_key_to_fluent_key)
        .map(|key| (key.key.clone(), key))
        .collect()
}

pub(super) fn keys_to_cached_file(
    size: u64,
    modified_ns: u128,
    keys: &FastHashMap<String, FluentKey>,
) -> CachedPyFile {
    CachedPyFile {
        size,
        modified_ns,
        keys: keys
            .values()
            .cloned()
            .map(fluent_key_to_cached_key)
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    fn options(default_ftl_file: &str) -> CacheOptions {
        let keys = FastHashSet::from_iter(["i18n".to_string()]);
        cache_options(
            &keys,
            &FastHashSet::default(),
            &FastHashSet::default(),
            &FastHashSet::default(),
            &FastHashSet::default(),
            &PathBuf::from(default_ftl_file),
        )
    }

    fn fluent_key() -> FluentKey {
        FluentKey::new(
            Arc::new(PathBuf::from("app.py")),
            "hello".to_string(),
            FluentEntry::Message(fluent_syntax::ast::Message {
                id: fluent_syntax::ast::Identifier {
                    name: "hello".to_string(),
                },
                value: Some(fluent_syntax::ast::Pattern {
                    elements: vec![
                        fluent_syntax::ast::PatternElement::TextElement {
                            value: "hello".to_string(),
                        },
                        fluent_syntax::ast::PatternElement::Placeable {
                            expression: fluent_syntax::ast::Expression::Inline(
                                fluent_syntax::ast::InlineExpression::VariableReference {
                                    id: fluent_syntax::ast::Identifier {
                                        name: "name".to_string(),
                                    },
                                },
                            ),
                        },
                    ],
                }),
                attributes: vec![],
                comment: None,
            }),
            Arc::new(PathBuf::from("_default.ftl")),
            None,
            None,
            FastHashSet::default(),
        )
    }

    fn term_key() -> FluentKey {
        FluentKey::new(
            Arc::new(PathBuf::from("app.py")),
            "brand".to_string(),
            FluentEntry::Term(fluent_syntax::ast::Term {
                id: fluent_syntax::ast::Identifier {
                    name: "brand".to_string(),
                },
                value: fluent_syntax::ast::Pattern { elements: vec![] },
                attributes: vec![],
                comment: None,
            }),
            Arc::new(PathBuf::from("_default.ftl")),
            None,
            None,
            FastHashSet::default(),
        )
    }

    #[test]
    fn test_cache_file_path_uses_versioned_name_for_directories() {
        assert_eq!(
            cache_file_path(None),
            PathBuf::from(".ftl-extract-cache")
                .join(format!("extract-{}-v3.bin", env!("CARGO_PKG_VERSION")))
        );
        assert_eq!(
            cache_file_path(Some(Path::new("cache-dir"))),
            PathBuf::from("cache-dir")
                .join(format!("extract-{}-v3.bin", env!("CARGO_PKG_VERSION")))
        );
        assert_eq!(
            cache_file_path(Some(Path::new("custom.bin"))),
            PathBuf::from("custom.bin")
        );
    }

    #[test]
    fn test_file_cache_key_normalizes_windows_separators() {
        assert_eq!(file_cache_key(Path::new("src\\app.py")), "src/app.py");
    }

    #[test]
    fn test_cache_roundtrip_and_option_mismatch() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("cache.bin");
        let opts = options("_default.ftl");

        let mut keys = FastHashMap::default();
        let key = fluent_key();
        keys.insert(key.key.clone(), key);

        let cached_file = keys_to_cached_file(123, 456, &keys);
        let mut cache = CacheFile {
            schema_version: CACHE_SCHEMA_VERSION,
            options: opts.clone(),
            files: FastHashMap::default(),
        };
        cache.files.insert("app.py".to_string(), cached_file);

        save_cache(&path, &cache);

        let loaded = load_cache(&path, &opts, false);
        let loaded_file = loaded.files.get("app.py").unwrap();
        assert_eq!(loaded_file.size, 123);
        assert_eq!(loaded_file.modified_ns, 456);

        let loaded_keys = cached_file_to_keys(loaded_file);
        let loaded_key = loaded_keys.get("hello").unwrap();
        assert_eq!(loaded_key.path.as_ref(), &PathBuf::from("_default.ftl"));

        let mismatched = load_cache(&path, &options("other.ftl"), false);
        assert!(mismatched.files.is_empty());
    }

    #[test]
    fn test_non_message_cache_key_has_no_kwargs() {
        let mut keys = FastHashMap::default();
        let key = term_key();
        keys.insert(key.key.clone(), key);

        let cached_file = keys_to_cached_file(1, 2, &keys);
        let loaded_keys = cached_file_to_keys(&cached_file);

        let loaded = loaded_keys.get("brand").unwrap();
        let FluentEntry::Message(message) = loaded.entry.as_ref() else {
            panic!("cached keys are reconstructed as messages")
        };
        assert_eq!(message.value.as_ref().unwrap().elements.len(), 1);
    }

    #[test]
    fn test_load_cache_clear_cache_removes_existing_file() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("cache.bin");
        fs::write(&path, b"bad cache").unwrap();

        let loaded = load_cache(&path, &options("_default.ftl"), true);

        assert!(loaded.files.is_empty());
        assert!(!path.exists());
    }

    #[test]
    fn test_cache_rejects_different_exclude_dirs() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("cache.bin");
        let keys = FastHashSet::from_iter(["i18n".to_string()]);
        let empty = FastHashSet::default();
        let with_excludes = |excludes: &FastHashSet<String>| {
            cache_options(
                &keys,
                &empty,
                &empty,
                &empty,
                excludes,
                &PathBuf::from("_default.ftl"),
            )
        };
        let none = with_excludes(&FastHashSet::default());
        let tests = with_excludes(&FastHashSet::from_iter(["**/tests/**".to_string()]));

        let mut cache = CacheFile {
            schema_version: CACHE_SCHEMA_VERSION,
            options: none.clone(),
            files: FastHashMap::default(),
        };
        let mut keys_map = FastHashMap::default();
        let key = fluent_key();
        keys_map.insert(key.key.clone(), key);
        cache
            .files
            .insert("app.py".to_string(), keys_to_cached_file(1, 2, &keys_map));
        save_cache(&path, &cache);

        assert_eq!(load_cache(&path, &none, false).files.len(), 1);
        assert!(load_cache(&path, &tests, false).files.is_empty());
    }

    #[test]
    fn test_cached_call_order_kwargs_are_normalized_on_load() {
        // An entry written by a build that stored kwargs in call order must produce the same
        // message as a fresh parse, which sorts them.
        let cached = CachedFluentKey {
            key: "order".to_string(),
            code_path: PathBuf::from("app.py"),
            ftl_path: PathBuf::from("_default.ftl"),
            kwargs: vec!["b".to_string(), "a".to_string()],
            source_line: Some(1),
            source_column: Some(1),
        };

        let loaded = cached_key_to_fluent_key(cached);

        assert_eq!(kwargs_from_key(&loaded), vec!["a", "b"]);
        assert_eq!(
            loaded.entry.as_ref(),
            &FluentEntry::Message(code_message(
                "order".to_string(),
                vec!["a".to_string(), "b".to_string()]
            ))
        );
    }
}
