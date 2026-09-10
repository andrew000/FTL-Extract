use crate::ftl::cache::{
    CacheFile, CacheUpdate, cache_file_path, cache_options, cached_file_to_keys, file_cache_key,
    file_modified_ns, keys_to_cached_file, load_cache, save_cache,
};
use crate::ftl::diagnostics::{
    CodeLocation, ExtractedCode, ExtractedFluentKey, ExtractionDiagnostic, ExtractionDiagnosticKind,
};
use crate::ftl::matcher::{FluentEntry, FluentKey, I18nMatcher, merge_key_occurrence};
use crate::ftl::utils::{FastHashMap, FastHashSet};
use anyhow::Result;
use common::LineIndex;
use ignore::overrides::{Override, OverrideBuilder};
use ignore::types::TypesBuilder;
use ignore::{WalkBuilder, WalkState};
use log::error;
use memchr::memmem;
use memmap2::Mmap;
use rayon::prelude::*;
use ruff_python_ast::visitor::source_order::SourceOrderVisitor;
use std::collections::hash_map::Entry;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Clone, Debug)]
struct PyFile {
    path: PathBuf,
    size: u64,
    modified_ns: u128,
}

pub type ExcludeMatcher = Override;

pub fn build_exclude_matcher(
    root: &Path,
    exclude_dirs: &FastHashSet<String>,
) -> Result<ExcludeMatcher> {
    let mut builder = OverrideBuilder::new(root);
    for exclude in exclude_dirs {
        builder.add(&format!("!{exclude}"))?;
        if let Some(directory_exclude) = exclude.strip_suffix("/**") {
            builder.add(&format!("!{directory_exclude}"))?;
        }
    }
    Ok(builder.build()?)
}

fn find_py_files(search_path: &Path, exclude_matcher: &ExcludeMatcher) -> Vec<PyFile> {
    let mut result_paths: Vec<PyFile> = Vec::new();

    if search_path.is_dir() {
        let mut type_builder = TypesBuilder::new();
        type_builder.add("py", "*.py").unwrap();
        type_builder.select("py");

        let result_paths_parallel = Arc::new(Mutex::new(Vec::new()));
        // `.gitignore` files inside `search_path` are honored with or without a git
        // repository, as before. Registering the name as a custom ignore file (instead of
        // `git_ignore(true)` + `require_git(false)`) lets the `ignore` crate skip its
        // ancestor scan, see `common::ftl_files`.
        WalkBuilder::new(search_path)
            .parents(false)
            .ignore(false)
            .git_ignore(false)
            .git_global(false)
            .git_exclude(false)
            .add_custom_ignore_filename(".gitignore")
            .overrides(exclude_matcher.clone())
            .types(type_builder.build().unwrap())
            .build_parallel()
            .run(|| {
                let result_paths = Arc::clone(&result_paths_parallel);
                Box::new(move |result| {
                    match result {
                        Ok(entry) => {
                            let path = entry.path();
                            if entry.file_type().is_some_and(|ft| ft.is_file())
                                && let Ok(metadata) = entry.metadata()
                            {
                                result_paths.lock().unwrap().push(PyFile {
                                    path: path.to_path_buf(),
                                    size: metadata.len(),
                                    modified_ns: file_modified_ns(&metadata),
                                });
                            }
                        }
                        Err(err) => error!(target: "extractor:code", "{}", err),
                    }
                    WalkState::Continue
                })
            });
        result_paths = Arc::into_inner(result_paths_parallel)
            .unwrap()
            .into_inner()
            .unwrap();
    } else if search_path.is_file()
        && search_path.extension().unwrap_or_default() == "py"
        && !exclude_matcher.matched(search_path, false).is_ignore()
        && let Ok(metadata) = fs::metadata(search_path)
    {
        result_paths.push(PyFile {
            path: search_path.to_path_buf(),
            size: metadata.len(),
            modified_ns: file_modified_ns(&metadata),
        });
    }

    result_paths.sort_by(|a, b| a.path.cmp(&b.path));
    result_paths
}

/// Matcher settings shared by every parsed Python file.
#[derive(Clone, Copy)]
struct ParseOptions<'a> {
    i18n_keys: &'a FastHashSet<String>,
    i18n_keys_prefix: &'a FastHashSet<String>,
    ignore_attributes: &'a FastHashSet<String>,
    ignore_kwargs: &'a FastHashSet<String>,
    default_ftl_file: &'a Path,
}

type ParsedFile = (FastHashMap<String, FluentKey>, Vec<ExtractionDiagnostic>);

fn file_diagnostic(
    kind: ExtractionDiagnosticKind,
    file: &Path,
    message: String,
    (line, column): (usize, usize),
) -> ExtractionDiagnostic {
    ExtractionDiagnostic {
        kind,
        key: None,
        message,
        locations: vec![CodeLocation {
            path: file.to_path_buf(),
            line,
            column,
        }],
    }
}

fn read_error(file: &Path, err: &std::io::Error) -> ExtractionDiagnostic {
    file_diagnostic(
        ExtractionDiagnosticKind::ReadError,
        file,
        format!("Failed to read Python file: {err}"),
        (1, 1),
    )
}

