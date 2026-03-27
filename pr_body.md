This PR addresses three issues:

## Issue #39: [Optimization] Dynamic Storage TTL Bumping
- ✅ Added `bump_if_needed()` helper function that checks `env.storage().instance().max_ttl()`
- ✅ Only bumps storage TTL if within 30 days of expiration (720*30 ledgers)
- ✅ Updated all storage operations to use optimized TTL bumping
- ✅ Reduces gas costs by avoiding unnecessary TTL extensions

## Issue #38: [Feature] Decentralized Identity (DID) Gating  
- ✅ Added `REQUIRED_SBT_ADDRESS` to global contract config
- ✅ Added `set_required_sbt()` function for configuration
- ✅ Updated `claim()` to perform cross-contract call to SBT contract
- ✅ Checks `balance_of(beneficiary) > 0` before allowing claims
- ✅ Added SBT verification to `claim_and_split()` as well

## Issue #40: [Logic] Split Claim Destinations
- ✅ Implemented `claim_and_split(vault_id, secondary_address, split_percentage)` function
- ✅ Executes logic to split tokens between primary and secondary addresses
- ✅ Returns tuple of (primary_amount, secondary_amount)
- ✅ Includes all validation and SBT checks
- ✅ Added `SplitClaimData` struct for future batch operations

### Testing
All functions include proper error handling and validation. The implementation follows the existing code patterns and maintains backward compatibility where possible.
