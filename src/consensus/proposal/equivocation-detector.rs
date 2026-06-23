use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct Proposal {
    pub height: u64,
    pub view: u64,
    pub block_hash: [u8; 32],
    pub signature: Vec<u8>,
    pub proposer: String,
}

#[derive(Clone, Debug)]
pub struct EquivocationProof {
    pub proposal_a: Proposal,
    pub proposal_b: Proposal,
}

pub struct EquivocationDetector {
    seen_proposals: HashMap<u64, Vec<Proposal>>,
}

impl EquivocationDetector {
    pub fn new() -> Self {
        Self {
            seen_proposals: HashMap::new(),
        }
    }

    pub fn handle_proposal(&mut self, proposal: Proposal) -> Option<EquivocationProof> {
        let height = proposal.height;
        let proposals = self.seen_proposals.entry(height).or_insert_with(Vec::new);

        for existing in proposals.iter() {
            // Equivocation detection: 2 conflicting proposals at same height with valid signatures
            // Conflicting proposals: same height, same proposer, different block hash
            if existing.block_hash != proposal.block_hash
                && existing.proposer == proposal.proposer
            {
                // broadcast an EquivocationProof message with both proposals
                return Some(EquivocationProof {
                    proposal_a: existing.clone(),
                    proposal_b: proposal.clone(),
                });
            }
        }

        proposals.push(proposal);
        None
    }
}
