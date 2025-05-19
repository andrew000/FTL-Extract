use crate::ftl::matcher::{FluentKey, I18nMatcher};
use fluent::types::AnyEq;
use globwalk::GlobWalkerBuilder;
use hashbrown::{HashMap, HashSet};
use rayon::prelude::*;
use rustpython_ast::Visitor;
use rustpython_parser::Mode;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

fn find_py_files(search_path: &Path, exclude_dir: HashSet<String>) -> Vec<PathBuf> {
    let mut result_paths: Vec<PathBuf> = Vec::new();

    let mut patterns = vec!["**/*.py".to_string()];
    patterns.extend(
        exclude_dir
            .iter()
            .map(|dir| {
                format!(
                    "!{}",
                    dir.strip_prefix(search_path.to_str().unwrap())
                        .unwrap_or(dir)
                )
            })
            .collect::<Vec<_>>(),
    );

    if search_path.is_dir() {
        for entry in GlobWalkerBuilder::from_patterns(search_path, &patterns)
            .build()
            .unwrap()
        {
            let py_file = match entry {
                Ok(entry) => entry,
                Err(_) => continue,
            };

            if py_file.file_type().is_file()
                && py_file.path().extension().unwrap_or_default() == "py"
            {
                result_paths.push(py_file.clone().into_path());
            }
        }
    } else if search_path.is_file() && search_path.extension().unwrap_or_default() == "py" {
        result_paths.push(search_path.to_path_buf());
    }

    result_paths
}
fn parse_file<'a>(
    file: &PathBuf,
    i18n_keys: &HashSet<String>,
    i18n_keys_prefix: &HashSet<String>,
    ignore_attributes: &HashSet<String>,
    ignore_kwargs: &HashSet<String>,
    default_ftl_file: &Path,
) -> HashMap<String, FluentKey> {
    let module = rustpython_parser::parse(
        fs::read_to_string(file).unwrap().as_str(),
        Mode::Module,
        "<embedded>",
    )
    .unwrap();
    let mut matcher = I18nMatcher::new(
        file.to_path_buf(),
        default_ftl_file.to_path_buf(),
        &i18n_keys,
        &i18n_keys_prefix,
        &ignore_attributes,
        &ignore_kwargs,
    );
    for node in module.module().unwrap().body {
        matcher.visit_stmt(node);
    }

    matcher.fluent_keys
}
fn post_process_fluent_keys(fluent_keys: &mut HashMap<String, FluentKey>, default_ftl_file: &Path) {
    for fluent_key in fluent_keys.values_mut() {
        if fluent_key.path.is_dir() {
            fluent_key.path.push(default_ftl_file);
        }
    }
}
fn find_conflicts(
    current_fluent_keys: &HashMap<String, FluentKey>,
    new_fluent_keys: &HashMap<String, FluentKey>,
) {
    // Find common keys
    let conflict_keys = current_fluent_keys
        .keys()
        .filter(|key| new_fluent_keys.contains_key(*key))
        .collect::<HashSet<_>>();

    if conflict_keys.is_empty() {
        return;
    }

    for key in conflict_keys {
        if current_fluent_keys[key].path != new_fluent_keys[key].path {
            panic!(
                "FluentKey conflict: {} in {} and {}",
                key,
                current_fluent_keys[key].path.display(),
                new_fluent_keys[key].path.display()
            );
        }
        if !current_fluent_keys[key]
            .message
            .equals(&new_fluent_keys[key].message)
        {
            panic!(
                "FluentKey conflict: {} in {} and {}",
                key,
                current_fluent_keys[key].path.display(),
                new_fluent_keys[key].path.display()
            );
        }
    }
}
pub(crate) fn extract_fluent_keys<'a>(
    path: &'a Path,
    i18n_keys: HashSet<String>,
    i18n_keys_prefix: HashSet<String>,
    exclude_dirs: HashSet<String>,
    ignore_attributes: HashSet<String>,
    ignore_kwargs: HashSet<String>,
    default_ftl_file: &'a Path,
) -> HashMap<String, FluentKey> {
    let fluent_keys: Arc<RwLock<HashMap<String, FluentKey>>> =
        Arc::new(RwLock::new(HashMap::new()));

    // Iterate over all files in the directory
    find_py_files(path, exclude_dirs)
        .par_iter()
        .for_each(|file| {
            let mut keys = parse_file(
                file,
                &i18n_keys,
                &i18n_keys_prefix,
                &ignore_attributes,
                &ignore_kwargs,
                default_ftl_file,
            );
            post_process_fluent_keys(&mut keys, default_ftl_file);

            let read_guard = fluent_keys.read().unwrap();
            find_conflicts(&read_guard, &keys);
            drop(read_guard);

            let mut write_guard = fluent_keys.write().unwrap();
            write_guard.extend(keys);
            drop(write_guard);
        });

    fluent_keys.read().unwrap().clone()
}

pub(crate) fn sort_fluent_keys_by_path(
    fluent_keys: HashMap<String, FluentKey>,
) -> HashMap<PathBuf, Vec<FluentKey>> {
    let mut sorted_fluent_keys: HashMap<PathBuf, Vec<FluentKey>> = HashMap::new();
    for fluent_key in fluent_keys.values() {
        sorted_fluent_keys
            .entry(fluent_key.path.to_path_buf())
            .or_default()
            .push(fluent_key.clone());
    }
    sorted_fluent_keys
}
