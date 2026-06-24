#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistrationAttempt {
    pub node_id: String,
    pub public_key: Vec<u8>,
    pub epoch: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConflictResolution {
    pub winner: RegistrationAttempt,
    pub rejected: Vec<RegistrationAttempt>,
    pub resolved_epoch: u64,
}

pub fn public_key_priority_hash(public_key: &[u8]) -> [u8; 32] {
    const OFFSETS: [u64; 4] = [
        0xcbf29ce484222325,
        0x84222325cbf29ce4,
        0x9e3779b97f4a7c15,
        0x94d049bb133111eb,
    ];
    let mut out = [0u8; 32];
    for (chunk, seed) in out.chunks_exact_mut(8).zip(OFFSETS) {
        let mut hash = seed;
        for byte in public_key {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3);
            hash ^= hash.rotate_left(13);
        }
        chunk.copy_from_slice(&hash.to_be_bytes());
    }
    out
}

pub fn select_priority_winner(attempts: &[RegistrationAttempt]) -> Option<RegistrationAttempt> {
    attempts.iter().cloned().max_by(|left, right| {
        public_key_priority_hash(&left.public_key)
            .cmp(&public_key_priority_hash(&right.public_key))
            .then_with(|| left.public_key.cmp(&right.public_key))
    })
}

pub fn resolve_conflict(
    attempts: Vec<RegistrationAttempt>,
    resolved_epoch: u64,
) -> Option<ConflictResolution> {
    let winner = select_priority_winner(&attempts)?;
    let rejected = attempts
        .into_iter()
        .filter(|attempt| attempt.public_key != winner.public_key)
        .collect();

    Some(ConflictResolution {
        winner,
        rejected,
        resolved_epoch,
    })
}
