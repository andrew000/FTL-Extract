use crate::ftl::utils::{FastHashMap, FastHashSet};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

const CACHE_FILE: &str = ".ftl_extract_cache";

#[derive(Default, Serialize, Deserialize)]
pub struct FileCache<MTime = u64> {
    pub files_with_keys: FastHashMap<PathBuf, MTime>,
    pub files_without_keys: FastHashSet<PathBuf>,
}

impl FileCache {
    pub fn load(cache_path: &Path) -> Result<Self> {
        let cache_path = Path::new(&cache_path);
        if !cache_path.exists() {
            return Ok(Self::default());
        }
        let bytes = fs::read(cache_path)?;
        let deserialized = serde_json::from_slice::<FileCache>(&bytes)?;

        Ok(deserialized)
    }

    pub fn save(&self, cache_path: &Path) -> Result<()> {
        let bytes = serde_json::to_vec_pretty(self)?;
        fs::write(cache_path, bytes)?;
        Ok(())
    }

    pub fn is_file_modified(&self, path: &PathBuf) -> bool {
        let metadata = fs::metadata(path).unwrap();
        let modified = metadata
            .modified()
            .unwrap()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        if let Some(mtime) = self.files_with_keys.get(path) {
            modified != *mtime
        } else if self.files_without_keys.contains(path) {
            false
        } else {
            true // File not in cache
        }
    }

    pub fn mark_file_with_keys(&mut self, path: &PathBuf) {
        self.files_with_keys.insert(
            path.clone(),
            fs::metadata(path)
                .unwrap()
                .modified()
                .unwrap()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        );
    }

    pub fn mark_file_without_keys(&mut self, path: &PathBuf) {
        self.files_without_keys.insert(path.clone());
    }

    pub fn remove_entry(&mut self, path: &PathBuf) {
        self.files_with_keys.remove(path);
    }

    pub fn has_entries(&self) -> bool {
        !self.files_with_keys.is_empty()
    }

    pub fn clean_files_with_keys(&mut self) {
        self.files_with_keys.clear();
    }

    pub fn clean_files_without_keys(&mut self) {
        self.files_without_keys.clear();
    }
}
