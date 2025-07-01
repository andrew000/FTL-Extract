use crate::ftl::matcher::{FluentEntry, FluentKey};
use crate::ftl::utils::ExtractionStatistics;
use fluent_syntax::ast::Entry;
use hashbrown::{HashMap, HashSet};
use ignore::WalkBuilder;
use ignore::types::TypesBuilder;
use std::fs;
use std::path::{Path, PathBuf};

fn import_from_ftl(
    path: &PathBuf,
    locale: &String,
) -> (
    HashMap<String, FluentKey>,
    HashMap<String, FluentKey>,
    Vec<FluentKey>,
) {
    let mut ftl_keys: HashMap<String, FluentKey> = HashMap::new();
    let mut terms: HashMap<String, FluentKey> = HashMap::new();
    let mut leave_as_is_keys: Vec<FluentKey> = Vec::new();

    let resource = fluent_syntax::parser::parse(fs::read_to_string(path).unwrap()).unwrap();

    for (position, entry) in resource.body.iter().enumerate() {
        match entry {
            Entry::Message(message) => {
                ftl_keys.insert(
                    message.id.name.clone(),
                    FluentKey::new(
                        PathBuf::new(),
                        message.id.name.clone(),
                        FluentEntry::Message(message.clone()),
                        path.to_path_buf(),
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
                        PathBuf::new(),
                        term.id.name.clone(),
                        FluentEntry::Term(term.clone()),
                        path.to_path_buf(),
                        Some(locale.to_string()),
                        Some(position),
                        HashSet::new(),
                    ),
                );
            }
            Entry::Comment(comment) => leave_as_is_keys.push(FluentKey::new(
                PathBuf::new(),
                "".to_string(),
                FluentEntry::Comment(comment.clone()),
                path.to_path_buf(),
                Some(locale.to_string()),
                Some(position),
                HashSet::new(),
            )),
            Entry::GroupComment(comment) => {
                leave_as_is_keys.push(FluentKey::new(
                    PathBuf::new(),
                    "".to_string(),
                    FluentEntry::Comment(comment.clone()),
                    path.to_path_buf(),
                    Some(locale.to_string()),
                    Some(position),
                    HashSet::new(),
                ));
            }
            Entry::ResourceComment(comment) => {
                leave_as_is_keys.push(FluentKey::new(
                    PathBuf::new(),
                    "".to_string(),
                    FluentEntry::Comment(comment.clone()),
                    path.to_path_buf(),
                    Some(locale.to_string()),
                    Some(position),
                    HashSet::new(),
                ));
            }
            Entry::Junk { content } => {
                leave_as_is_keys.push(FluentKey::new(
                    PathBuf::new(),
                    "".to_string(),
                    FluentEntry::Junk(content.clone()),
                    path.to_path_buf(),
                    Some(locale.to_string()),
                    Some(position),
                    HashSet::new(),
                ));
            }
        }
    }

    (ftl_keys, terms, leave_as_is_keys)
}

pub(crate) fn import_ftl_from_dir(
    path: &Path,
    locale: &String,
    statistics: &mut ExtractionStatistics,
) -> (
    HashMap<String, FluentKey>,
    HashMap<String, FluentKey>,
    Vec<FluentKey>,
) {
    let ftl_files = {
        let mut type_builder = TypesBuilder::new();
        type_builder.add("ftl", "*.ftl").unwrap();
        type_builder.select("ftl");
        if path.is_dir() {
            WalkBuilder::new(path.join(locale))
                .types(type_builder.build().unwrap())
                .parents(false)
                .ignore(false)
                .git_global(false)
                .git_exclude(false)
                .require_git(false)
                .build()
        } else {
            WalkBuilder::new(path.join(locale))
                .types(type_builder.build().unwrap())
                .parents(false)
                .ignore(false)
                .git_global(false)
                .git_exclude(false)
                .require_git(false)
                .build()
        }
    };
    let mut stored_ftl_keys = HashMap::<String, FluentKey>::new();
    let mut stored_terms = HashMap::<String, FluentKey>::new();
    let mut stored_leave_as_is_keys: Vec<FluentKey> = Vec::new();

    for entry in ftl_files {
        let ftl_file = match entry {
            Ok(entry) => {
                let path = entry.path();
                if entry.file_type().map_or(false, |ft| ft.is_file()) {
                    path.to_path_buf()
                } else {
                    continue;
                }
            }
            Err(_) => continue,
        };

        let (keys, terms, leave_as_is) = import_from_ftl(&ftl_file, &locale);
        stored_ftl_keys.extend(keys);
        stored_terms.extend(terms);
        stored_leave_as_is_keys.extend(leave_as_is);

        *statistics.ftl_files_count.get_mut(locale).unwrap() += 1;
    }

    (stored_ftl_keys, stored_terms, stored_leave_as_is_keys)
}
