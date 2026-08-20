use std::collections::HashMap;

pub struct KeyStore {
    pub keys: HashMap<String, Vec<u8>>,
}

impl KeyStore {
    pub fn new() -> Self {
        Self { keys: HashMap::new() }
    }
}
