use crate::ftl::matcher::{FluentEntry, FluentKey};
use crate::ftl::utils::ExtractionStatistics;
use anyhow::{Result, bail};
use fluent_syntax::ast::Entry;
use hashbrown::{HashMap, HashSet};
use ignore::WalkBuilder;
use ignore::types::TypesBuilder;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

type ImportFtlResult = Result<(
    HashMap<String, FluentKey>,
    HashMap<String, FluentKey>,
    Vec<FluentKey>,
)>;

fn process_raw_ftl(
    body: &[Entry<String>],
    path: &Path,
    locale: &String,
    ftl_keys: &mut HashMap<String, FluentKey>,
    terms: &mut HashMap<String, FluentKey>,
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
                        HashSet::new(),
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
                        HashSet::new(),
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
                HashSet::new(),
            )),
            Entry::GroupComment(comment) => {
                leave_as_is_keys.push(FluentKey::new(
                    Arc::new(PathBuf::new()),
                    "".to_string(),
                    FluentEntry::GroupComment(comment.clone()),
                    Arc::new(path.to_path_buf()),
                    Some(locale.to_string()),
                    Some(position),
                    HashSet::new(),
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
                    HashSet::new(),
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

fn import_from_ftl(path: &PathBuf, locale: &String) -> ImportFtlResult {
    let mut ftl_keys: HashMap<String, FluentKey> = HashMap::new();
    let mut terms: HashMap<String, FluentKey> = HashMap::new();
    let mut leave_as_is_keys: Vec<FluentKey> = Vec::new();

    let resource = fluent_syntax::parser::parse(fs::read_to_string(path)?);
    let resource = match resource {
        Ok(res) => res,
        Err(err) => {
            bail!("Failed to parse FTL file {}: {:?}", path.display(), err.1);
        }
    };

    process_raw_ftl(
        &resource.body,
        path,
        locale,
        &mut ftl_keys,
        &mut terms,
        &mut leave_as_is_keys,
    )?;

    Ok((ftl_keys, terms, leave_as_is_keys))
}

pub(crate) fn import_ftl_from_dir(
    path: &Path,
    locale: &String,
    statistics: &mut ExtractionStatistics,
) -> ImportFtlResult {
    let ftl_files = {
        let mut type_builder = TypesBuilder::new();
        type_builder.add("ftl", "*.ftl")?;
        type_builder.select("ftl");
        WalkBuilder::new(path.join(locale))
            .types(type_builder.build()?)
            .parents(false)
            .ignore(false)
            .git_global(false)
            .git_exclude(false)
            .require_git(false)
            .build()
    };
    let mut stored_ftl_keys = HashMap::<String, FluentKey>::new();
    let mut stored_terms = HashMap::<String, FluentKey>::new();
    let mut stored_leave_as_is_keys: Vec<FluentKey> = Vec::new();

    for entry in ftl_files {
        let ftl_file = match entry {
            Ok(entry) => {
                let path = entry.path();
                if entry.file_type().is_some_and(|ft| ft.is_file()) {
                    path.to_path_buf()
                } else {
                    continue;
                }
            }
            Err(_) => continue,
        };

        let (keys, terms, leave_as_is) = import_from_ftl(&ftl_file, locale).unwrap();
        stored_ftl_keys.extend(keys);
        stored_terms.extend(terms);
        stored_leave_as_is_keys.extend(leave_as_is);

        *statistics.ftl_files_count.get_mut(locale).unwrap() += 1;
    }

    Ok((stored_ftl_keys, stored_terms, stored_leave_as_is_keys))
}

#[cfg(test)]
mod tests {
    use fluent_syntax::ast::Entry::Junk;
    use hashbrown::HashMap;
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
        let mut ftl_keys: HashMap<String, super::FluentKey> = HashMap::new();
        let mut terms: HashMap<String, super::FluentKey> = HashMap::new();
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