fn parse_file(file: &Path, file_size: u64, options: ParseOptions<'_>) -> ParsedFile {
    if file_size == 0 {
        return (FastHashMap::default(), Vec::new());
    }

    let file_handle = match fs::File::open(file) {
        Ok(f) => f,
        Err(err) => return (FastHashMap::default(), vec![read_error(file, &err)]),
    };

    // Unsafe is required for mmap (file could change under us), but standard for tools like this.
    let mmap = unsafe {
        match Mmap::map(&file_handle) {
            Ok(m) => m,
            Err(err) => return (FastHashMap::default(), vec![read_error(file, &err)]),
        }
    };

    // Quick check: does the file contain any of the i18n keys or prefixes?
    let has_key = options
        .i18n_keys
        .iter()
        .chain(options.i18n_keys_prefix.iter())
        .any(|key| memmem::find(&mmap, key.as_bytes()).is_some());

    if !has_key {
        return (FastHashMap::default(), Vec::new());
    }

    let code = match std::str::from_utf8(&mmap) {
        Ok(c) => c,
        Err(err) => {
            // Everything before `valid_up_to` is guaranteed to be valid UTF-8.
            let valid = std::str::from_utf8(&mmap[..err.valid_up_to()]).unwrap_or_default();
            let location = LineIndex::new(valid).line_column(valid, valid.len());
            return (
                FastHashMap::default(),
                vec![file_diagnostic(
                    ExtractionDiagnosticKind::InvalidUtf8,
                    file,
                    format!("Python file is not valid UTF-8: {err}"),
                    location,
                )],
            );
        }
    };
    let module = match ruff_python_parser::parse_module(code) {
        Ok(m) => m,
        Err(err) => {
            let location = LineIndex::new(code).line_column(code, err.location.start().to_usize());
            return (
                FastHashMap::default(),
                vec![file_diagnostic(
                    ExtractionDiagnosticKind::ParseError,
                    file,
                    format!("Failed to parse Python file: {}", err.error),
                    location,
                )],
            );
        }
    };

    let mut matcher = I18nMatcher::new(
        file.to_path_buf(),
        code,
        options.default_ftl_file.to_path_buf(),
        options.i18n_keys,
        options.i18n_keys_prefix,
        options.ignore_attributes,
        options.ignore_kwargs,
    );

    matcher.visit_body(module.suite());

    (matcher.fluent_keys, matcher.diagnostics)
}

fn extract_from_file(
    file: &PyFile,
    options: ParseOptions<'_>,
    cache: Option<&CacheFile>,
) -> (
    FastHashMap<String, FluentKey>,
    Vec<ExtractionDiagnostic>,
    Option<CacheUpdate>,
) {
    let cache_key = file_cache_key(&file.path);
    let cached_file = cache.and_then(|cache| cache.files.get(&cache_key));

    if let Some(cached) = cached_file
        .filter(|cached| cached.size == file.size && cached.modified_ns == file.modified_ns)
    {
        return (cached_file_to_keys(cached), Vec::new(), None);
    }

    let (keys, diagnostics) = parse_file(&file.path, file.size, options);

    if keys.is_empty() || !diagnostics.is_empty() {
        let update = cached_file
            .is_some()
            .then_some(CacheUpdate::Remove(cache_key));
        return (keys, diagnostics, update);
    }

    let cached_file = keys_to_cached_file(file.size, file.modified_ns, &keys);

    (
        keys,
        diagnostics,
        Some(CacheUpdate::Upsert(cache_key, cached_file)),
    )
}

fn merge_fluent_key(
    target: &mut FastHashMap<String, FluentKey>,
    diagnostics: &mut Vec<ExtractionDiagnostic>,
    key: String,
    val: FluentKey,
) {
    match target.entry(key) {
        Entry::Occupied(mut entry) => {
            if let Some(conflict) = merge_key_occurrence(entry.get_mut(), val) {
                diagnostics.push(conflict);
            }
        }
        Entry::Vacant(entry) => {
            entry.insert(val);
        }
    }
}

