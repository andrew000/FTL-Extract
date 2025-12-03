use hashbrown::HashMap;

pub struct ExtractionStatistics {
    pub py_files_count: usize,
    pub ftl_files_count: HashMap<String, usize>,
    pub ftl_in_code_keys_count: usize,
    pub ftl_stored_keys_count: HashMap<String, usize>,
    pub ftl_keys_updated: HashMap<String, usize>,
    pub ftl_keys_added: HashMap<String, usize>,
    pub ftl_keys_commented: HashMap<String, usize>,
}

impl ExtractionStatistics {
    pub(crate) fn new() -> Self {
        Self {
            py_files_count: 0,
            ftl_files_count: HashMap::new(),
            ftl_in_code_keys_count: 0,
            ftl_stored_keys_count: HashMap::new(),
            ftl_keys_updated: HashMap::new(),
            ftl_keys_added: HashMap::new(),
            ftl_keys_commented: HashMap::new(),
        }
    }

    pub(crate) fn init_lang(&mut self, lang: &str) {
        let l = lang.to_string();
        self.ftl_files_count.insert(l.clone(), 0);
        self.ftl_stored_keys_count.insert(l.clone(), 0);
        self.ftl_keys_updated.insert(l.clone(), 0);
        self.ftl_keys_added.insert(l.clone(), 0);
        self.ftl_keys_commented.insert(l, 0);
    }

    pub fn merge(&mut self, other: ExtractionStatistics) {
        // Merge HashMaps
        for (k, v) in other.ftl_files_count {
            *self.ftl_files_count.entry(k).or_default() += v;
        }
        for (k, v) in other.ftl_stored_keys_count {
            *self.ftl_stored_keys_count.entry(k).or_default() += v;
        }
        for (k, v) in other.ftl_keys_updated {
            *self.ftl_keys_updated.entry(k).or_default() += v;
        }
        for (k, v) in other.ftl_keys_added {
            *self.ftl_keys_added.entry(k).or_default() += v;
        }
        for (k, v) in other.ftl_keys_commented {
            *self.ftl_keys_commented.entry(k).or_default() += v;
        }
    }
}
