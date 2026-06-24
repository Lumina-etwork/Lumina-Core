use super::proposal::equivocation_detector::{Proposal, EquivocationDetector, EquivocationProof};
use super::leader_election::timeout_leader::TimeoutLeader;
use super::recovery::fallback_sync::FallbackSync;

pub struct ConsensusEngine {
    pub height: u64,
    pub view: u64,
    pub locked_proposal: Option<Proposal>,
    pub locked_view: u64,
    pub consecutive_deadlocked_views: u64,
    pub timeout_leader: TimeoutLeader,
    pub equivocation_detector: EquivocationDetector,
    pub fallback_sync: FallbackSync,
    pub in_fallback_mode: bool,
}

impl ConsensusEngine {
    pub fn new() -> Self {
        Self {
            height: 0,
            view: 0,
            locked_proposal: None,
            locked_view: 0,
            consecutive_deadlocked_views: 0,
            timeout_leader: TimeoutLeader::new(),
            equivocation_detector: EquivocationDetector::new(),
            fallback_sync: FallbackSync::new(),
            in_fallback_mode: false,
        }
    }

    // Locking rule: a replica locks on first proposal it sees (no unlock until next view)
    pub fn handle_proposal(&mut self, proposal: Proposal) {
        if self.in_fallback_mode {
            return;
        }

        // Detect equivocation
        if let Some(proof) = self.equivocation_detector.handle_proposal(proposal.clone()) {
            self.handle_equivocation(proof);
            return;
        }

        if self.locked_proposal.is_none() {
            self.locked_proposal = Some(proposal);
            self.locked_view = self.view;
        }
    }

    pub fn handle_equivocation(&mut self, proof: EquivocationProof) {
        // immediately advance to the next view without waiting for timeout
        self.timeout_leader.handle_equivocation_proof(&proof);
        self.view = self.timeout_leader.current_view;
        
        // Reset locking rule for next view
        self.locked_proposal = None;
        
        // Track deadlocked views
        self.consecutive_deadlocked_views += 1;
        
        self.check_fallback_trigger();
    }

    pub fn handle_view_timeout(&mut self) {
        if self.timeout_leader.handle_timeout_check() {
            self.view = self.timeout_leader.current_view;
            
            // Reset locking rule for next view
            self.locked_proposal = None;
            
            // Track deadlocked views
            self.consecutive_deadlocked_views += 1;
            
            self.check_fallback_trigger();
        }
    }

    pub fn handle_commit(&mut self) {
        self.consecutive_deadlocked_views = 0;
        self.in_fallback_mode = false;
        self.height += 1;
        self.locked_proposal = None;
    }

    fn check_fallback_trigger(&mut self) {
        // Deadlock threshold: >5 consecutive views without a committed block
        if self.consecutive_deadlocked_views >= 5 && !self.in_fallback_mode {
            self.in_fallback_mode = true;
            self.trigger_fallback_sync();
        }
    }

    fn trigger_fallback_sync(&mut self) {
        let fallback = self.fallback_sync.run_byzantine_agreement(
            self.height,
            self.view,
            self.locked_proposal.clone(),
            self.locked_view,
        );
        
        if let Some(proposal) = fallback {
            self.locked_proposal = Some(proposal);
            self.locked_view = self.view;
            self.handle_commit();
        }
    }
}
