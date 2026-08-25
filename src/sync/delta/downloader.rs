use alloc::vec::Vec;

pub struct Block {
    pub height: u64,
    pub data: Vec<u8>,
}

pub fn download_blocks(from_height: u64, to_height: u64) -> Vec<Block> {
    let mut blocks = Vec::new();
    for height in from_height..=to_height {
        // standard block sync request
        blocks.push(Block {
            height,
            data: Vec::new(),
        });
    }
    blocks
}
