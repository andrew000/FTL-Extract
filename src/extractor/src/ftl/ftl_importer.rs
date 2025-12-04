use crate::ftl::matcher::{FluentEntry, FluentKey};
use crate::ftl::utils::{ExtractionStatistics, FastHashMap, FastHashSet};
use anyhow::{Context, Result, bail};
use fluent_syntax::ast::Entry;
use ignore::WalkBuilder;
use ignore::types::TypesBuilder;
use rayon::prelude::{IntoParallelRefIterator, ParallelIterator};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
type ImportResult = (
    FastHashMap<String, FluentKey>,
    FastHashMap<String, FluentKey>,
    Vec<FluentKey>,
);

fn process_raw_ftl(
    body: &[Entry<String>],
    path: &Path,
    locale: &String,
    ftl_keys: &mut FastHashMap<String, FluentKey>,
    terms: &mut FastHashMap<String, FluentKey>,
    leave_as_is_keys: &mut Vec<FluentKey>,
) -> Result<()> {
    for (position, entry) in body.iter().enumerate() {
        match entry {
            Entry::Message(message) => {
                ftl_keys.insert(
                    message.id.name.clone(),
                    FluentKey::new(
                        Arc::new(PathBuf::new()),
                        message.id.name.clone(),
                        FluentEntry::Message(message.clone()),
                        Arc::new(path.to_path_buf()),
                        Some(locale.to_string()),
                        Some(position),
                        FastHashSet::default(),
                    ),
                );
            }
            Entry::Term(term) => {
                terms.insert(
                    term.id.name.clone(),
                    FluentKey::new(
                        Arc::new(PathBuf::new()),
                        term.id.name.clone(),
                        FluentEntry::Term(term.clone()),
                        Arc::new(path.to_path_buf()),
                        Some(locale.to_string()),
                        Some(position),
                        FastHashSet::default(),
                    ),
                );
            }
            Entry::Comment(comment) => leave_as_is_keys.push(FluentKey::new(
                Arc::new(PathBuf::new()),
                "".to_string(),
                FluentEntry::Comment(comment.clone()),
                Arc::new(path.to_path_buf()),
                Some(locale.to_string()),
                Some(position),
                FastHashSet::default(),
            )),
            Entry::GroupComment(comment) => {
                leave_as_is_keys.push(FluentKey::new(
                    Arc::new(PathBuf::new()),
                    "".to_string(),
                    FluentEntry::GroupComment(comment.clone()),
                    Arc::new(path.to_path_buf()),
                    Some(locale.to_string()),
                    Some(position),
                    FastHashSet::default(),
                ));
            }
            Entry::ResourceComment(comment) => {
                leave_as_is_keys.push(FluentKey::new(
                    Arc::new(PathBuf::new()),
                    "".to_string(),
                    FluentEntry::ResourceComment(comment.clone()),
                    Arc::new(path.to_path_buf()),
                    Some(locale.to_string()),
                    Some(position),
                    FastHashSet::default(),
                ));
            }
            _ => {
                bail!(
                    "Unsupported FTL entry type in file {} at position {}",
                    path.display(),
                    position
                )
            }
        }
    }
    Ok(())
}

fn import_from_ftl(path: &Path, locale: &str) -> Result<ImportResult> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read FTL file: {}", path.display()))?;

    let resource = fluent_syntax::parser::parse(content).map_err(|err| {
        anyhow::anyhow!("Failed to parse FTL file {}: {:?}", path.display(), err.1)
    })?;

    let mut keys = FastHashMap::default();
    let mut terms = FastHashMap::default();
    let mut misc = Vec::new();

    let path_arc = Arc::new(path.to_path_buf());
    // Empty code path for imported keys
    let code_path_arc = Arc::new(PathBuf::new());

    for (pos, entry) in resource.body.into_iter().enumerate() {
        // Helper closure to avoid repeating new() calls
        let make_key = |key_name: String, entry_type: FluentEntry| -> FluentKey {
            FluentKey::new(
                code_path_arc.clone(),
                key_name,
                entry_type,
                path_arc.clone(),
                Some(locale.to_string()),
                Some(pos),
                FastHashSet::default(),
            )
        };

        match entry {
            Entry::Message(m) => {
                let name = m.id.name.clone();
                keys.insert(name.clone(), make_key(name, FluentEntry::Message(m)));
            }
            Entry::Term(t) => {
                let name = t.id.name.clone();
                terms.insert(name.clone(), make_key(name, FluentEntry::Term(t)));
            }
            Entry::Comment(c) => misc.push(make_key("".into(), FluentEntry::Comment(c))),
            Entry::GroupComment(c) => misc.push(make_key("".into(), FluentEntry::GroupComment(c))),
            Entry::ResourceComment(c) => {
                misc.push(make_key("".into(), FluentEntry::ResourceComment(c)))
            }
            _ => bail!("Unsupported entry in {}: {:?}", path.display(), entry),
        }
    }

    Ok((keys, terms, misc))
}

