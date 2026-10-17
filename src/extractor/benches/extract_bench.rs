mod gen_fixtures;

use criterion::{Criterion, criterion_group, criterion_main};
use extractor::ftl::consts::{
    CommentsKeyModes, DEFAULT_I18N_KEYS, DEFAULT_IGNORE_KWARGS, LineEndings,
};
use extractor::ftl::ftl_extractor::{ExtractConfig, extract};
use extractor::ftl::utils::FastHashSet;
use gen_fixtures::{setup_large_fixtures, setup_project_fixtures};
use mimalloc::MiMalloc;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

// Match the `ftl` binary, which uses mimalloc as its global allocator.
#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

const LANGS_1: &[&str] = &["en"];
const LANGS_5: &[&str] = &["en", "uk", "pl", "de", "fr"];
const LANGS_10: &[&str] = &["en", "uk", "pl", "de", "fr", "es", "it", "pt", "ja", "zh"];

fn make_config(code_path: PathBuf, locales_path: PathBuf, languages: &[&str]) -> ExtractConfig {
    let mut i18n_keys = DEFAULT_I18N_KEYS.clone();
    i18n_keys.insert("self".to_string());
    i18n_keys.insert("cls".to_string());

    ExtractConfig {
        code_path,
        locales_path,
        languages: languages.iter().map(|lang| lang.to_string()).collect(),
        i18n_keys,
        i18n_keys_prefix: FastHashSet::default(),
        exclude_dirs: FastHashSet::default(),
        ignore_attributes: FastHashSet::default(),
        ignore_kwargs: DEFAULT_IGNORE_KWARGS.clone(),
        default_ftl_file: PathBuf::from("_default.ftl"),
        comment_keys_mode: CommentsKeyModes::Comment,
        line_endings: LineEndings::LF,
        // dry_run leaves the fixtures untouched, so every iteration sees the same files.
        dry_run: true,
        cache: false,
        cache_path: None,
        clear_cache: false,
        allow_parse_errors: false,
    }
}

/// Times `extract` on fixtures written once by `setup`.
///
/// The fixtures are created outside the measured loop and reused by every
/// iteration: on Windows the first open of a freshly written file costs several
/// milliseconds (antivirus scan, cache miss), which would otherwise dominate the
/// measurement. One untimed `extract` warms the OS file cache before Criterion
/// starts its own warm-up.
fn bench_extract(c: &mut Criterion, name: &str, languages: &[&str], setup: impl FnOnce(&Path)) {
    let tmp = TempDir::new().unwrap();
    setup(tmp.path());
    let config = make_config(
        tmp.path().join("code"),
        tmp.path().join("locales"),
        languages,
    );
    let stats = extract(config.clone()).unwrap();
    // The fixtures already hold every key, so the bench measures the steady-state path.
    assert!(
        stats.ftl_in_code_keys_count > 0,
        "{name}: no keys found in code"
    );
    assert!(
        stats.ftl_keys_added.values().all(|&n| n == 0)
            && stats.ftl_keys_commented.values().all(|&n| n == 0),
        "{name}: fixtures out of sync with code: {stats:?}"
    );

    c.bench_function(name, |b| b.iter(|| extract(config.clone()).unwrap()));
}

fn bench_500keys_1lang(c: &mut Criterion) {
    bench_extract(c, "500keys_1lang", LANGS_1, |dir| {
        setup_large_fixtures(dir, 500, LANGS_1)
    });
}

fn bench_500keys_5langs(c: &mut Criterion) {
    bench_extract(c, "500keys_5langs", LANGS_5, |dir| {
        setup_large_fixtures(dir, 500, LANGS_5)
    });
}

fn bench_500keys_10langs(c: &mut Criterion) {
    bench_extract(c, "500keys_10langs", LANGS_10, |dir| {
        setup_large_fixtures(dir, 500, LANGS_10)
    });
}

fn bench_2000keys_10langs(c: &mut Criterion) {
    bench_extract(c, "2000keys_10langs", LANGS_10, |dir| {
        setup_large_fixtures(dir, 2000, LANGS_10)
    });
}

/// A project-shaped fixture: 100 modules with 20 keys each (2000 keys), split
/// over several `.ftl` files via `_path=`, in 5 languages.
fn bench_project_100files_5langs(c: &mut Criterion) {
    bench_extract(c, "project_100files_5langs", LANGS_5, |dir| {
        setup_project_fixtures(dir, 100, 20, LANGS_5)
    });
}

criterion_group!(
    benches,
    bench_500keys_1lang,
    bench_500keys_5langs,
    bench_500keys_10langs,
    bench_2000keys_10langs,
    bench_project_100files_5langs,
);
criterion_main!(benches);
