// SPDX-License-Identifier: MIT
pragma solidity ^0.8.19;

import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";
import "@openzeppelin/contracts/security/ReentrancyGuard.sol";
import "@openzeppelin/contracts/access/Ownable.sol";

error InvalidBondAmount();
error InvalidLockDuration();
error BondAlreadyLocked();
error BondNotLocked();
error LockDurationNotElapsed();
error BondNotSlashed();

contract PoolManager is ReentrancyGuard, Ownable {
    using SafeERC20 for IERC20;

    struct TenantBond {
        uint256 amount;
        uint256 lockEndTime;
        bool isActive;
        bool isSlashed;
    }

    IERC20 public immutable lumToken;
    mapping(address => TenantBond) public tenantBonds;
    uint256 public totalBonded;

    event BondLocked(address indexed tenant, uint256 amount, uint256 lockEndTime);
    event BondUnlocked(address indexed tenant, uint256 amount);
    event BondSlashed(address indexed tenant, uint256 amount);
    event SlashedBondClaimed(address indexed owner, uint256 amount);

    constructor(address lumTokenAddress) {
        require(lumTokenAddress != address(0), "Invalid LUM token address");
        lumToken = IERC20(lumTokenAddress);
    }

    function lockTenantBond(uint256 amount, uint256 lockDuration) external nonReentrant {
        if (amount < 100 || amount > 10000) {
            revert InvalidBondAmount();
        }
        if (lockDuration < 7 days) {
            revert InvalidLockDuration();
        }
        if (tenantBonds[msg.sender].isActive) {
            revert BondAlreadyLocked();
        }

        uint256 lockEndTime = block.timestamp + lockDuration;
        tenantBonds[msg.sender] = TenantBond({
            amount: amount,
            lockEndTime: lockEndTime,
            isActive: true,
            isSlashed: false
        });
        totalBonded += amount;

        emit BondLocked(msg.sender, amount, lockEndTime);

        lumToken.safeTransferFrom(msg.sender, address(this), amount);
    }

    function unlockTenantBond() external nonReentrant {
        TenantBond storage bond = tenantBonds[msg.sender];
        if (!bond.isActive) {
            revert BondNotLocked();
        }
        if (block.timestamp < bond.lockEndTime) {
            revert LockDurationNotElapsed();
        }
        if (bond.isSlashed) {
            revert BondNotLocked();
        }

        uint256 amount = bond.amount;
        bond.isActive = false;
        totalBonded -= amount;

        emit BondUnlocked(msg.sender, amount);

        lumToken.safeTransfer(msg.sender, amount);
    }

    function slashTenantBond(address tenant) external onlyOwner nonReentrant {
        TenantBond storage bond = tenantBonds[tenant];
        if (!bond.isActive) {
            revert BondNotLocked();
        }
        if (bond.isSlashed) {
            revert BondNotLocked();
        }

        bond.isSlashed = true;

        emit BondSlashed(tenant, bond.amount);
    }

    function claimSlashedBond(address tenant) external onlyOwner nonReentrant {
        TenantBond storage bond = tenantBonds[tenant];
        if (!bond.isSlashed) {
            revert BondNotSlashed();
        }

        uint256 amount = bond.amount;
        bond.isActive = false;
        totalBonded -= amount;

        emit SlashedBondClaimed(msg.sender, amount);

        lumToken.safeTransfer(msg.sender, amount);
    }

    function getTenantBond(address tenant) external view returns (TenantBond memory) {
        return tenantBonds[tenant];
    }
}