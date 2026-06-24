use super::super::proposal::equivocation_detector::Proposal;

#[derive(Clone, Debug)]
pub struct FallbackProposal {
    pub proposal: Proposal,
    pub locked_view: u64,
}

pub struct FallbackSync {
    pub received_proposals: Vec<FallbackProposal>,
}

impl FallbackSync {
    pub fn new() -> Self {
        Self {
            received_proposals: Vec::new(),
        }
    }

    pub fn receive_locked_value(&mut self, proposal: Proposal, locked_view: u64) {
        self.received_proposals.push(FallbackProposal {
            proposal,
            locked_view,
        });
    }

    // In fallback-sync.rs, all replicas exchange locked values and the one with highest view-number lock is chosen as the fallback proposal
    pub fn run_byzantine_agreement(
        &mut self,
        _height: u64,
        _view: u64,
        local_proposal: Option<Proposal>,
        local_locked_view: u64,
    ) -> Option<Proposal> {
        let mut best_proposal = local_proposal;
        let mut highest_view = local_locked_view;

        for item in &self.received_proposals {
            if item.locked_view > highest_view {
                highest_view = item.locked_view;
                best_proposal = Some(item.proposal.clone());
            }
        }

        // Clear received proposals for next run
        self.received_proposals.clear();

        best_proposal
    }
}
