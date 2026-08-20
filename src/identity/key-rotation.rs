#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyEpoch {
    pub key: Vec<u8>,
    pub activation_epoch: u64,
}

impl KeyEpoch {
    pub fn new(key: Vec<u8>, activation_epoch: u64) -> Self {
        Self { key, activation_epoch }
    }
}
