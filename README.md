# Solana Dead Man's Switch (Anchor)

This project implements a Solana dead man's switch smart contract using the Anchor framework.

An owner creates a vault PDA, deposits SOL, sets a backup wallet, and defines a check-in interval in days.
If the owner stops checking in before the interval expires, the backup wallet can claim the funds.
If the owner is still active, they can cancel the vault and recover their SOL.

## Core Instructions

### 1) initialize
Creates a vault and transfers the owner's SOL deposit into it.

Inputs:
- `backup_wallet: Pubkey`
- `interval_days: u64`
- `deposit_lamports: u64`

Behavior:
- Creates PDA at seeds `["vault", owner, backup_wallet]`
- Stores owner, backup wallet, last check-in timestamp, interval, SOL amount, and bump
- Transfers `deposit_lamports` from owner to vault account

### 2) check_in
Resets the countdown by updating `last_check_in` to the current blockchain timestamp.

Who can call:
- Owner only

### 3) claim
Allows the backup wallet to claim the vault funds if the owner has not checked in for at least the configured interval.

Who can call:
- Backup wallet only

Behavior:
- Verifies elapsed time condition:
  - `current_timestamp >= last_check_in + interval_days * 86400`
- Closes the vault account to the backup wallet
- Sends all vault lamports (including rent) to backup wallet

### 4) cancel
Allows the owner to close the vault and reclaim funds while they are still active.

Who can call:
- Owner only

Behavior:
- Verifies vault has not expired:
  - `current_timestamp < last_check_in + interval_days * 86400`
- Closes the vault account to owner
- Sends all vault lamports (including rent) back to owner

## Vault Account Structure

The program stores this account:

```rust
pub struct Vault {
    pub owner: Pubkey,
    pub backup_wallet: Pubkey,
    pub last_check_in: i64,
    pub interval_days: u64,
    pub sol_amount: u64,
    pub bump: u8,
}
```

Field details:
- `owner`: Wallet that initializes and manages the vault
- `backup_wallet`: Wallet allowed to claim after expiration
- `last_check_in`: Unix timestamp of last owner activity
- `interval_days`: Inactivity window before claim is allowed
- `sol_amount`: Initial deposited amount tracked by the program
- `bump`: PDA bump for deterministic address derivation

## Program Errors

The program returns explicit Anchor errors for key failure cases:
- `IntervalNotElapsed`: Claim attempted before inactivity interval has passed
- `UnauthorizedBackup`: Claim signer is not the configured backup wallet
- `VaultExpired`: Owner tried to cancel after expiration
- `OwnerIsBackup`: Owner tried to configure self as backup wallet
- `ZeroDeposit`: Initialization attempted with 0 lamports
- `MathOverflow`: Overflow while computing `interval_days * 86400`

## Test Coverage

The TypeScript test suite covers all required behaviors:
- Successful initialization
- Valid owner check-in
- Failed claim before interval expiration
- Successful claim after expiration (demonstrated using `interval_days = 0`, which expires immediately)
- Successful owner cancel while still active

## Project Layout

- `programs/dead_switch/src/lib.rs`: Anchor program logic
- `tests/dead_switch.ts`: Anchor TypeScript tests
- `Anchor.toml`: Anchor workspace config
- `migrations/deploy.ts`: Deployment script entry

## Run Locally (Localnet)

Prerequisites:
- Rust + Cargo
- Solana CLI
- Anchor CLI
- Node.js + npm

Recommended setup steps:

1. Install JS dependencies:
   ```bash
   npm install
   ```

2. Ensure Solana points to localhost:
   ```bash
   solana config set --url localhost
   ```

3. Start local validator (separate terminal):
   ```bash
   solana-test-validator
   ```

4. Build and test with Anchor:
   ```bash
   anchor test
   ```

Anchor automatically builds the program and runs the TypeScript tests in `tests/`.

## Notes

- Vault SOL is held directly in the PDA account lamports.
- On `claim` and `cancel`, the account is closed and all lamports are returned to the destination wallet.
- `interval_days = 0` is allowed and makes the vault immediately claimable.
