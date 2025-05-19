use crate::ftl::matcher::FluentKey;
use crate::ftl::utils::ExtractionStatistics;
use fluent_syntax::ast::Entry;
use globwalk::GlobWalkerBuilder;
use hashbrown::{HashMap, HashSet};
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
                        Some(message.clone()),
                        None,
                        None,
                        None,
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
                        None,
                        Some(term.clone()),
                        None,
                        None,
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
                None,
                None,
                Some(comment.clone()),
                None,
                path.to_path_buf(),
                Some(locale.to_string()),
                Some(position),
                HashSet::new(),
            )),
            Entry::GroupComment(comment) => {
                leave_as_is_keys.push(FluentKey::new(
                    PathBuf::new(),
                    "".to_string(),
                    None,
                    None,
                    Some(comment.clone()),
                    None,
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
                    None,
                    None,
                    Some(comment.clone()),
                    None,
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
                    None,
                    None,
                    None,
                    Some(content.clone()),
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
        if path.is_dir() {
            GlobWalkerBuilder::from_patterns(path.join(locale), &["**/*.ftl"])
                .build()
                .unwrap()
        } else {
            GlobWalkerBuilder::from_patterns(path.join(locale), &[path.to_str().unwrap()])
                .build()
                .unwrap()
        }
    };
    let mut stored_ftl_keys = HashMap::<String, FluentKey>::new();
    let mut stored_terms = HashMap::<String, FluentKey>::new();
    let mut stored_leave_as_is_keys: Vec<FluentKey> = Vec::new();

    for entry in ftl_files {
        let ftl_file = match entry {
            Ok(entry) => entry,
            Err(_) => continue,
        };
        let (keys, terms, leave_as_is) = import_from_ftl(&ftl_file.into_path(), &locale);
        stored_ftl_keys.extend(keys);
        stored_terms.extend(terms);
        stored_leave_as_is_keys.extend(leave_as_is);

        *statistics.ftl_files_count.get_mut(locale).unwrap() += 1;
    }

    (stored_ftl_keys, stored_terms, stored_leave_as_is_keys)
}
