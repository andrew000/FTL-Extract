use crate::ftl::code_extractor::{extract_fluent_keys, sort_fluent_keys_by_path};
use crate::ftl::consts::CommentsKeyModes;
use crate::ftl::ftl_importer::import_ftl_from_dir;
use crate::ftl::matcher::{FluentEntry, FluentKey};
use crate::ftl::process::commentator::comment_ftl_key;
use crate::ftl::process::kwargs_extractor::extract_kwargs;
use crate::ftl::process::serializer::generate_ftl;
use crate::ftl::utils::ExtractionStatistics;
use anyhow::Result;
use hashbrown::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

pub fn extract(
    code_path: &Path,
    output_path: &Path,
    language: Vec<String>,
    mut i18n_keys: HashSet<String>, // = consts::DEFAULT_I18N_KEYS.clone(),
    i18n_keys_append: HashSet<String>, // = HashSet::from(),
    i18n_keys_prefix: HashSet<String>, // = HashSet::from(),
    mut exclude_dirs: HashSet<String>, // = &mut consts::DEFAULT_EXCLUDE_DIRS.clone(),
    exclude_dirs_append: HashSet<String>, // = HashSet::from(),
    mut ignore_attributes: HashSet<String>, // = &mut consts::DEFAULT_IGNORE_ATTRIBUTES.clone(),
    append_ignore_attributes: HashSet<String>, // = HashSet::new(),
    ignore_kwargs: HashSet<String>, // = consts::DEFAULT_IGNORE_KWARGS,
    comment_junks: bool,            // = true,
    default_ftl_file: &Path,        // = consts::DEFAULT_FTL_FILENAME
    comment_keys_mode: CommentsKeyModes, // = consts::CommentsKeyModes::Comment,
    dry_run: bool,                  // = false,
) -> Result<ExtractionStatistics> {
    let mut statistics = ExtractionStatistics::new();
    statistics
        .ftl_files_count
        .extend(language.iter().map(|lang| (lang.clone(), 0)));
    statistics
        .ftl_stored_keys_count
        .extend(language.iter().map(|lang| (lang.clone(), 0)));
    statistics
        .ftl_keys_updated
        .extend(language.iter().map(|lang| (lang.clone(), 0)));
    statistics
        .ftl_keys_added
        .extend(language.iter().map(|lang| (lang.clone(), 0)));
    statistics
        .ftl_keys_commented
        .extend(language.iter().map(|lang| (lang.clone(), 0)));

    if !i18n_keys_append.is_empty() {
        i18n_keys.extend(i18n_keys_append);
    };

    if !exclude_dirs_append.is_empty() {
        exclude_dirs.extend(exclude_dirs_append);
    };

    if !append_ignore_attributes.is_empty() {
        ignore_attributes.extend(append_ignore_attributes);
    };

    // Extract fluent keys from code
    let mut in_code_fluent_keys = extract_fluent_keys(
        code_path,
        i18n_keys,
        i18n_keys_prefix,
        exclude_dirs,
        ignore_attributes,
        ignore_kwargs,
        default_ftl_file,
        &mut statistics,
    );
    statistics.ftl_in_code_keys_count = in_code_fluent_keys.len();

    for lang in &language {
        // Import fluent keys and terms from existing FTL files
        let (mut stored_fluent_keys, mut stored_terms, mut leave_as_is) =
            import_ftl_from_dir(output_path, lang, &mut statistics);
        for fluent_key in stored_fluent_keys.values_mut() {
            fluent_key.path = fluent_key
                .path
                .strip_prefix(output_path.join(lang))
                .unwrap_or(&fluent_key.path)
                .to_path_buf();
        }
        for term in stored_terms.values_mut() {
            term.path = term
                .path
                .strip_prefix(output_path.join(lang))
                .unwrap_or(&term.path)
                .to_path_buf();
        }

        let mut keys_to_comment: HashMap<String, FluentKey> = HashMap::new();
        let mut keys_to_add: HashMap<String, FluentKey> = HashMap::new();

        // Find keys should be commented
        // Keys, that are not in code or not in their `path_` file
        // First step: find keys that have different paths
        for (key, fluent_key) in in_code_fluent_keys.iter() {
            if stored_fluent_keys.contains_key(key)
                && fluent_key.path != stored_fluent_keys.get(key).unwrap().path
            {
                keys_to_comment
                    .insert(key.clone(), stored_fluent_keys.remove(key).unwrap().clone());
                *statistics.ftl_keys_commented.get_mut(lang).unwrap() += 1;

                keys_to_add.insert(key.clone(), fluent_key.clone());
                *statistics.ftl_keys_updated.get_mut(lang).unwrap() += 1;
            } else if !stored_fluent_keys.contains_key(key) {
                keys_to_add.insert(key.clone(), fluent_key.clone());
                *statistics.ftl_keys_added.get_mut(lang).unwrap() += 1;
            } else {
                stored_fluent_keys.get_mut(key).unwrap().code_path = fluent_key.code_path.clone();
            }
        }

        // Second step: find keys that have different kwargs
        // Make copy of in_code_fluent_keys and stored_fluent_keys to check references
        let in_code_fluent_keys_copy = in_code_fluent_keys.clone();
        let stored_fluent_keys_copy = stored_fluent_keys.clone();

        // Keys that are not in code but stored keys are depends on them
        let mut depend_keys: HashSet<String> = HashSet::new();

        for (key, fluent_key) in in_code_fluent_keys.iter_mut() {
            if !stored_fluent_keys.contains_key(key) {
                continue;
            }

            let fluent_key_placeable_set = extract_kwargs(
                fluent_key,
                &mut stored_terms,
                &in_code_fluent_keys_copy,
                &mut depend_keys,
            );

            let stored_fluent_key_placeable_set = extract_kwargs(
                stored_fluent_keys.get_mut(key).unwrap(),
                &mut stored_terms,
                &stored_fluent_keys_copy,
                &mut depend_keys,
            );

            if fluent_key_placeable_set != stored_fluent_key_placeable_set {
                keys_to_comment.insert(key.clone(), stored_fluent_keys.remove(key).unwrap());
                *statistics.ftl_keys_commented.get_mut(lang).unwrap() += 1;

                keys_to_add.insert(key.clone(), fluent_key.clone());
                *statistics.ftl_keys_updated.get_mut(lang).unwrap() += 1;
            }
        }

        // Third step: find keys that are not in code
        let diff = stored_fluent_keys
            .keys()
            .filter(|key| !in_code_fluent_keys.contains_key(*key))
            .cloned()
            .collect::<Vec<String>>();
        for key in diff {
            if depend_keys.contains(&key) {
                continue;
            };

            keys_to_comment.insert(key.clone(), stored_fluent_keys.remove(&key).unwrap());
            *statistics.ftl_keys_commented.get_mut(lang).unwrap() += 1;
        }

        match comment_keys_mode {
            CommentsKeyModes::Comment => {
                for fluent_key in keys_to_comment.values_mut() {
                    comment_ftl_key(fluent_key);
                }
            }
            CommentsKeyModes::Warn => {
                for fluent_key in keys_to_comment.values_mut() {
                    keys_to_add.remove(&fluent_key.key);
                    println!(
                        "Key `{}` with such kwargs in `{}` is not in code.",
                        fluent_key.key,
                        output_path
                            .join(lang)
                            .join(fluent_key.path.clone())
                            .display()
                    );
                }
            }
        }
        // Comment Junk elements if needed
        if comment_junks {
            for fluent_key in &mut leave_as_is {
                if matches!(fluent_key.entry, FluentEntry::Junk(_)) {
                    comment_ftl_key(fluent_key);
                    *statistics.ftl_keys_commented.get_mut(lang).unwrap() += 1;
                }
            }
        }

        let mut sorted_fluent_keys = sort_fluent_keys_by_path(stored_fluent_keys);

        for (path, keys) in sort_fluent_keys_by_path(keys_to_add) {
            sorted_fluent_keys.entry(path).or_default().extend(keys);
        }

        for (path, keys) in sort_fluent_keys_by_path(keys_to_comment) {
            sorted_fluent_keys.entry(path).or_default().extend(keys);
        }

        for (path, keys) in sort_fluent_keys_by_path(stored_terms) {
            sorted_fluent_keys.entry(path).or_default().extend(keys);
        }

        let mut leave_as_is_with_path: HashMap<PathBuf, Vec<FluentKey>> = HashMap::new();

        for fluent_key in leave_as_is {
            leave_as_is_with_path
                .entry(fluent_key.path.clone())
                .or_default()
                .push(fluent_key);
        }

        for (path, keys) in &sorted_fluent_keys {
            let ftl = generate_ftl(
                keys,
                leave_as_is_with_path
                    .get(&output_path.join(lang).join(path))
                    .unwrap_or(&vec![]),
            );

            if dry_run {
                println!(
                    "[DRY-RUN] File {} has been saved. {} keys found.",
                    output_path.join(lang).join(path).display(),
                    keys.len()
                );
            } else {
                write(output_path.join(lang).join(path), ftl);
                println!(
                    "File {} has been saved. {} keys found.",
                    output_path.join(lang).join(path).display(),
                    keys.len()
                );
            }

            *statistics.ftl_stored_keys_count.get_mut(lang).unwrap() += keys
                .iter()
                .filter(|key| matches!(key.entry, FluentEntry::Message(_)))
                .count();
        }
    }

    Ok(statistics)
}

fn write(path: PathBuf, ftl: String) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, ftl).unwrap();
}