pub(crate) fn import_ftl_from_dir(
    path: &Path,
    locale: &String,
    statistics: &mut ExtractionStatistics,
) -> Result<ImportResult> {
    let mut type_builder = TypesBuilder::new();
    type_builder.add("ftl", "*.ftl")?;
    type_builder.select("ftl");

    let walker = WalkBuilder::new(path.join(locale))
        .types(type_builder.build()?)
        .parents(false)
        .git_global(false)
        .build();

    let paths: Vec<PathBuf> = walker
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_some_and(|ft| ft.is_file()))
        .map(|entry| entry.path().to_path_buf())
        .collect();

    let files_count = paths.len();
    *statistics.ftl_files_count.get_mut(locale).unwrap() += files_count;

    let (stored_keys, stored_terms, stored_misc) = paths
        .par_iter()
        .map(|file_path| import_from_ftl(file_path, locale))
        .try_fold(
            || (FastHashMap::default(), FastHashMap::default(), Vec::new()),
            |mut acc, result| {
                let (keys, terms, misc) = result?;
                acc.0.extend(keys);
                acc.1.extend(terms);
                acc.2.extend(misc);
                Ok::<ImportResult, anyhow::Error>(acc)
            },
        )
        .try_reduce(
            || (FastHashMap::default(), FastHashMap::default(), Vec::new()),
            |mut a, b| {
                a.0.extend(b.0);
                a.1.extend(b.1);
                a.2.extend(b.2);
                Ok(a)
            },
        )?;

    Ok((stored_keys, stored_terms, stored_misc))
}

#[cfg(test)]
mod tests {
    use crate::ftl::utils::FastHashMap;
    use fluent_syntax::ast::Entry::Junk;
    use std::path::PathBuf;

    #[test]
    fn test_import_from_ftl() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("locales")
            .join("en")
            .join("_default.ftl");
        let locale = "en".to_string();
        let (ftl_keys, terms, leave_as_is_keys) = super::import_from_ftl(&path, &locale).unwrap();

        assert_eq!(ftl_keys.len(), 13);
        assert_eq!(terms.len(), 5);
        assert_eq!(leave_as_is_keys.len(), 3);
    }

    #[test]
    #[should_panic = "Unsupported FTL entry type in file"]
    fn test_process_raw_ftl_with_junk() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("_junk.ftl");
        let locale = "en".to_string();
        let body: Vec<fluent_syntax::ast::Entry<String>> = vec![Junk {
            content: "This is junk".to_string(),
        }];
        let mut ftl_keys: FastHashMap<String, super::FluentKey> = FastHashMap::default();
        let mut terms: FastHashMap<String, super::FluentKey> = FastHashMap::default();
        let mut leave_as_is_keys: Vec<super::FluentKey> = Vec::new();

        super::process_raw_ftl(
            &body,
            &path,
            &locale,
            &mut ftl_keys,
            &mut terms,
            &mut leave_as_is_keys,
        )
        .unwrap();
    }

    #[test]
    #[should_panic = "Failed to parse FTL file"]
    fn test_import_from_ftl_with_junk() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("_junk.ftl");
        let locale = "en".to_string();
        let (_ftl_keys, _terms, _leave_as_is_keys) =
            super::import_from_ftl(&path, &locale).unwrap();
    }

    #[test]
    fn test_import_ftl_from_dir() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("locales");
        let locale = "en".to_string();
        let mut statistics = super::ExtractionStatistics::new();
        statistics.ftl_files_count.insert(locale.clone(), 0);

        let (ftl_keys, terms, leave_as_is_keys) =
            super::import_ftl_from_dir(&path, &locale, &mut statistics).unwrap();
        assert_eq!(ftl_keys.len(), 13);
        assert_eq!(terms.len(), 5);
        assert_eq!(leave_as_is_keys.len(), 3);
        assert_eq!(*statistics.ftl_files_count.get(&locale).unwrap(), 1);
    }
}
