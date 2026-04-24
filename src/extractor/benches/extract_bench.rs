mod gen_fixtures;

use criterion::{Criterion, criterion_group, criterion_main};
use extractor::ftl::consts::{
    CommentsKeyModes, DEFAULT_I18N_KEYS, DEFAULT_IGNORE_KWARGS, LineEndings,
};
use extractor::ftl::ftl_extractor::{ExtractConfig, extract};
use extractor::ftl::utils::FastHashSet;
use gen_fixtures::setup_large_fixtures;
use std::path::PathBuf;
use tempfile::TempDir;

fn make_config(code_path: PathBuf, output_path: PathBuf, languages: Vec<&str>) -> ExtractConfig {
    let mut i18n_keys = DEFAULT_I18N_KEYS.clone();
    i18n_keys.insert("self".to_string());
    i18n_keys.insert("cls".to_string());

    ExtractConfig {
        code_path,
        output_path,
        languages: languages.into_iter().map(String::from).collect(),
        i18n_keys,
        i18n_keys_prefix: FastHashSet::default(),
        exclude_dirs: FastHashSet::default(),
        ignore_attributes: FastHashSet::default(),
        ignore_kwargs: DEFAULT_IGNORE_KWARGS.clone(),
        default_ftl_file: PathBuf::from("_default.ftl"),
        comment_junks: true,
        comment_keys_mode: CommentsKeyModes::Comment,
        line_endings: LineEndings::LF,
        dry_run: true, // dry_run to avoid I/O noise
        cache: false,
        cache_path: None,
        clear_cache: false,
    }
}

fn bench_500keys_1lang(c: &mut Criterion) {
    c.bench_function("500keys_1lang", |b| {
        b.iter_with_setup(
            || {
                let tmp = TempDir::new().unwrap();
                setup_large_fixtures(tmp.path(), 500, &["en"]);
                let config = make_config(
                    tmp.path().join("code"),
                    tmp.path().join("locales"),
                    vec!["en"],
                );
                (config, tmp)
            },
            |(config, _tmp)| {
                extract(config).unwrap();
            },
        );
    });
}

fn bench_500keys_5langs(c: &mut Criterion) {
    c.bench_function("500keys_5langs", |b| {
        b.iter_with_setup(
            || {
                let tmp = TempDir::new().unwrap();
                setup_large_fixtures(tmp.path(), 500, &["en", "uk", "ru", "de", "fr"]);
                let config = make_config(
                    tmp.path().join("code"),
                    tmp.path().join("locales"),
                    vec!["en", "uk", "ru", "de", "fr"],
                );
                (config, tmp)
            },
            |(config, _tmp)| {
                extract(config).unwrap();
            },
        );
    });
}

fn bench_500keys_10langs(c: &mut Criterion) {
    c.bench_function("500keys_10langs", |b| {
        b.iter_with_setup(
            || {
                let tmp = TempDir::new().unwrap();
                setup_large_fixtures(
                    tmp.path(),
                    500,
                    &["en", "uk", "ru", "de", "fr", "es", "it", "pt", "ja", "zh"],
                );
                let config = make_config(
                    tmp.path().join("code"),
                    tmp.path().join("locales"),
                    vec!["en", "uk", "ru", "de", "fr", "es", "it", "pt", "ja", "zh"],
                );
                (config, tmp)
            },
            |(config, _tmp)| {
                extract(config).unwrap();
            },
        );
    });
}

fn bench_2000keys_10langs(c: &mut Criterion) {
    c.bench_function("2000keys_10langs", |b| {
        b.iter_with_setup(
            || {
                let tmp = TempDir::new().unwrap();
                setup_large_fixtures(
                    tmp.path(),
                    2000,
                    &["en", "uk", "ru", "de", "fr", "es", "it", "pt", "ja", "zh"],
                );
                let config = make_config(
                    tmp.path().join("code"),
                    tmp.path().join("locales"),
                    vec!["en", "uk", "ru", "de", "fr", "es", "it", "pt", "ja", "zh"],
                );
                (config, tmp)
            },
            |(config, _tmp)| {
                extract(config).unwrap();
            },
        );
    });
}

criterion_group!(
    benches,
    bench_500keys_1lang,
    bench_500keys_5langs,
    bench_500keys_10langs,
    bench_2000keys_10langs,
);
criterion_main!(benches);
