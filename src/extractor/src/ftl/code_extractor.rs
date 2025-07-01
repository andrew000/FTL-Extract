use crate::ftl::matcher::{FluentEntry, FluentKey, I18nMatcher};
use crate::ftl::utils::ExtractionStatistics;
use anyhow::{Result, bail};
use globset::{Glob, GlobSetBuilder};
use hashbrown::{HashMap, HashSet};
use ignore::WalkBuilder;
use ignore::types::TypesBuilder;
use rayon::prelude::*;
use ruff_python_ast::visitor::source_order::SourceOrderVisitor;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

fn find_py_files(search_path: &Path, exclude_dir: HashSet<String>) -> Vec<PathBuf> {
    let mut result_paths: Vec<PathBuf> = Vec::new();

    if search_path.is_dir() {
        let mut ignore_builder = GlobSetBuilder::new();
        for exclude in exclude_dir {
            ignore_builder.add(Glob::new(exclude.as_str()).unwrap());
        }
        let ignore_set = ignore_builder.build().unwrap();

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
                    if entry.file_type().map_or(false, |ft| ft.is_file())
                        && !ignore_set.is_match(path)
                    {
                        result_paths.push(path.to_path_buf());
                    }
                }
                Err(err) => println!("ERROR: {}", err),
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
    let module =
        ruff_python_parser::parse_module(fs::read_to_string(file).unwrap().as_str()).unwrap();

    let mut matcher = I18nMatcher::new(
        file.to_path_buf(),
        default_ftl_file.to_path_buf(),
        &i18n_keys,
        &i18n_keys_prefix,
        &ignore_attributes,
        &ignore_kwargs,
    );

    matcher.visit_body(module.suite());

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
) -> Result<()> {
    // Find common keys
    let conflict_keys = current_fluent_keys
        .keys()
        .filter(|key| new_fluent_keys.contains_key(*key))
        .collect::<HashSet<_>>();

    if conflict_keys.is_empty() {
        return Ok(());
    }

    for key in conflict_keys {
        if current_fluent_keys[key].path != new_fluent_keys[key].path {
            bail!(
                "FluentKey conflict: {} in {} and {}",
                key,
                current_fluent_keys[key].path.display(),
                new_fluent_keys[key].path.display()
            )
        }
        match (&current_fluent_keys[key].entry, &new_fluent_keys[key].entry) {
            (FluentEntry::Message(a), FluentEntry::Message(b)) if a != b => {
                bail!(
                    "FluentKey conflict: {} in {} and {}",
                    key,
                    current_fluent_keys[key].path.display(),
                    new_fluent_keys[key].path.display()
                );
            }
            (a, b) if a != b => {
                bail!("FluentKey type conflict: {} ({:?} vs {:?})", key, a, b);
            }
            _ => {}
        }
    }

    Ok(())
}
pub(crate) fn extract_fluent_keys<'a>(
    path: &'a Path,
    i18n_keys: HashSet<String>,
    i18n_keys_prefix: HashSet<String>,
    exclude_dirs: HashSet<String>,
    ignore_attributes: HashSet<String>,
    ignore_kwargs: HashSet<String>,
    default_ftl_file: &'a Path,
    statistics: &mut ExtractionStatistics,
) -> HashMap<String, FluentKey> {
    let statistics_guard: Arc<RwLock<&mut ExtractionStatistics>> =
        Arc::new(RwLock::new(statistics));

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
            find_conflicts(&read_guard, &keys).unwrap();
            drop(read_guard);

            let mut write_guard = statistics_guard.write().unwrap();
            if !keys.is_empty() {
                write_guard.py_files_count += 1;
            }
            drop(write_guard);

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
