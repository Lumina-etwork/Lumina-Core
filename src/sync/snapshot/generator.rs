use alloc::vec::Vec;

pub struct Snapshot {
    pub block_height: u64,
    pub state_root: [u8; 32],
    pub compressed_state: Vec<u8>,
}

pub struct Account {
    pub id: [u8; 32],
    pub state: Vec<u8>,
}

pub fn generate_snapshot(block_height: u64, state_root: [u8; 32], accounts: &[Account]) -> Snapshot {
    let mut state_data = Vec::new();
    // In a real implementation, we would iterate all accounts, compute Merkle proofs, serialize and append.
    for account in accounts {
        // compute Merkle proof per account
        // serialize state
        state_data.extend_from_slice(&account.id);
        state_data.extend_from_slice(&account.state);
    }
    
    // Compress with Zstd level 10 (mocked)
    let compressed_state = compress_zstd_level_10(&state_data);

    Snapshot {
        block_height,
        state_root,
        compressed_state,
    }
}

fn compress_zstd_level_10(data: &[u8]) -> Vec<u8> {
    // Mock Zstd compression
    let mut compressed = Vec::new();
    compressed.extend_from_slice(b"zstd");
    compressed.extend_from_slice(data);
    compressed
}
