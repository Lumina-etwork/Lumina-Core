# Security Invariants — Vesting Vault Contract

This document is the primary reference for third-party security auditors.
It enumerates every mathematical guarantee the contract makes, grouped by
subsystem.  Each invariant is stated in plain English, then as a formal
expression, and finally mapped to the relevant contract fields.

---

## 1. Core Vesting Partition

**Plain English:** At every point in time the vested and unvested portions of
a vault sum exactly to the original deposit.  No tokens can be created or
destroyed by vesting math alone.

**Formal:**

```
∀ vault v, ∀ time t:
  Vested(v, t) + Unvested(v, t) = TotalDeposit(v)
```

**Bounds:**

```
0 ≤ Vested(v, t)   ≤ TotalDeposit(v)
0 ≤ Unvested(v, t) ≤ TotalDeposit(v)
```

**Field mapping:**

| Symbol | Contract field |
|--------|---------------|
| `TotalDeposit(v)` | `Vault.total_amount` |
| `Vested(v, t)` | output of `calculate_time_vested_amount` |
| `Unvested(v, t)` | `Vault.total_amount - Vested(v, t)` |

---

## 2. Full Maturity

**Plain English:** Once a vault's end time has passed, the entire deposit is
vested.  No residual locked balance can remain after vesting completes.

**Formal:**

```
∀ vault v, t ≥ end_time(v):
  Vested(v, t) = TotalDeposit(v)
  Unvested(v, t) = 0
```

---

## 3. Monotonic Vesting

**Plain English:** The vested amount never decreases over time (absent
revocation or clawback).

**Formal:**

```
∀ vault v, t₁ ≤ t₂:
  Vested(v, t₁) ≤ Vested(v, t₂)
```

---

## 4. Cliff Guard

**Plain English:** No tokens vest before the cliff timestamp.

**Formal:**

```
∀ vault v, t < cliff_time(v):
  Vested(v, t) = 0
```

**Error returned:** `CliffNotReached`

---

## 5. Claim Accounting

**Plain English:** The cumulative amount claimed from a vault never exceeds
the amount vested at the time of the last claim.

**Formal:**

```
∀ vault v:
  TotalClaimed(v) ≤ Vested(v, now)
  TotalClaimed(v) ≤ TotalDeposit(v)
```

**Field mapping:**

| Symbol | Contract field |
|--------|---------------|
| `TotalClaimed(v)` | `Vault.claimed_amount` |

---

## 6. Revocation Conservation

**Plain English:** When a vault is revoked, the sum of tokens transferred to
the treasury and tokens already claimed equals the original deposit.  No
tokens are lost.

**Formal:**

```
∀ revoked vault v:
  TreasuryTransfer(v) + TotalClaimed(v) = TotalDeposit(v)
```

**Corollary:** `TreasuryTransfer(v) = Unvested(v, revocation_time)`

**Error returned:** `VaultFrozen` on any subsequent claim attempt.

---

## 7. Irrevocability

**Plain English:** A vault marked irrevocable can never be revoked by the
admin.  This guarantee is permanent and cannot be undone.

**Formal:**

```
∀ vault v where v.is_irrevocable = true:
  revoke_vault(v) → Err(VaultFrozen)
```

---

## 8. Governance Veto Threshold

**Plain English:** A governance proposal is cancelled if the "No" vote weight
exceeds 51 % of the total locked token value before the challenge period ends.

**Formal:**

```
∀ proposal p:
  NoVotes(p) > 0.51 × TotalLocked → proposal cancelled
```

Where:

```
TotalLocked = Σ Unvested(v, now)  for all active vaults v
NoVotes(p)  = Σ VotingPower(voter) for all "No" voters on p
VotingPower(voter) = Unvested(voter_vault, now)
```

**Constants:**

| Name | Value |
|------|-------|
| `VOTING_THRESHOLD` | 5100 basis points (51.00 %) |
| `CHALLENGE_PERIOD` | 259 200 seconds (72 hours) |

---

## 9. Governance Challenge Period

**Plain English:** No governance proposal can be executed before 72 hours
have elapsed since it was created.

**Formal:**

```
∀ proposal p:
  execute_proposal(p) requires now ≥ p.created_at + CHALLENGE_PERIOD
```

**Error returned:** `VotingPeriodEnded` if called too early.

---

## 10. Staking — No Token Transfer

**Plain English:** Calling `auto_stake` never moves tokens out of the vault.
The staking contract records a stake against the vault's balance; the tokens
remain in the vault at all times.

**Formal:**

```
∀ vault v, before and after auto_stake(v):
  vault_token_balance(v) is unchanged
```

---

## 11. Staking — Whitelist Enforcement

**Plain English:** The vault only calls a staking contract that has been
explicitly whitelisted by the admin.

**Formal:**

```
auto_stake(v, staking_contract) requires
  staking_contract ∈ ApprovedStakingContracts
```

**Error returned:** `Unauthorized` if the contract is not whitelisted.

