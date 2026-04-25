use std::fmt::Write;
use std::path::Path;

/// Generate a Python file with `n` i18n calls and a matching FTL file
pub fn generate_py_file(n: usize) -> String {
    let mut s = String::with_capacity(n * 80);
    s.push_str("from stub import I18nContext\n\ni18n = I18nContext()\n\n");
    for i in 0..n {
        writeln!(s, "i18n.key_{i}(kwarg_{i}=\"val\")", i = i).unwrap();
    }
    s
}

pub fn generate_ftl_file(n: usize) -> String {
    let mut s = String::with_capacity(n * 60);
    for i in 0..n {
        writeln!(s, "key-{i} = key-{i} {{ $kwarg_{i} }}", i = i).unwrap();
    }
    s
}

pub fn setup_large_fixtures(dir: &Path, num_keys: usize, languages: &[&str]) {
    // Write Python source
    let py_dir = dir.join("code");
    std::fs::create_dir_all(&py_dir).unwrap();
    std::fs::write(py_dir.join("main.py"), generate_py_file(num_keys)).unwrap();

    // Write FTL locales
    for lang in languages {
        let locale_dir = dir.join("locales").join(lang);
        std::fs::create_dir_all(&locale_dir).unwrap();
        std::fs::write(locale_dir.join("_default.ftl"), generate_ftl_file(num_keys)).unwrap();
    }
}
