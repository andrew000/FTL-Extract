use std::fs;
use std::io::{self, Write};
use std::path::Path;

/// Writes `contents` to `path` atomically.
///
/// The data goes to a temporary file in the same directory, is flushed to disk, and is then
/// renamed over `path`. An interrupted run therefore leaves either the previous file or the
/// complete new one on disk, never a truncated one. On failure the temporary file is removed
/// and the existing file is left untouched.
pub(crate) fn write_atomically(path: &Path, contents: &[u8]) -> io::Result<()> {
    let parent = match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    fs::create_dir_all(parent)?;

    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("file");
    let mut temp = tempfile::Builder::new()
        .prefix(&format!(".{file_name}."))
        .suffix(".tmp")
        .tempfile_in(parent)?;
    temp.write_all(contents)?;
    temp.as_file().sync_all()?;
    temp.persist(path).map_err(|err| err.error)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::write_atomically;
    use std::fs;
    use std::path::Path;
    use tempfile::TempDir;

    fn temp_files_in(dir: &Path) -> Vec<String> {
        fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().to_string())
            .filter(|name| name.ends_with(".tmp"))
            .collect()
    }

    #[test]
    fn test_write_creates_parent_directories_and_replaces_content() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("nested").join("dir").join("file.ftl");

        write_atomically(&path, b"first").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "first");

        write_atomically(&path, b"second").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "second");
        assert!(temp_files_in(path.parent().unwrap()).is_empty());
    }

    #[test]
    fn test_write_fails_when_parent_is_a_file() {
        let temp = TempDir::new().unwrap();
        let blocker = temp.path().join("not-a-dir");
        fs::write(&blocker, "file").unwrap();

        let error = write_atomically(&blocker.join("file.ftl"), b"content").unwrap_err();

        assert!(!error.to_string().is_empty());
        assert_eq!(fs::read_to_string(&blocker).unwrap(), "file");
    }

    #[test]
    fn test_write_fails_when_target_is_a_directory() {
        let temp = TempDir::new().unwrap();
        let target = temp.path().join("file.ftl");
        fs::create_dir_all(&target).unwrap();

        write_atomically(&target, b"content").unwrap_err();

        assert!(target.is_dir());
        assert!(temp_files_in(temp.path()).is_empty());
    }

    #[test]
    fn test_failed_replace_leaves_existing_file_untouched() {
        let temp = TempDir::new().unwrap();
        let target = temp.path().join("file.ftl");
        fs::write(&target, "original").unwrap();

        // Make the replacement fail: Windows refuses to rename over a read-only file, Unix
        // refuses to create or rename entries in a read-only directory.
        #[cfg(windows)]
        let locked = target.clone();
        #[cfg(not(windows))]
        let locked = temp.path().to_path_buf();
        let mut permissions = fs::metadata(&locked).unwrap().permissions();
        let original_permissions = permissions.clone();
        permissions.set_readonly(true);
        fs::set_permissions(&locked, permissions).unwrap();

        let result = write_atomically(&target, b"replacement");

        fs::set_permissions(&locked, original_permissions).unwrap();
        assert!(result.is_err());
        assert_eq!(fs::read_to_string(&target).unwrap(), "original");
        assert!(temp_files_in(temp.path()).is_empty());
    }
}
