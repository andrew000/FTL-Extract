use rustc_hash::FxHasher;
use std::collections::{HashMap, HashSet};
use std::hash::BuildHasherDefault;

pub type FastHashMap<K, V> = HashMap<K, V, BuildHasherDefault<FxHasher>>;
pub type FastHashSet<K> = HashSet<K, BuildHasherDefault<FxHasher>>;
