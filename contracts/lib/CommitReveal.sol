// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

contract CommitReveal {
    struct CommitInfo {
        uint256 blockNumber;
        bool exists;
    }
    mapping(bytes32 => CommitInfo) public commitments;

    event Committed(bytes32 indexed commitment, uint256 blockNumber);

    function _commit(bytes32 commitment) internal {
        require(!commitments[commitment].exists, "Already committed");
        commitments[commitment] = CommitInfo({
            blockNumber: block.number,
            exists: true
        });
        emit Committed(commitment, block.number);
    }

    function _verifyReveal(bytes32 commitment) internal view {
        require(commitments[commitment].exists, "Commitment not found");
        uint256 commitBlock = commitments[commitment].blockNumber;
        require(block.number >= commitBlock + 2, "Reveal too early");
        require(block.number <= commitBlock + 10, "Reveal too late");
    }
}
