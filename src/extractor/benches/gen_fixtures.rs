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
        writeln!(s, "key_{i} = key_{i} {{ $kwarg_{i} }}", i = i).unwrap();
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

/// Number of `.ftl` files the keys of a project fixture are split over.
pub const PROJECT_FTL_FILES: usize = 8;

/// Generate a project-like layout: `num_py_files` Python modules spread over
/// nested packages, each with `keys_per_file` calls. Keys are routed to
/// `PROJECT_FTL_FILES` files via `_path=`, and every language gets matching
/// `.ftl` files so extraction finds every key already present.
pub fn setup_project_fixtures(
    dir: &Path,
    num_py_files: usize,
    keys_per_file: usize,
    languages: &[&str],
) {
    let code_dir = dir.join("code");
    let mut ftl_bodies = vec![String::new(); PROJECT_FTL_FILES];

    for f in 0..num_py_files {
        let pkg_dir = code_dir
            .join(format!("pkg_{}", f % 10))
            .join(format!("sub_{}", f % 3));
        std::fs::create_dir_all(&pkg_dir).unwrap();

        let mut py = String::with_capacity(keys_per_file * 120);
        py.push_str("from aiogram_i18n import I18nContext\n\n\n");
        writeln!(
            py,
            "async def handler_{f}(i18n: I18nContext, user) -> None:"
        )
        .unwrap();
        for k in 0..keys_per_file {
            let key = format!("mod_{f}-key_{k}");
            let ftl_idx = (f + k) % PROJECT_FTL_FILES;
            let ftl_path = format!("section_{ftl_idx}/messages.ftl");
            match k % 3 {
                0 => {
                    writeln!(
                        py,
                        "    i18n.get(\"{key}\", name=user.name, _path=\"{ftl_path}\")"
                    )
                    .unwrap();
                    writeln!(ftl_bodies[ftl_idx], "{key} = Hello, {{ $name }}!").unwrap();
                }
                1 => {
                    writeln!(
                        py,
                        "    i18n.get(\"{key}\", count=len(user.items), _path=\"{ftl_path}\")"
                    )
                    .unwrap();
                    writeln!(
                        ftl_bodies[ftl_idx],
                        "{key} = {{ $count ->\n    [one] One item\n   *[other] {{ $count }} items\n}}"
                    )
                    .unwrap();
                }
                _ => {
                    writeln!(py, "    i18n.get(\"{key}\", _path=\"{ftl_path}\")").unwrap();
                    writeln!(ftl_bodies[ftl_idx], "{key} = Plain message {k}").unwrap();
                }
            }
        }
        std::fs::write(pkg_dir.join(format!("module_{f}.py")), py).unwrap();
    }

    for lang in languages {
        for (idx, body) in ftl_bodies.iter().enumerate() {
            let section_dir = dir
                .join("locales")
                .join(lang)
                .join(format!("section_{idx}"));
            std::fs::create_dir_all(&section_dir).unwrap();
            std::fs::write(section_dir.join("messages.ftl"), body).unwrap();
        }
    }
}