---

## 12. Staking — Single Active Stake

**Plain English:** A vault cannot be staked twice simultaneously.

**Formal:**

```
∀ vault v:
  auto_stake(v) requires StakeState(v) = Unstaked
```

**Error returned:** `AlreadyStaked`

---

## 13. Succession — Inactivity Timer

**Plain English:** A backup address can only initiate a succession claim after
the primary has been inactive for at least `switch_duration` seconds.

**Formal:**

```
initiate_succession_claim(v) requires
  now - last_activity(v) ≥ switch_duration(v)
```

**Bounds on `switch_duration`:**

```
MIN_SWITCH_DURATION (2 592 000 s / 30 days)
  ≤ switch_duration
  ≤ MAX_SWITCH_DURATION (63 072 000 s / 730 days)
```

---

## 14. Succession — Challenge Window

**Plain English:** Succession cannot be finalised until the challenge window
has fully elapsed after the claim was initiated.

**Formal:**

```
finalise_succession(v) requires
  now - claimed_at(v) ≥ challenge_window(v)
```

**Bounds on `challenge_window`:**

```
MIN_CHALLENGE_WINDOW (86 400 s / 1 day)
  ≤ challenge_window
  ≤ MAX_CHALLENGE_WINDOW (2 592 000 s / 30 days)
```

---

## 15. Succession — Primary Activity Cancels Claim

**Plain English:** Any on-chain vault interaction by the primary (claim,
stake, unstake, yield claim) resets the inactivity timer and cancels any
pending succession claim.

**Formal:**

```
∀ primary action a on vault v:
  last_activity(v) := now
  if SuccessionState(v) = ClaimPending:
    SuccessionState(v) := Nominated
```

---

## 16. Succession — Irreversibility

**Plain English:** Once succession is finalised the vault owner is permanently
changed to the backup address.  This state cannot be reversed.

**Formal:**

```
∀ vault v where SuccessionState(v) = Succeeded:
  vault.owner = backup_address(v)
  nominate_backup(v) → Err(AlreadySucceeded)
```

---

## 17. Succession — Backup ≠ Primary

**Plain English:** The backup address must differ from the current vault owner.

**Formal:**

```
nominate_backup(v, backup) requires backup ≠ vault.owner
```

**Error returned:** `BackupEqualsPrimary`

---

## 18. Oracle Circuit Breaker

**Plain English:** If an oracle price update within the same ledger deviates
more than 30 % from the previous price, the vault is frozen and no further
price updates are accepted until the admin manually resets the breaker.

**Formal:**

```
|new_price - old_price| / old_price > 0.30
  AND same ledger sequence
  → vault frozen, Err(OraclePriceDeviationTooHigh)
```

**Constant:**

| Name | Value |
|------|-------|
| `ORACLE_DEVIATION_THRESHOLD_BPS` | 3000 (30.00 %) |

---

## 19. Upgrade Safety — No Trapped Funds

**Plain English:** The contract cannot be upgraded or self-destructed while
any unvested tokens remain.  This prevents an admin from trapping user funds
via a malicious upgrade.

**Formal:**

```
assert_safe_to_upgrade() requires
  get_contract_total_unvested() = 0
```

**Error returned:** `UpgradeBlockedByUnvestedFunds`

---

## 20. Admin Dead-Man's Switch

**Plain English:** If the admin is inactive for 365 days, the pre-configured
recovery address can claim admin rights.  The admin can prevent this by
calling `ping_admin_activity` at least once per year.

**Formal:**

```
claim_admin_recovery(recovery) requires
  now - last_admin_activity ≥ ADMIN_INACTIVITY_TIMEOUT (31 536 000 s / 365 days)
  AND recovery = configured_recovery_address
  AND switch.is_triggered = false
```

---

## 21. Tax Withholding Conservation

**Plain English:** When tax withholding is enabled, the gross claim amount
equals the net amount paid to the beneficiary plus the tax amount sent to the
treasury.  No tokens are lost.

**Formal:**

```
gross_amount = net_amount + tax_amount
tax_amount   = floor(gross_amount × tax_withholding_bps / 10000)
net_amount   = gross_amount - tax_amount
```

**Bound:** `tax_withholding_bps ≤ 10000` (100 %)

---

## 22. Milestone Sequencing

**Plain English:** Milestones must be completed in order.  Milestone N cannot
be completed before milestone N-1.

**Formal:**

```
complete_milestone(v, N) requires
  N = 1  OR  milestone_completed(v, N-1) = true
```

**Error returned:** `MilestoneNotCompleted`

---

## 23. Milestone Percentage Sum

**Plain English:** The sum of all milestone percentages for a vault must equal
exactly 100.

**Formal:**

```
configure_milestone_vesting(v, percentages) requires
  Σ percentages[i] = 100
```

---

## 24. Revocability Expiration (Cliff-Drop)

**Plain English:** A vault's revocability expires 12 months after creation.
After that point the admin can no longer revoke the vault even if it was
originally marked revocable.

