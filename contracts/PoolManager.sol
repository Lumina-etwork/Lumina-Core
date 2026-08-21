// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import "./lib/MerkleProof.sol";
import "./lib/CommitReveal.sol";

contract PoolManager is CommitReveal {
    struct Settlement {
        bytes32 root;
        uint256 revealBlock;
        bool challenged;
    }
    
    mapping(uint256 => Settlement) public settlements;
    
    function batchSettleCommit(bytes32 commitment) external {
        _commit(commitment);
    }

    function batchSettleReveal(uint256 batchId, bytes32 root, bytes32[] memory proofs, uint256[] memory leafIndices) external {
        bytes32 expectedCommitment = keccak256(abi.encodePacked(batchId, msg.sender, root, leafIndices));
        _verifyReveal(expectedCommitment);
        
        // Settle logic here: verify proofs using MerkleProof
        
        settlements[batchId] = Settlement({
            root: root,
            revealBlock: block.number,
            challenged: false
        });
    }

    function challengeBatchSettlement(uint256 batchId, bytes32[] memory fraudProof) external {
        Settlement storage s = settlements[batchId];
        require(s.revealBlock > 0, "Not revealed");
        require(block.number <= s.revealBlock + 5, "Challenge period ended");
        
        // verify fraud logic here
        
        s.challenged = true;
    }
}
