use anyhow::{Context, Result};
use ignore::WalkBuilder;
use ignore::types::TypesBuilder;
use std::path::{Path, PathBuf};

/// Which `.ftl` files a directory walk should yield.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FtlWalk {
    /// Skip hidden entries and honor `.ignore` and `.gitignore` files found inside `dir`,
    /// whether or not `dir` is inside a git repository. Nothing outside `dir` is read. When a
    /// directory has both files, `.gitignore` rules win. This is what `extract` writes and
    /// what `check` validates.
    Filtered,
    /// Every `.ftl` file below `dir`, including hidden and ignored ones.
    All,
}

/// Lists every `.ftl` file below `dir`, sorted by path. A missing `dir` yields no files;
/// any other I/O error while walking is returned.
pub fn ftl_files(dir: &Path, walk: FtlWalk) -> Result<Vec<PathBuf>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut type_builder = TypesBuilder::new();
    type_builder.add("ftl", "*.ftl")?;
    type_builder.select("ftl");

    let mut builder = WalkBuilder::new(dir);
    builder.types(type_builder.build()?);
    match walk {
        FtlWalk::Filtered => {
            // Even with `parents(false)`, the `ignore` crate canonicalizes `dir` and stats or
            // opens ignore files in every ancestor as long as any git source is enabled. Those
            // ancestor rules were never applied; the scan only detected a git repository to
            // gate `.gitignore`. Treating `.gitignore` as a plain ignore file keeps it honored
            // inside `dir` and lets the crate skip the ancestor walk entirely.
            builder
                .parents(false)
                .git_ignore(false)
                .git_exclude(false)
                .git_global(false)
                .add_custom_ignore_filename(".gitignore")
        }
        FtlWalk::All => builder.standard_filters(false),
    };

    let mut files = Vec::new();
    for entry in builder.build() {
        let entry = entry.with_context(|| format!("Failed to walk {}", dir.display()))?;
        if entry.file_type().is_some_and(|ft| ft.is_file()) {
            files.push(entry.into_path());
        }
    }
    files.sort();
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::{FtlWalk, ftl_files};
    use std::fs;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    fn names(files: &[PathBuf], root: &Path) -> Vec<String> {
        files
            .iter()
            .map(|file| {
                file.strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect()
    }

    fn fixture() -> TempDir {
        let temp = TempDir::new().unwrap();
        let root = temp.path();
        fs::create_dir_all(root.join("pages")).unwrap();
        fs::create_dir_all(root.join(".hidden")).unwrap();
        fs::write(root.join("b.ftl"), "b = B\n").unwrap();
        fs::write(root.join("a.ftl"), "a = A\n").unwrap();
        fs::write(root.join("pages").join("main.ftl"), "m = M\n").unwrap();
        fs::write(root.join("ignored.ftl"), "i = I\n").unwrap();
        fs::write(root.join("gitignored.ftl"), "g = G\n").unwrap();
        fs::write(root.join(".hidden").join("h.ftl"), "h = H\n").unwrap();
        fs::write(root.join("notes.txt"), "not ftl").unwrap();
        // Both ignore files are honored without a git repository around the fixture.
        fs::write(root.join(".ignore"), "ignored.ftl\n").unwrap();
        fs::write(root.join(".gitignore"), "gitignored.ftl\n").unwrap();
        temp
    }

    #[test]
    fn test_filtered_walk_skips_hidden_and_ignored_files_and_sorts() {
        let temp = fixture();

        let files = ftl_files(temp.path(), FtlWalk::Filtered).unwrap();

        assert_eq!(
            names(&files, temp.path()),
            vec!["a.ftl", "b.ftl", "pages/main.ftl"]
        );
    }

    #[test]
    fn test_all_walk_includes_hidden_and_ignored_files() {
        let temp = fixture();

        let files = ftl_files(temp.path(), FtlWalk::All).unwrap();

        assert_eq!(
            names(&files, temp.path()),
            vec![
                ".hidden/h.ftl",
                "a.ftl",
                "b.ftl",
                "gitignored.ftl",
                "ignored.ftl",
                "pages/main.ftl"
            ]
        );
    }

    #[test]
    fn test_filtered_walk_ignores_rules_from_parent_directories() {
        let temp = TempDir::new().unwrap();
        let locale = temp.path().join("locales").join("en");
        fs::create_dir_all(&locale).unwrap();
        fs::write(temp.path().join(".gitignore"), "*.ftl\n").unwrap();
        fs::write(temp.path().join(".ignore"), "*.ftl\n").unwrap();
        fs::write(temp.path().join("locales").join(".gitignore"), "*.ftl\n").unwrap();
        fs::write(locale.join("main.ftl"), "m = M\n").unwrap();

        let files = ftl_files(&locale, FtlWalk::Filtered).unwrap();

        assert_eq!(names(&files, &locale), vec!["main.ftl"]);
    }

    #[test]
    fn test_missing_directory_yields_no_files() {
        let temp = TempDir::new().unwrap();

        let files = ftl_files(&temp.path().join("missing"), FtlWalk::Filtered).unwrap();

        assert!(files.is_empty());
    }
}