**Formal:**

```
∀ vault v where v.is_revocable = true:
  now ≥ v.revocability_expires_at
    → v.is_revocable := false  (write-once transition)
    → revoke_vault(v) → Err(VaultFrozen)
```

---

## 25. Payout Address Timelock

**Plain English:** A newly requested authorised payout address does not take
effect until 48 hours after the request.  This protects against phishing
attacks that attempt to redirect funds immediately.

**Formal:**

```
confirm_auth_payout_addr(beneficiary) requires
  now ≥ pending_request.effective_at
  effective_at = requested_at + TIMELOCK_DURATION (172 800 s / 48 hours)
```

**Error returned:** `TimelockNotElapsed`

---

## 26. Beneficiary Reassignment Veto

**Plain English:** A reassignment that moves more than the governance veto
threshold percentage of total supply requires a 7-day veto period.  If veto
votes exceed the threshold the reassignment is cancelled.

**Formal:**

```
total_amount > (total_supply × threshold_pct / 100)
  → requires_governance_veto = true
  → effective_at = now + VETO_PERIOD (604 800 s / 7 days)

VetoVotes(reassignment) ≥ (total_supply × threshold_pct / 100)
  → reassignment cancelled
```

---

## 27. Multisig Quorum

**Plain English:** An admin proposal can only be executed once the number of
valid signatures reaches the quorum threshold.

**Formal:**

```
execute(proposal) requires
  |{signer : signed(proposal, signer) ∧ signer ∈ AdminSet}|
    ≥ QuorumThreshold
```

**Bound:** `1 ≤ QuorumThreshold ≤ |AdminSet|`

---

## 28. Maximum Vesting Duration

**Plain English:** No vesting schedule can have a duration longer than 10 years.

**Formal:**

```
create_vault(start, end) requires
  end - start ≤ MAX_DURATION (315 360 000 s / 10 years)
```

**Error returned:** `InvalidSchedule`

---

## 29. Cross-Contract Reentrancy Lock

**Plain English:** Every state-mutating flow that can synchronously call an
external contract must acquire the global reentrancy lock first. Any callback
that tries to enter a protected flow while that lock is held is rejected.

**Formal:**

```
Locked = storage[DataKey::ReentrancyLock]

ProtectedEntryPoint() requires Locked = false
ProtectedEntryPoint():
  Locked := true
  ...
  ExternalCall()
  ...
  Locked := false

forall callback c during ExternalCall():
  c in ProtectedEntryPoints -> Err(ReentrancyDetected)
```

**Protected entry points:**

- `claim_tokens`
- `claim_tokens_diversified`
- `batch_claim`
- `claim_tokens_regulated`
- `claim_yield`
- `claim_and_swap`
- `auto_stake`
- `manual_unstake` / internal `do_unstake`

**CI witness:** `contracts/vesting_contracts/tests/formal_reentrancy_verification.rs`

---

## 30. Claim CEI Ordering Across Tokens, Staking, and Path Payments

**Plain English:** The vault must persist every claim-side effect that changes
`released_amount` before any untrusted token, staking, or path-payment callback
is given control. A recursive caller must therefore only ever observe the
post-claim state.

**Formal:**

```
forall vault v, claim amount a:
  let Released_before = released_amount(v)

  claim(v, a):
    require a <= claimable(v)
    released_amount(v) := Released_before + a
    persist(v)
    external token / staking / path-payment calls...

  During every callback frame:
    released_amount(v) = Released_before + a
```

**Bounded scenarios enforced in CI:**

- malicious payout token reentering `claim_tokens`
- malicious staking contract reentering from `claim_yield_for`
- multi-hop path payment route where each DEX hop and the destination token
  attempt recursive entry during `claim_and_swap`

**Failure condition:** If any callback can increase `released_amount` twice or
observe the pre-claim value, the formal reentrancy harness must fail.

---

## Halmos / Certora Skeleton

```solidity
// Invariant 1 — Vesting partition
invariant VestingPartition(uint64 vaultId, uint64 t)
    vestedAt(vaultId, t) + unvestedAt(vaultId, t) == totalDeposit(vaultId);

// Invariant 2 — Full maturity
invariant MaturedVaultFullyUnlocked(uint64 vaultId, uint64 t)
    t >= endTime(vaultId) => vestedAt(vaultId, t) == totalDeposit(vaultId);

// Invariant 5 — Claim accounting
invariant ClaimNeverExceedsVested(uint64 vaultId)
    totalClaimed(vaultId) <= vestedAt(vaultId, block.timestamp);

// Invariant 6 — Revocation conservation
invariant RevocationConservation(uint64 vaultId)
    isRevoked(vaultId) =>
        treasuryTransfer(vaultId) + totalClaimed(vaultId) == totalDeposit(vaultId);

// Invariant 19 — Upgrade safety
invariant NoUpgradeWithUnvestedFunds()
    contractTotalUnvested() == 0 => upgradeAllowed();
```