/// Result of scanning a code tree for Fluent keys.
pub(crate) struct CodeExtraction {
    pub(crate) keys: FastHashMap<String, FluentKey>,
    pub(crate) diagnostics: Vec<ExtractionDiagnostic>,
    /// Number of Python files that were scanned.
    pub(crate) py_files_count: usize,
    /// Number of Python files that contained at least one Fluent key.
    pub(crate) py_files_with_keys: usize,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn extract_fluent_keys(
    path: &Path,
    i18n_keys: FastHashSet<String>,
    i18n_keys_prefix: FastHashSet<String>,
    exclude_dirs: &FastHashSet<String>,
    ignore_attributes: FastHashSet<String>,
    ignore_kwargs: FastHashSet<String>,
    default_ftl_file: &Path,
    use_cache: bool,
    cache_path: Option<&Path>,
    clear_cache: bool,
) -> Result<CodeExtraction> {
    let exclude_matcher = build_exclude_matcher(path, exclude_dirs)?;
    let py_files = find_py_files(path, &exclude_matcher);
    let py_files_with_keys = AtomicUsize::new(0);
    let options = cache_options(
        &i18n_keys,
        &i18n_keys_prefix,
        &ignore_attributes,
        &ignore_kwargs,
        exclude_dirs,
        default_ftl_file,
    );
    let cache_file_path = cache_file_path(cache_path);
    let cache = use_cache.then(|| load_cache(&cache_file_path, &options, clear_cache));
    let parse_options = ParseOptions {
        i18n_keys: &i18n_keys,
        i18n_keys_prefix: &i18n_keys_prefix,
        ignore_attributes: &ignore_attributes,
        ignore_kwargs: &ignore_kwargs,
        default_ftl_file,
    };

    // Parallel Map-Reduce
    let (fluent_keys, mut diagnostics, cache_updates): (
        FastHashMap<String, FluentKey>,
        Vec<ExtractionDiagnostic>,
        Vec<CacheUpdate>,
    ) = py_files
        .par_iter()
        .fold(
            || (FastHashMap::default(), Vec::new(), Vec::new()),
            |(mut acc, mut diagnostics, mut updates), file| {
                let (keys, mut file_diagnostics, cache_update) =
                    extract_from_file(file, parse_options, cache.as_ref());

                if !keys.is_empty() {
                    py_files_with_keys.fetch_add(1, Ordering::Relaxed);
                }

                diagnostics.append(&mut file_diagnostics);
                if let Some(update) = cache_update {
                    updates.push(update);
                }

                for (key, fluent_key) in keys {
                    merge_fluent_key(&mut acc, &mut diagnostics, key, fluent_key);
                }

                (acc, diagnostics, updates)
            },
        )
        .reduce(
            || (FastHashMap::default(), Vec::new(), Vec::new()),
            |a, b| {
                // Merge the smaller map into the larger one for efficiency.
                let (
                    (mut target, mut target_diagnostics, mut target_updates),
                    (source, source_diagnostics, source_updates),
                ) = if a.0.len() > b.0.len() {
                    (a, b)
                } else {
                    (b, a)
                };

                for (key, fluent_key) in source {
                    merge_fluent_key(&mut target, &mut target_diagnostics, key, fluent_key);
                }
                target_diagnostics.extend(source_diagnostics);
                target_updates.extend(source_updates);

                (target, target_diagnostics, target_updates)
            },
        );

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

    // Every field takes part, so the order (and the full `Extraction aborted` text) is the
    // same on every run once the messages themselves are deterministic.
    diagnostics.sort_by(|a, b| {
        a.key
            .cmp(&b.key)
            .then_with(|| a.message.cmp(&b.message))
            .then_with(|| a.kind.as_str().cmp(b.kind.as_str()))
            .then_with(|| a.locations.cmp(&b.locations))
    });

    Ok(CodeExtraction {
        keys: fluent_keys,
        diagnostics,
        py_files_count: py_files.len(),
        py_files_with_keys: py_files_with_keys.load(Ordering::Relaxed),
    })
}

pub fn kwargs_from_key(fluent_key: &FluentKey) -> Vec<String> {
    match fluent_key.entry.as_ref() {
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
    }
}

fn extracted_key_from_fluent_key(fluent_key: FluentKey) -> ExtractedFluentKey {
    ExtractedFluentKey {
        kwargs: kwargs_from_key(&fluent_key),
        key: fluent_key.key,
        ftl_path: fluent_key.path.as_ref().clone(),
        code_location: fluent_key.source_location,
        kwargs_unknown: fluent_key.kwargs_unknown,
    }
}

pub fn extract_code_with_diagnostics(
    path: &Path,
    i18n_keys: FastHashSet<String>,
    i18n_keys_prefix: FastHashSet<String>,
    exclude_dirs: &FastHashSet<String>,
    ignore_attributes: FastHashSet<String>,
    ignore_kwargs: FastHashSet<String>,
    default_ftl_file: &Path,
) -> Result<ExtractedCode> {
    extract_code_with_diagnostics_cached(
        path,
        i18n_keys,
        i18n_keys_prefix,
        exclude_dirs,
        ignore_attributes,
        ignore_kwargs,
        default_ftl_file,
        false,
        None,
        false,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn extract_code_with_diagnostics_cached(
    path: &Path,
    i18n_keys: FastHashSet<String>,
    i18n_keys_prefix: FastHashSet<String>,
    exclude_dirs: &FastHashSet<String>,
    ignore_attributes: FastHashSet<String>,
    ignore_kwargs: FastHashSet<String>,
    default_ftl_file: &Path,
    use_cache: bool,
    cache_path: Option<&Path>,
    clear_cache: bool,
) -> Result<ExtractedCode> {
    let extraction = extract_fluent_keys(
        path,
        i18n_keys,
        i18n_keys_prefix,
        exclude_dirs,
        ignore_attributes,
        ignore_kwargs,
        default_ftl_file,
        use_cache,
        cache_path,
        clear_cache,
    )?;

    let mut keys = extraction
        .keys
        .into_values()
        .map(extracted_key_from_fluent_key)
        .collect::<Vec<_>>();
    keys.sort_by(|a, b| a.key.cmp(&b.key));

    Ok(ExtractedCode {
        keys,
        diagnostics: extraction.diagnostics,
        py_files_count: extraction.py_files_count,
    })
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
    use super::{ParseOptions, extract_fluent_keys};
    use crate::ftl::consts;
    use crate::ftl::diagnostics::ExtractionDiagnosticKind;
    use crate::ftl::matcher::{FluentEntry, FluentKey};
    use crate::ftl::utils::{FastHashMap, FastHashSet};
    use pretty_assertions::assert_eq;
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, LazyLock};
    use tempfile::TempDir;

    const DEFAULT_PY: &str = r#"
from .stub import I18nContext

i18n = I18nContext()

i18n.text()
i18n.text.kwargs(kwarg1="value1", kwarg2="value2")
i18n.text.args.term()
i18n.text.args.term.args(kwarg1="value1", kwarg2="value2")
i18n.text.message_reference()
i18n.text.message_reference.args(kwarg1="value1", kwarg2="value2")
i18n.text.selector(selector=1)
i18n.text.selector.selectors(selector=1)
i18n.text.selector.kwargs(selector=1, kwarg1="value1", kwarg2="value2")
i18n.text.selector.reference.selector.kwargs.terms(
    selector=1,
    kwarg1="value1",
    kwarg2="value2",
)
"#;

    const CLASSLIKE_PY: &str = r#"
from typing import Any


class I18nContext:
    def get(self, *_, **__) -> None: ...

    def __getattr__(self, item: str) -> Any: ...

    def __call__(self, *_, **__) -> None: ...


class Mock:
    cls_i18n: I18nContext

    def __init__(self, i18n: I18nContext) -> None:
        self.i18n = i18n

    def self_i18n(self) -> None:
        self.i18n.self.key(some_kwarg="...", _path="classlike.ftl")
        self.i18n.get("self-get-key", some_kwarg="...", _path="classlike.ftl")

    @classmethod
    def cls_i18n(cls) -> None:
        cls.cls_i18n.cls.key(some_kwarg="...", _path="classlike.ftl")
        cls.cls_i18n.get("cls-get-key", some_kwarg="...", _path="classlike.ftl")
"#;

    static EMPTY: LazyLock<FastHashSet<String>> = LazyLock::new(FastHashSet::default);
    static I18N_ONLY: LazyLock<FastHashSet<String>> =
        LazyLock::new(|| FastHashSet::from_iter(["i18n".to_string()]));
    static DEFAULT_FTL: LazyLock<PathBuf> = LazyLock::new(|| PathBuf::from("_default.ftl"));

    fn i18n_options() -> ParseOptions<'static> {
        ParseOptions {
            i18n_keys: &I18N_ONLY,
            i18n_keys_prefix: &EMPTY,
            ignore_attributes: &EMPTY,
            ignore_kwargs: &EMPTY,
            default_ftl_file: &DEFAULT_FTL,
        }
    }

    fn write_python_fixture(dir: &Path) {
        std::fs::write(dir.join("__init__.py"), "").unwrap();
        std::fs::write(dir.join("default.py"), DEFAULT_PY).unwrap();
        std::fs::write(dir.join("classlike.py"), CLASSLIKE_PY).unwrap();
    }

    fn empty_exclude_matcher(root: &Path) -> super::ExcludeMatcher {
        super::build_exclude_matcher(root, &FastHashSet::default()).unwrap()
    }

    #[test]
    fn test_find_py_files_dir() {
        let temp = TempDir::new().unwrap();
        let code_path = temp.path().join("py");
        std::fs::create_dir_all(&code_path).unwrap();
        write_python_fixture(&code_path);

        let py_files = super::find_py_files(&code_path, &empty_exclude_matcher(&code_path));
        assert_eq!(py_files.len(), 3);
    }

    #[test]
    fn test_find_py_files_honors_gitignore_without_a_git_repository() {
        let temp = TempDir::new().unwrap();
        let code_path = temp.path().join("py");
        std::fs::create_dir_all(code_path.join("generated")).unwrap();
        write_python_fixture(&code_path);
        std::fs::write(code_path.join(".gitignore"), "generated/\n").unwrap();
        std::fs::write(
            code_path.join("generated").join("gen.py"),
            r#"i18n.get("generated")"#,
        )
        .unwrap();
        // Rules above `code_path` are not consulted.
        std::fs::write(temp.path().join(".gitignore"), "*.py\n").unwrap();

        let py_files = super::find_py_files(&code_path, &empty_exclude_matcher(&code_path));

        assert_eq!(py_files.len(), 3);
        assert!(py_files.iter().all(|file| !file.path.ends_with("gen.py")));
    }

    #[test]
    fn test_find_py_files_applies_exclude_dirs() {
        let temp = TempDir::new().unwrap();
        let code_path = temp.path().join("py");
        let ignored_dir = code_path.join(".venv");
        std::fs::create_dir_all(&ignored_dir).unwrap();
        write_python_fixture(&code_path);
        std::fs::write(ignored_dir.join("ignored.py"), r#"i18n.get("ignored")"#).unwrap();

        let excludes = FastHashSet::from_iter(["**/.venv/**".to_string()]);
        let matcher = super::build_exclude_matcher(&code_path, &excludes).unwrap();
        let py_files = super::find_py_files(&code_path, &matcher);

        assert_eq!(py_files.len(), 3);
        assert!(py_files.iter().all(|file| {
            !file
                .path
                .components()
                .any(|part| part.as_os_str() == ".venv")
        }));
    }

    #[test]
    fn test_find_py_files_file() {
        let temp = TempDir::new().unwrap();
        let code_dir = temp.path().join("py");
        std::fs::create_dir_all(&code_dir).unwrap();
        write_python_fixture(&code_dir);

        let code_path = code_dir.join("default.py");
        let py_files = super::find_py_files(&code_path, &empty_exclude_matcher(&code_path));
        assert_eq!(py_files.len(), 1);
        assert_eq!(py_files[0].path, code_path);
    }

    #[test]
    fn test_extract_fluent_keys() {
        let temp = TempDir::new().unwrap();
        let code_path = temp.path().join("py");
        std::fs::create_dir_all(&code_path).unwrap();
        write_python_fixture(&code_path);

        let mut key_prefixes = consts::DEFAULT_I18N_KEYS.clone();
        key_prefixes.insert("self".to_string());
        key_prefixes.insert("cls".to_string());

        let extraction = extract_fluent_keys(
            &code_path,
            key_prefixes.clone(),
            FastHashSet::default(),
            &FastHashSet::default(),
            FastHashSet::default(),
            FastHashSet::default(),
            &PathBuf::from("locales/en.ftl"),
            false,
            None,
            false,
        )
        .unwrap();
        let fluent_keys = extraction.keys;

        eprintln!("Extracted Fluent Keys: {:?}", fluent_keys.keys());

        assert!(extraction.diagnostics.is_empty());
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
        assert_eq!(extraction.py_files_count, 3);
        assert_eq!(extraction.py_files_with_keys, 2);
    }

    #[test]
    fn test_extract_fluent_keys_matcher_edges() {
        let temp = TempDir::new().unwrap();
        let code_path = temp.path().join("app.py");
        std::fs::write(
            &code_path,
            r#"
L("named-key", kwarg="value", when=True)
i18n.get()
i18n.get(dynamic_key)
i18n.get("folder-key", _path="nested")
i18n.get("file-key", _path="custom.ftl")
i18n.set_locale("uk")
self.i18n.get("prefixed-get")
self.i18n.menu.open()
unknown("ignored")
"#,
        )
        .unwrap();

        let mut i18n_keys = FastHashSet::default();
        i18n_keys.insert("i18n".to_string());
        i18n_keys.insert("L".to_string());

        let mut prefixes = FastHashSet::default();
        prefixes.insert("self".to_string());

        let mut ignore_attributes = FastHashSet::default();
        ignore_attributes.insert("set_locale".to_string());

        let mut ignore_kwargs = FastHashSet::default();
        ignore_kwargs.insert("when".to_string());

        let extraction = extract_fluent_keys(
            &code_path,
            i18n_keys,
            prefixes,
            &FastHashSet::default(),
            ignore_attributes,
            ignore_kwargs,
            &PathBuf::from("_default.ftl"),
            false,
            None,
            false,
        )
        .unwrap();
        let fluent_keys = extraction.keys;

        assert_eq!(extraction.py_files_with_keys, 1);
        assert!(fluent_keys.contains_key("named-key"));
        assert!(fluent_keys.contains_key("folder-key"));
        assert!(fluent_keys.contains_key("file-key"));
        assert!(fluent_keys.contains_key("prefixed-get"));
        assert!(fluent_keys.contains_key("menu-open"));
        assert!(!fluent_keys.contains_key("set-locale"));

        assert_eq!(
            fluent_keys["folder-key"].path.as_ref(),
            &PathBuf::from("nested").join("_default.ftl")
        );
        assert_eq!(
            fluent_keys["file-key"].path.as_ref(),
            &PathBuf::from("custom.ftl")
        );

        let FluentEntry::Message(message) = fluent_keys["named-key"].entry.as_ref() else {
            panic!("expected message")
        };
        let elements = &message.value.as_ref().unwrap().elements;
        assert_eq!(elements.len(), 2);
    }

    #[test]
    fn test_extract_code_with_diagnostics_includes_source_location() {
        let temp = TempDir::new().unwrap();
        let code_path = temp.path().join("app.py");
        std::fs::write(
            &code_path,
            r#"def handler():
    i18n.get("hello", name="Andrew")
"#,
        )
        .unwrap();

        let extracted = super::extract_code_with_diagnostics(
            &code_path,
            I18N_ONLY.clone(),
            FastHashSet::default(),
            &FastHashSet::default(),
            FastHashSet::default(),
            FastHashSet::default(),
            &PathBuf::from("_default.ftl"),
        )
        .unwrap();

        assert!(extracted.diagnostics.is_empty());
        assert_eq!(extracted.py_files_count, 1);
        assert_eq!(extracted.keys.len(), 1);
        assert_eq!(extracted.keys[0].key, "hello");
        assert_eq!(extracted.keys[0].ftl_path, PathBuf::from("_default.ftl"));
        assert_eq!(extracted.keys[0].kwargs, vec!["name"]);

        let location = extracted.keys[0].code_location.as_ref().unwrap();
        assert_eq!(location.path, code_path);
        assert_eq!(location.line, 2);
        assert_eq!(location.column, 5);
    }

    #[test]
    fn test_extract_code_with_diagnostics_reports_same_file_conflict() {
        let temp = TempDir::new().unwrap();
        let code_path = temp.path().join("app.py");
        std::fs::write(
            &code_path,
            r#"i18n.get("hello", _path="one.ftl")
i18n.get("hello", _path="two.ftl")
"#,
        )
        .unwrap();

        let extracted = super::extract_code_with_diagnostics(
            &code_path,
            I18N_ONLY.clone(),
            FastHashSet::default(),
            &FastHashSet::default(),
            FastHashSet::default(),
            FastHashSet::default(),
            &PathBuf::from("_default.ftl"),
        )
        .unwrap();

        assert_eq!(extracted.keys.len(), 1);
        assert_eq!(extracted.diagnostics.len(), 1);
        assert_eq!(
            extracted.diagnostics[0].kind,
            ExtractionDiagnosticKind::KeyPathConflict
        );
        assert_eq!(extracted.diagnostics[0].key.as_deref(), Some("hello"));
        assert_eq!(extracted.diagnostics[0].locations.len(), 2);
        assert_eq!(extracted.diagnostics[0].locations[0].line, 1);
        assert_eq!(extracted.diagnostics[0].locations[1].line, 2);
    }

    #[test]
    fn test_extract_code_with_diagnostics_reports_cross_file_conflict() {
        let temp = TempDir::new().unwrap();
        let code_dir = temp.path().join("py");
        std::fs::create_dir_all(&code_dir).unwrap();
        std::fs::write(code_dir.join("a.py"), r#"i18n.get("hello", name="Andrew")"#).unwrap();
        std::fs::write(code_dir.join("b.py"), r#"i18n.get("hello", title="Dr")"#).unwrap();

        let extracted = super::extract_code_with_diagnostics(
            &code_dir,
            I18N_ONLY.clone(),
            FastHashSet::default(),
            &FastHashSet::default(),
            FastHashSet::default(),
            FastHashSet::default(),
            &PathBuf::from("_default.ftl"),
        )
        .unwrap();

        assert_eq!(extracted.keys.len(), 1);
        assert_eq!(extracted.py_files_count, 2);
        assert_eq!(extracted.diagnostics.len(), 1);
        assert_eq!(
            extracted.diagnostics[0].kind,
            ExtractionDiagnosticKind::KeyMessageConflict
        );
        assert_eq!(extracted.diagnostics[0].key.as_deref(), Some("hello"));
        assert_eq!(extracted.diagnostics[0].locations.len(), 2);
    }

    #[test]
    fn test_parse_file_skips_empty_and_keyless_files() {
        let temp = TempDir::new().unwrap();
        let empty = temp.path().join("empty.py");
        let no_key = temp.path().join("no_key.py");
        std::fs::write(&empty, "").unwrap();
        std::fs::write(&no_key, "print('hello')").unwrap();

        for path in [&empty, &no_key] {
            let (keys, diagnostics) =
                super::parse_file(path, std::fs::metadata(path).unwrap().len(), i18n_options());
            assert!(keys.is_empty());
            assert!(diagnostics.is_empty(), "{}", path.display());
        }
    }

    #[test]
    fn test_parse_file_skips_calls_without_a_positional_string_key() {
        // Every line before `i18n.get("ok")` used to panic on `find_positional(0).unwrap()`
        // (`*args`, `**kwargs` and keyword-only calls are not "empty" arguments) or on
        // `attrs.last().unwrap()` (`self.i18n("...")` with `self` as a prefix leaves no
        // attributes). None of them is a key, except the direct prefixed call, which is
        // extracted like `i18n("...")`.
        let temp = TempDir::new().unwrap();
        let file = temp.path().join("wrappers.py");
        std::fs::write(
            &file,
            r#"
i18n.get(*args)
i18n.get(**kwargs)
i18n.get(key="x")
L(name="x")
L(*args)
L(**kwargs)
self.i18n(*args)
self.i18n(key="x")
self.i18n("self-call")
i18n("direct")
i18n.get("ok")
"#,
        )
        .unwrap();

        let i18n_keys = FastHashSet::from_iter(["i18n".to_string(), "L".to_string()]);
        let prefixes = FastHashSet::from_iter(["self".to_string()]);
        let options = ParseOptions {
            i18n_keys: &i18n_keys,
            i18n_keys_prefix: &prefixes,
            ignore_attributes: &EMPTY,
            ignore_kwargs: &EMPTY,
            default_ftl_file: &DEFAULT_FTL,
        };

        let (keys, diagnostics) =
            super::parse_file(&file, std::fs::metadata(&file).unwrap().len(), options);

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        let mut found: Vec<&str> = keys.keys().map(String::as_str).collect();
        found.sort_unstable();
        assert_eq!(found, ["direct", "ok", "self-call"]);
    }

    #[test]
    fn test_parse_file_reports_invalid_utf8() {
        let temp = TempDir::new().unwrap();
        let invalid_utf8 = temp.path().join("invalid_utf8.py");
        std::fs::write(&invalid_utf8, b"x = 1\ni18n.get('\xFF')").unwrap();

        let (keys, diagnostics) = super::parse_file(
            &invalid_utf8,
            std::fs::metadata(&invalid_utf8).unwrap().len(),
            i18n_options(),
        );

        assert!(keys.is_empty());
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].kind, ExtractionDiagnosticKind::InvalidUtf8);
        assert_eq!(diagnostics[0].key, None);
        assert!(
            diagnostics[0]
                .message
                .starts_with("Python file is not valid UTF-8: "),
            "{}",
            diagnostics[0].message
        );
        assert_eq!(diagnostics[0].locations.len(), 1);
        assert_eq!(diagnostics[0].locations[0].path, invalid_utf8);
        assert_eq!(diagnostics[0].locations[0].line, 2);
        assert_eq!(diagnostics[0].locations[0].column, 11);
    }

    #[test]
    fn test_parse_file_reports_python_syntax_errors_with_location() {
        let temp = TempDir::new().unwrap();
        let invalid_python = temp.path().join("invalid_python.py");
        std::fs::write(&invalid_python, "x = 1\ni18n.get(\"ok\")\ni18n.get(\n").unwrap();

        let (keys, diagnostics) = super::parse_file(
            &invalid_python,
            std::fs::metadata(&invalid_python).unwrap().len(),
            i18n_options(),
        );

        assert!(keys.is_empty(), "no keys are trusted from a broken file");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].kind, ExtractionDiagnosticKind::ParseError);
        assert_eq!(diagnostics[0].key, None);
        assert!(
            diagnostics[0]
                .message
                .starts_with("Failed to parse Python file: "),
            "{}",
            diagnostics[0].message
        );
        assert_eq!(diagnostics[0].locations.len(), 1);
        assert_eq!(diagnostics[0].locations[0].path, invalid_python);
        assert!(diagnostics[0].locations[0].line >= 3);
    }

