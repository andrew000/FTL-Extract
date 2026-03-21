use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use extractor::ftl::consts::{
    CommentsKeyModes, DEFAULT_EXCLUDE_DIRS, DEFAULT_I18N_KEYS, DEFAULT_IGNORE_ATTRIBUTES,
    DEFAULT_IGNORE_KWARGS, LineEndings,
};
use extractor::ftl::ftl_extractor::{ExtractConfig, extract};
use std::hint::black_box;
use std::path::{Path, PathBuf};
use std::time::Duration;
use std::{fs, io::Write};

fn fixture_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative)
}

struct FixtureDirGuard {
    root: PathBuf,
}

impl FixtureDirGuard {
    fn new(root: PathBuf) -> Self {
        Self { root }
    }
}

impl Drop for FixtureDirGuard {
    fn drop(&mut self) {
        if self.root.exists() {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

fn make_config(code_path: PathBuf, locales_path: PathBuf) -> ExtractConfig {
    let default_ftl_file = PathBuf::from("en").join("_default.ftl");
    ExtractConfig {
        code_path,
        output_path: locales_path,
        languages: vec!["en".to_string()],
        i18n_keys: DEFAULT_I18N_KEYS.clone(),
        i18n_keys_prefix: {
            let mut set = DEFAULT_I18N_KEYS.clone();
            set.insert("self".to_string());
            set.insert("cls".to_string());
            set
        },
        exclude_dirs: DEFAULT_EXCLUDE_DIRS.clone(),
        ignore_attributes: DEFAULT_IGNORE_ATTRIBUTES.clone(),
        ignore_kwargs: DEFAULT_IGNORE_KWARGS.clone(),
        default_ftl_file,
        comment_junks: false,
        comment_keys_mode: CommentsKeyModes::Comment,
        line_endings: LineEndings::Default,
        dry_run: true,
    }
}

fn ensure_large_fixture(
    root: &Path,
    file_count: usize,
    keys_per_file: usize,
) -> (PathBuf, PathBuf) {
    let marker = root.join(format!("{file_count}_{keys_per_file}.ready"));
    let code_dir = root.join("code");
    let locales_dir = root.join("locales");
    let locales_en_dir = locales_dir.join("en");
    let ftl_path = locales_en_dir.join("_default.ftl");

    if marker.exists() {
        return (code_dir, locales_dir);
    }

    if root.exists() {
        let _ = fs::remove_dir_all(root);
    }

    fs::create_dir_all(&code_dir).expect("failed to create large benchmark code dir");
    fs::create_dir_all(&locales_en_dir).expect("failed to create large benchmark locales dir");

    for file_idx in 0..file_count {
        let py_path = code_dir.join(format!("bench_{file_idx}.py"));
        let mut py = String::from("from .stub import I18nContext\ni18n = I18nContext()\n");
        for key_idx in 0..keys_per_file {
            py.push_str(&format!("i18n.mod_{file_idx}.key_{key_idx}()\n"));
        }
        fs::write(py_path, py).expect("failed to write large benchmark Python file");
    }

    let mut ftl = String::new();
    for file_idx in 0..file_count {
        for key_idx in 0..keys_per_file {
            ftl.push_str(&format!(
                "mod_{file_idx}-key_{key_idx} = msg {file_idx} {key_idx}\n"
            ));
        }
    }
    fs::write(ftl_path, ftl).expect("failed to write large benchmark FTL file");

    let mut marker_file = fs::File::create(marker).expect("failed to create fixture marker");
    marker_file
        .write_all(b"ready")
        .expect("failed to write fixture marker");

    (code_dir, locales_dir)
}

fn benchmark_extract_end_to_end(c: &mut Criterion) {
    let small_config = make_config(fixture_path("tests/py"), fixture_path("tests/locales"));
    let small_py_file_count = 3u64;

    let large_root = fixture_path("target")
        .join("bench-fixtures")
        .join("extractor-large");
    let _large_fixture_guard = FixtureDirGuard::new(large_root.clone());
    let large_files = 250usize;
    let large_keys_per_file = 80usize;
    let (large_code_path, large_locales_path) =
        ensure_large_fixture(&large_root, large_files, large_keys_per_file);
    let large_config = make_config(large_code_path, large_locales_path);
    let large_py_file_count = large_files as u64;

    let mut group = c.benchmark_group("extractor");
    group.throughput(Throughput::Elements(small_py_file_count));
    group.bench_with_input(
        BenchmarkId::new("extract_end_to_end", "tests_py_locales_en"),
        &small_config,
        |b, cfg| {
            b.iter(|| {
                let stats = extract(cfg.clone()).expect("extract benchmark run should succeed");
                black_box(stats);
            });
        },
    );

    group.throughput(Throughput::Elements(large_py_file_count));
    group.bench_with_input(
        BenchmarkId::new("extract_end_to_end", "generated_large_250x80"),
        &large_config,
        |b, cfg| {
            b.iter(|| {
                let stats = extract(cfg.clone()).expect("extract benchmark run should succeed");
                black_box(stats);
            });
        },
    );
    group.finish();
}

criterion_group!(
    name = extractor_benches;
    config = Criterion::default()
        .sample_size(30)
        .measurement_time(Duration::from_secs(10));
    targets = benchmark_extract_end_to_end
);
criterion_main!(extractor_benches);
