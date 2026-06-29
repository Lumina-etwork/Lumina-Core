// SPDX-License-Identifier: MIT
pragma solidity ^0.8.19;

import "forge-std/Test.sol";
import "../src/PoolManager.sol";
import "../src/VestingVault/MockERC20.sol";

contract MaliciousTenant {
    PoolManager public poolManager;
    MockERC20 public lumToken;
    uint256 public reentrancyCount;

    constructor(PoolManager _poolManager, MockERC20 _lumToken) {
        poolManager = _poolManager;
        lumToken = _lumToken;
    }

    function attack() external {
        // First lock a bond
        lumToken.approve(address(poolManager), 1000);
        poolManager.lockTenantBond(1000, 7 days);
        
        // Warp time to after lock end
        vm.warp(block.timestamp + 7 days + 1);
        
        // Try to unlock (which will trigger reentrancy)
        reentrancyCount = 0;
        poolManager.unlockTenantBond();
    }

    function onERC20Received(address, address, uint256, bytes calldata) external returns (bytes4) {
        // Try to re-enter unlockTenantBond
        if (reentrancyCount < 100) {
            reentrancyCount++;
            try poolManager.unlockTenantBond() {} catch {}
        }
        return this.onERC20Received.selector;
    }

    receive() external payable {
        // Fallback for possible ETH transfers
        if (reentrancyCount < 100) {
            reentrancyCount++;
            try poolManager.unlockTenantBond() {} catch {}
        }
    }
}

contract PoolManagerTest is Test {
    PoolManager public poolManager;
    MockERC20 public lumToken;
    address public owner = address(0x1);
    address public tenant = address(0x2);

    function setUp() public {
        vm.startPrank(owner);
        lumToken = new MockERC20();
        poolManager = new PoolManager(address(lumToken));
        vm.stopPrank();
    }

    function testLockBond() public {
        vm.startPrank(tenant);
        lumToken.mint(tenant, 1000);
        lumToken.approve(address(poolManager), 1000);
        poolManager.lockTenantBond(1000, 7 days);
        vm.stopPrank();

        PoolManager.TenantBond memory bond = poolManager.getTenantBond(tenant);
        assertEq(bond.amount, 1000);
        assertEq(bond.isActive, true);
        assertEq(poolManager.totalBonded(), 1000);
    }

    function testUnlockBond() public {
        vm.startPrank(tenant);
        lumToken.mint(tenant, 1000);
        lumToken.approve(address(poolManager), 1000);
        poolManager.lockTenantBond(1000, 7 days);
        vm.warp(block.timestamp + 7 days + 1);
        poolManager.unlockTenantBond();
        vm.stopPrank();

        PoolManager.TenantBond memory bond = poolManager.getTenantBond(tenant);
        assertEq(bond.isActive, false);
        assertEq(poolManager.totalBonded(), 0);
        assertEq(lumToken.balanceOf(tenant), 1000);
    }

    function testSlashAndClaimBond() public {
        vm.startPrank(tenant);
        lumToken.mint(tenant, 1000);
        lumToken.approve(address(poolManager), 1000);
        poolManager.lockTenantBond(1000, 7 days);
        vm.stopPrank();

        vm.startPrank(owner);
        poolManager.slashTenantBond(tenant);
        poolManager.claimSlashedBond(tenant);
        vm.stopPrank();

        PoolManager.TenantBond memory bond = poolManager.getTenantBond(tenant);
        assertEq(bond.isActive, false);
        assertEq(bond.isSlashed, true);
        assertEq(poolManager.totalBonded(), 0);
        assertEq(lumToken.balanceOf(owner), 1000);
    }

    function testReentrancyProtection() public {
        // Create malicious tenant
        MaliciousTenant malicious = new MaliciousTenant(poolManager, lumToken);
        lumToken.mint(address(malicious), 1000);

        // Attack
        vm.expectRevert(); // Should fail or not allow reentrancy
        malicious.attack();

        // Check that reentrancy didn't succeed
        assertEq(lumToken.balanceOf(address(malicious)), 1000);
    }

    // Fuzz test: test various bond amounts and lock durations
    function testFuzzLockBond(uint256 amount, uint256 lockDuration) public {
        // Bound the inputs
        vm.assume(amount >= 100 && amount <= 10000);
        vm.assume(lockDuration >= 7 days);

        vm.startPrank(tenant);
        lumToken.mint(tenant, amount);
        lumToken.approve(address(poolManager), amount);
        poolManager.lockTenantBond(amount, lockDuration);
        vm.stopPrank();

        PoolManager.TenantBond memory bond = poolManager.getTenantBond(tenant);
        assertEq(bond.amount, amount);
        assertEq(bond.isActive, true);
        assertEq(poolManager.totalBonded(), amount);
    }
}