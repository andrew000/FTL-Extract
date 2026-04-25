use crate::ftl::matcher::{FluentEntry, FluentKey};
use crate::ftl::utils::{FastHashMap, FastHashSet};
use bincode_next::{Decode, Encode};
use log::error;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::UNIX_EPOCH;

pub(super) const CACHE_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub(super) struct CacheOptions {
    i18n_keys: Vec<String>,
    i18n_keys_prefix: Vec<String>,
    ignore_attributes: Vec<String>,
    ignore_kwargs: Vec<String>,
    default_ftl_file: PathBuf,
}

#[derive(Clone, Debug, Encode, Decode)]
pub(super) struct CacheFile {
    schema_version: u32,
    options: CacheOptions,
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
