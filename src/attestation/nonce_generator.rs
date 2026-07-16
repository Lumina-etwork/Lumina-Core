//! Nonce generator using BLAKE2b hashing for epoch-scoped challenges.

use blake2::{Blake2b512, Digest};

/// Generates epoch-scoped nonces for proof-of-connectivity challenges.
pub struct NonceGenerator {
    /// Random seed used to diversify nonces across runs.
    seed: [u8; 32],
}

impl NonceGenerator {
    /// Create a new NonceGenerator with a given random seed.
    pub fn new(seed: [u8; 32]) -> Self {
        Self { seed }
    }

    /// Generate a nonce using BLAKE2b(epoch_id || seed || node_id).
    ///
    /// # Arguments
    /// * `epoch_id` - The current epoch ID
    /// * `node_id` - The ID of the node the challenge is for
    ///
    /// # Returns
    /// 256-bit (32-byte) nonce
    pub fn generate(&self, epoch_id: u32, node_id: &str) -> [u8; 32] {
        let mut hasher = Blake2b512::new();
        hasher.update(epoch_id.to_be_bytes());
        hasher.update(&self.seed);
        hasher.update(node_id.as_bytes());

        let hash = hasher.finalize();
        let mut nonce = [0u8; 32];
        nonce.copy_from_slice(&hash[0..32]);
        nonce
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn different_epochs_produce_different_nonces() {
        let gen = NonceGenerator::new([1u8; 32]);
        let n1 = gen.generate(1, "node-1");
        let n2 = gen.generate(2, "node-1");
        assert_ne!(n1, n2);
    }

    #[test]
    fn different_nodes_produce_different_nonces() {
        let gen = NonceGenerator::new([1u8; 32]);
        let n1 = gen.generate(1, "node-1");
        let n2 = gen.generate(1, "node-2");
        assert_ne!(n1, n2);
    }

    #[test]
    fn same_inputs_produce_same_nonce() {
        let gen = NonceGenerator::new([1u8; 32]);
        let n1 = gen.generate(1, "node-1");
        let n2 = gen.generate(1, "node-1");
        assert_eq!(n1, n2);
    }

    proptest! {
        #[test]
        fn no_collisions_across_epochs(
            seed in any::<[u8; 32]>(),
            node_id in ".*",
            epoch1 in any::<u32>(),
            epoch2 in any::<u32>()
        ) {
            let gen = NonceGenerator::new(seed);
            if epoch1 != epoch2 {
                let n1 = gen.generate(epoch1, &node_id);
                let n2 = gen.generate(epoch2, &node_id);
                assert_ne!(n1, n2);
            }
        }

        #[test]
        fn no_collisions_across_nodes(
            seed in any::<[u8; 32]>(),
            node_id1 in ".*",
            node_id2 in ".*",
            epoch in any::<u32>()
        ) {
            let gen = NonceGenerator::new(seed);
            if node_id1 != node_id2 {
                let n1 = gen.generate(epoch, &node_id1);
                let n2 = gen.generate(epoch, &node_id2);
                assert_ne!(n1, n2);
            }
        }

        #[test]
        fn same_inputs_give_same_nonce(
            seed in any::<[u8; 32]>(),
            node_id in ".*",
            epoch in any::<u32>()
        ) {
            let gen = NonceGenerator::new(seed);
            let n1 = gen.generate(epoch, &node_id);
            let n2 = gen.generate(epoch, &node_id);
            assert_eq!(n1, n2);
        }
    }
}
