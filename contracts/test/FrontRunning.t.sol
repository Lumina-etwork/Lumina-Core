// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import "forge-std/Test.sol";
import "../PoolManager.sol";

contract FrontRunningTest is Test {
    PoolManager public manager;

    function setUp() public {
        manager = new PoolManager();
    }

    function testFrontRunningProtection() public {
        uint256 batchId = 1;
        bytes32 root = bytes32(uint256(1));
        bytes32[] memory proofs = new bytes32[](1);
        uint256[] memory leafIndices = new uint256[](1);
        
        address user = address(1);
        vm.startPrank(user);
        
        bytes32 commitment = keccak256(abi.encodePacked(batchId, user, root, leafIndices));
        manager.batchSettleCommit(commitment);
        
        vm.roll(block.number + 3); // 2-10 blocks delay
        
        vm.stopPrank();
        
        // Attacker tries to frontrun the reveal
        address attacker = address(2);
        vm.startPrank(attacker);
        // Attacker doesn't have a commitment matching their address
        vm.expectRevert("Commitment not found");
        manager.batchSettleReveal(batchId, root, proofs, leafIndices);
        vm.stopPrank();
        
        // User reveals successfully
        vm.startPrank(user);
        manager.batchSettleReveal(batchId, root, proofs, leafIndices);
        vm.stopPrank();
        
        (bytes32 savedRoot, uint256 revealBlock, bool challenged) = manager.settlements(batchId);
        assertEq(savedRoot, root);
        assertEq(revealBlock, block.number);
        assertEq(challenged, false);
    }
}
