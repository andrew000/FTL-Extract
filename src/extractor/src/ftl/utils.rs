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
}