    #[test]
    fn test_parse_file_reports_unreadable_files() {
        let temp = TempDir::new().unwrap();
        let missing = temp.path().join("missing.py");

        let (keys, diagnostics) = super::parse_file(&missing, 1, i18n_options());

        assert!(keys.is_empty());
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].kind, ExtractionDiagnosticKind::ReadError);
        assert!(
            diagnostics[0]
                .message
                .starts_with("Failed to read Python file: "),
            "{}",
            diagnostics[0].message
        );
        assert_eq!(diagnostics[0].locations[0].path, missing);
    }

    #[test]
    fn test_extract_fluent_keys_collects_file_errors_from_directory() {
        let temp = TempDir::new().unwrap();
        let code_dir = temp.path().join("py");
        std::fs::create_dir_all(&code_dir).unwrap();
        std::fs::write(code_dir.join("good.py"), r#"i18n.get("hello")"#).unwrap();
        std::fs::write(code_dir.join("broken.py"), "i18n.get(").unwrap();

        let extraction = extract_fluent_keys(
            &code_dir,
            I18N_ONLY.clone(),
            FastHashSet::default(),
            &FastHashSet::default(),
            FastHashSet::default(),
            FastHashSet::default(),
            &PathBuf::from("_default.ftl"),
            false,
            None,
            false,
        )
        .unwrap();

        assert!(extraction.keys.contains_key("hello"));
        assert_eq!(extraction.diagnostics.len(), 1);
        assert!(extraction.diagnostics[0].is_file_error());
        assert_eq!(
            extraction.diagnostics[0].locations[0].path,
            code_dir.join("broken.py")
        );
    }

    #[test]
    fn test_extract_from_file_cache_hit_and_stale_remove() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("app.py");
        std::fs::write(&path, r#"i18n.get("hello")"#).unwrap();
        let metadata = std::fs::metadata(&path).unwrap();
        let py_file = super::PyFile {
            path: path.clone(),
            size: metadata.len(),
            modified_ns: super::file_modified_ns(&metadata),
        };

        let (keys, diagnostics, update) = super::extract_from_file(&py_file, i18n_options(), None);
        assert!(keys.contains_key("hello"));
        assert!(diagnostics.is_empty());
        assert!(matches!(update, Some(super::CacheUpdate::Upsert(_, _))));

        let mut cache = super::CacheFile {
            schema_version: 1,
            options: super::cache_options(
                &I18N_ONLY,
                &FastHashSet::default(),
                &FastHashSet::default(),
                &FastHashSet::default(),
                &FastHashSet::default(),
                &PathBuf::from("_default.ftl"),
            ),
            files: FastHashMap::default(),
        };
        let Some(super::CacheUpdate::Upsert(cache_key, cached_file)) = update else {
            panic!("expected cache upsert")
        };
        cache.files.insert(cache_key, cached_file);

        let (cached_keys, _, cached_update) =
            super::extract_from_file(&py_file, i18n_options(), Some(&cache));
        assert!(cached_keys.contains_key("hello"));
        assert!(cached_update.is_none());

        std::fs::write(&path, "print('hello')").unwrap();
        let metadata = std::fs::metadata(&path).unwrap();
        let changed = super::PyFile {
            path: path.clone(),
            size: metadata.len(),
            modified_ns: super::file_modified_ns(&metadata),
        };
        let (empty_keys, _, remove_update) =
            super::extract_from_file(&changed, i18n_options(), Some(&cache));
        assert!(empty_keys.is_empty());
        assert!(matches!(remove_update, Some(super::CacheUpdate::Remove(_))));

        // A file that fails to parse is never cached, and drops a stale cache entry.
        std::fs::write(&path, "i18n.get(").unwrap();
        let metadata = std::fs::metadata(&path).unwrap();
        let broken = super::PyFile {
            path,
            size: metadata.len(),
            modified_ns: super::file_modified_ns(&metadata),
        };
        let (broken_keys, broken_diagnostics, broken_update) =
            super::extract_from_file(&broken, i18n_options(), Some(&cache));
        assert!(broken_keys.is_empty());
        assert_eq!(broken_diagnostics.len(), 1);
        assert!(matches!(broken_update, Some(super::CacheUpdate::Remove(_))));
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

    #[test]
    fn test_cache_is_not_shared_between_different_exclude_dirs() {
        let temp = TempDir::new().unwrap();
        let code_dir = temp.path().join("py");
        let cache_dir = temp.path().join("cache");
        std::fs::create_dir_all(code_dir.join("tests")).unwrap();
        std::fs::write(code_dir.join("app.py"), r#"i18n.get("hello")"#).unwrap();
        std::fs::write(
            code_dir.join("tests").join("test_app.py"),
            r#"i18n.get("only-in-tests")"#,
        )
        .unwrap();

        let run = |exclude_dirs: &FastHashSet<String>| {
            extract_fluent_keys(
                &code_dir,
                I18N_ONLY.clone(),
                FastHashSet::default(),
                exclude_dirs,
                FastHashSet::default(),
                FastHashSet::default(),
                &DEFAULT_FTL,
                true,
                Some(&cache_dir),
                false,
            )
            .unwrap()
        };

        let everything = FastHashSet::default();
        let without_tests = FastHashSet::from_iter(["tests/**".to_string()]);

        assert_eq!(run(&everything).keys.len(), 2);
        assert_eq!(run(&without_tests).keys.len(), 1);

        // The second run rebuilt the cache with its own options: the entry for the excluded
        // test file is gone rather than carried over from the first run.
        let options = super::cache_options(
            &I18N_ONLY,
            &FastHashSet::default(),
            &FastHashSet::default(),
            &FastHashSet::default(),
            &without_tests,
            &DEFAULT_FTL,
        );
        let cache = super::load_cache(&super::cache_file_path(Some(&cache_dir)), &options, false);
        assert_eq!(cache.files.len(), 1);
        assert!(cache.files.keys().all(|file| file.ends_with("app.py")));

        assert_eq!(run(&everything).keys.len(), 2);
    }

    #[test]
    fn test_same_kwargs_in_a_different_order_across_files_is_one_key() {
        let temp = TempDir::new().unwrap();
        let code_dir = temp.path().join("py");
        std::fs::create_dir_all(&code_dir).unwrap();
        std::fs::write(code_dir.join("a.py"), r#"i18n.get("order", a=1, b=2)"#).unwrap();
        std::fs::write(code_dir.join("b.py"), r#"i18n.get("order", b=2, a=1)"#).unwrap();
        std::fs::write(code_dir.join("c.py"), r#"i18n.get("order", b=2, a=1)"#).unwrap();

        // Files are reduced in parallel; the result must not depend on which one wins.
        for _ in 0..5 {
            let extracted = super::extract_code_with_diagnostics(
                &code_dir,
                I18N_ONLY.clone(),
                FastHashSet::default(),
                &FastHashSet::default(),
                FastHashSet::default(),
                FastHashSet::default(),
                &PathBuf::from("_default.ftl"),
            )
            .unwrap();

            assert!(
                extracted.diagnostics.is_empty(),
                "{:?}",
                extracted.diagnostics
            );
            assert_eq!(extracted.keys.len(), 1);
            assert_eq!(extracted.keys[0].key, "order");
            assert_eq!(extracted.keys[0].kwargs, vec!["a", "b"]);
        }
    }

    #[test]
    fn test_cross_file_conflict_message_is_readable() {
        let temp = TempDir::new().unwrap();
        let code_dir = temp.path().join("py");
        std::fs::create_dir_all(&code_dir).unwrap();
        let a_py = code_dir.join("a.py");
        let b_py = code_dir.join("b.py");
        std::fs::write(&a_py, r#"i18n.get("order", a=1, b=2)"#).unwrap();
        std::fs::write(&b_py, r#"i18n.get("order", a=1, c=3)"#).unwrap();

        let extracted = super::extract_code_with_diagnostics(
            &code_dir,
            I18N_ONLY.clone(),
            FastHashSet::default(),
            &FastHashSet::default(),
            FastHashSet::default(),
            FastHashSet::default(),
            &PathBuf::from("_default.ftl"),
        )
        .unwrap();

        assert_eq!(extracted.diagnostics.len(), 1);
        let diagnostic = &extracted.diagnostics[0];
        assert_eq!(
            diagnostic.kind,
            ExtractionDiagnosticKind::KeyMessageConflict
        );
        assert_eq!(diagnostic.key.as_deref(), Some("order"));
        assert_eq!(diagnostic.locations.len(), 2);

        // The sides are ordered by call site, so the text does not depend on which file the
        // parallel fold reduced first.
        assert_eq!(
            diagnostic.message,
            "Fluent key order is used with different keyword arguments: a, b and a, c"
        );
        assert_eq!(
            diagnostic.to_string(),
            format!(
                "[key-message-conflict] Fluent key order is used with different keyword arguments: a, b and a, c ({}:1:1, {}:1:1)",
                a_py.display(),
                b_py.display()
            )
        );
    }

    #[test]
    fn test_double_star_kwargs_in_another_file_mark_the_extracted_key() {
        let temp = TempDir::new().unwrap();
        let code_dir = temp.path().join("py");
        std::fs::create_dir_all(&code_dir).unwrap();
        let b_py = code_dir.join("b.py");
        std::fs::write(code_dir.join("a.py"), r#"i18n.get("welcome")"#).unwrap();
        std::fs::write(&b_py, r#"i18n.get("welcome", **data)"#).unwrap();

        let extracted = super::extract_code_with_diagnostics(
            &code_dir,
            I18N_ONLY.clone(),
            FastHashSet::default(),
            &FastHashSet::default(),
            FastHashSet::default(),
            FastHashSet::default(),
            &PathBuf::from("_default.ftl"),
        )
        .unwrap();

        assert!(
            extracted.diagnostics.is_empty(),
            "{:?}",
            extracted.diagnostics
        );
        assert_eq!(extracted.keys.len(), 1);
        assert!(extracted.keys[0].kwargs.is_empty());
        let unknown = extracted.keys[0].kwargs_unknown.as_ref().unwrap();
        assert_eq!(unknown.path, b_py);
        assert_eq!((unknown.line, unknown.column), (1, 1));
    }

    #[test]
    fn test_cross_file_conflict_diagnostics_are_identical_across_runs() {
        let temp = TempDir::new().unwrap();
        let code_dir = temp.path().join("py");
        std::fs::create_dir_all(&code_dir).unwrap();
        // Two conflicts on two keys, so both the per-diagnostic text and the overall order
        // have to be stable.
        std::fs::write(
            code_dir.join("a.py"),
            "i18n.get(\"order\", a=1, b=2)\ni18n.get(\"title\", x=1)\n",
        )
        .unwrap();
        std::fs::write(
            code_dir.join("b.py"),
            "i18n.get(\"title\", y=2)\ni18n.get(\"order\", a=1, c=3)\n",
        )
        .unwrap();

        let run = || {
            super::extract_code_with_diagnostics(
                &code_dir,
                I18N_ONLY.clone(),
                FastHashSet::default(),
                &FastHashSet::default(),
                FastHashSet::default(),
                FastHashSet::default(),
                &PathBuf::from("_default.ftl"),
            )
            .unwrap()
            .diagnostics
        };

        let first = run();
        assert_eq!(first.len(), 2, "{first:?}");
        let rendered = first.iter().map(ToString::to_string).collect::<Vec<_>>();
        let a = code_dir.join("a.py").display().to_string();
        let b = code_dir.join("b.py").display().to_string();
        assert_eq!(
            rendered,
            vec![
                format!(
                    "[key-message-conflict] Fluent key order is used with different keyword arguments: a, b and a, c ({a}:1:1, {b}:2:1)"
                ),
                format!(
                    "[key-message-conflict] Fluent key title is used with different keyword arguments: x and y ({a}:2:1, {b}:1:1)"
                ),
            ]
        );

        for _ in 0..7 {
            assert_eq!(run(), first);
        }
    }

    #[test]
    fn test_double_star_calls_across_files_never_conflict_and_keep_the_explicit_call() {
        // (a.py, b.py, expected kwargs, file whose call is kept, file with the first `**`)
        let cases: [(&str, &str, &[&str], &str, &str); 4] = [
            (
                r#"i18n.get("welcome", name=x)"#,
                r#"i18n.get("welcome", **data)"#,
                &["name"],
                "a.py",
                "b.py",
            ),
            (
                r#"i18n.get("welcome", **data)"#,
                r#"i18n.get("welcome", name=x)"#,
                &["name"],
                "b.py",
                "a.py",
            ),
            (
                r#"i18n.get("welcome", b=2, **data)"#,
                r#"i18n.get("welcome", a=1, **data)"#,
                &["b"],
                "a.py",
                "a.py",
            ),
            (
                r#"i18n.get("welcome", **data)"#,
                r#"i18n.get("welcome", **other)"#,
                &[],
                "a.py",
                "a.py",
            ),
        ];

        for (a, b, kwargs, kept_in, double_star_in) in cases {
            let temp = TempDir::new().unwrap();
            let code_dir = temp.path().join("py");
            std::fs::create_dir_all(&code_dir).unwrap();
            std::fs::write(code_dir.join("a.py"), a).unwrap();
            std::fs::write(code_dir.join("b.py"), b).unwrap();

            // The parallel fold can reduce either file first; the result must not depend on it.
            for _ in 0..5 {
                let extracted = super::extract_code_with_diagnostics(
                    &code_dir,
                    I18N_ONLY.clone(),
                    FastHashSet::default(),
                    &FastHashSet::default(),
                    FastHashSet::default(),
                    FastHashSet::default(),
                    &PathBuf::from("_default.ftl"),
                )
                .unwrap();

                assert!(
                    extracted.diagnostics.is_empty(),
                    "[{a} / {b}] {:?}",
                    extracted.diagnostics
                );
                assert_eq!(extracted.keys.len(), 1, "[{a} / {b}]");
                let key = &extracted.keys[0];
                assert_eq!(key.kwargs, kwargs, "[{a} / {b}]");
                assert_eq!(
                    key.code_location.as_ref().unwrap().path,
                    code_dir.join(kept_in),
                    "[{a} / {b}] kept call"
                );
                assert_eq!(
                    key.kwargs_unknown.as_ref().unwrap().path,
                    code_dir.join(double_star_in),
                    "[{a} / {b}] first ** call"
                );
            }
        }
    }

    #[test]
    fn test_explicit_calls_still_conflict_across_files_next_to_a_double_star_call() {
        let temp = TempDir::new().unwrap();
        let code_dir = temp.path().join("py");
        std::fs::create_dir_all(&code_dir).unwrap();
        std::fs::write(code_dir.join("a.py"), r#"i18n.get("order", a=1, b=2)"#).unwrap();
        std::fs::write(code_dir.join("b.py"), r#"i18n.get("order", **data)"#).unwrap();
        std::fs::write(code_dir.join("c.py"), r#"i18n.get("order", a=1, c=3)"#).unwrap();

        for _ in 0..5 {
            let extracted = super::extract_code_with_diagnostics(
                &code_dir,
                I18N_ONLY.clone(),
                FastHashSet::default(),
                &FastHashSet::default(),
                FastHashSet::default(),
                FastHashSet::default(),
                &PathBuf::from("_default.ftl"),
            )
            .unwrap();

            // Only the two explicit calls take part; `b.py` is neither named nor a side.
            assert_eq!(
                extracted.diagnostics.len(),
                1,
                "{:?}",
                extracted.diagnostics
            );
            let diagnostic = &extracted.diagnostics[0];
            assert_eq!(
                diagnostic.kind,
                ExtractionDiagnosticKind::KeyMessageConflict
            );
            assert_eq!(
                diagnostic.message,
                "Fluent key order is used with different keyword arguments: a, b and a, c"
            );
            assert_eq!(
                diagnostic
                    .locations
                    .iter()
                    .map(|location| location.path.clone())
                    .collect::<Vec<_>>(),
                vec![code_dir.join("a.py"), code_dir.join("c.py")]
            );
            assert_eq!(
                extracted.keys[0].kwargs_unknown.as_ref().unwrap().path,
                code_dir.join("b.py")
            );
        }
    }
}
