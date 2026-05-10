use anchor_lang::prelude::*;
use anchor_lang::system_program::{self, Transfer};

declare_id!("Fg6PaFpoGXkYsidMpWxTWqkZNVYJz8Nwz9fJf3N4Yf5");

const SECONDS_PER_DAY: i64 = 86_400;

#[program]
pub mod dead_switch {
    use super::*;

    pub fn initialize(
        ctx: Context<Initialize>,
        backup_wallet: Pubkey,
        interval_days: u64,
        deposit_lamports: u64,
    ) -> Result<()> {
        require!(deposit_lamports > 0, DeadSwitchError::ZeroDeposit);
        require!(
            backup_wallet != ctx.accounts.owner.key(),
            DeadSwitchError::OwnerIsBackup
        );

        let clock = Clock::get()?;
        let vault = &mut ctx.accounts.vault;

        vault.owner = ctx.accounts.owner.key();
        vault.backup_wallet = backup_wallet;
        vault.last_check_in = clock.unix_timestamp;
        vault.interval_days = interval_days;
        vault.sol_amount = deposit_lamports;
        vault.bump = ctx.bumps.vault;

        // Move the user's initial SOL deposit into the vault PDA account.
        let transfer_ctx = CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            Transfer {
                from: ctx.accounts.owner.to_account_info(),
                to: ctx.accounts.vault.to_account_info(),
            },
        );
        system_program::transfer(transfer_ctx, deposit_lamports)?;

        Ok(())
    }

    pub fn check_in(ctx: Context<CheckIn>) -> Result<()> {
        let clock = Clock::get()?;
        let vault = &mut ctx.accounts.vault;

        vault.last_check_in = clock.unix_timestamp;

        Ok(())
    }

    pub fn claim(ctx: Context<Claim>) -> Result<()> {
        let clock = Clock::get()?;
        let vault = &mut ctx.accounts.vault;
        let interval_seconds = interval_to_seconds(vault.interval_days)?;
        let deadline = vault
            .last_check_in
            .checked_add(interval_seconds)
            .ok_or_else(|| error!(DeadSwitchError::MathOverflow))?;

        require!(
            clock.unix_timestamp >= deadline,
            DeadSwitchError::IntervalNotElapsed
        );

        vault.sol_amount = 0;

        // Close vault to backup wallet, returning all lamports in the account.
        vault.close(ctx.accounts.backup_wallet.to_account_info())?;

        Ok(())
    }

    pub fn cancel(ctx: Context<Cancel>) -> Result<()> {
        let clock = Clock::get()?;
        let vault = &mut ctx.accounts.vault;
        let interval_seconds = interval_to_seconds(vault.interval_days)?;
        let deadline = vault
            .last_check_in
            .checked_add(interval_seconds)
            .ok_or_else(|| error!(DeadSwitchError::MathOverflow))?;

        require!(
            clock.unix_timestamp < deadline,
            DeadSwitchError::VaultExpired
        );

        vault.sol_amount = 0;

        // Owner can recover funds and close the vault while still active.
        vault.close(ctx.accounts.owner.to_account_info())?;

        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(backup_wallet: Pubkey, _interval_days: u64, _deposit_lamports: u64)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        init,
        payer = owner,
        space = Vault::LEN,
        seeds = [b"vault", owner.key().as_ref(), backup_wallet.as_ref()],
        bump
    )]
    pub vault: Account<'info, Vault>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CheckIn<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        mut,
        seeds = [b"vault", vault.owner.as_ref(), vault.backup_wallet.as_ref()],
        bump = vault.bump,
        has_one = owner
    )]
    pub vault: Account<'info, Vault>,
}

#[derive(Accounts)]
pub struct Claim<'info> {
    #[account(mut)]
    pub backup_wallet: Signer<'info>,

    #[account(
        mut,
        seeds = [b"vault", vault.owner.as_ref(), vault.backup_wallet.as_ref()],
        bump = vault.bump,
        constraint = vault.backup_wallet == backup_wallet.key() @ DeadSwitchError::UnauthorizedBackup
    )]
    pub vault: Account<'info, Vault>,
}

#[derive(Accounts)]
pub struct Cancel<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        mut,
        seeds = [b"vault", vault.owner.as_ref(), vault.backup_wallet.as_ref()],
        bump = vault.bump,
        has_one = owner
    )]
    pub vault: Account<'info, Vault>,
}

#[account]
pub struct Vault {
    pub owner: Pubkey,
    pub backup_wallet: Pubkey,
    pub last_check_in: i64,
    pub interval_days: u64,
    pub sol_amount: u64,
    pub bump: u8,
}

impl Vault {
    pub const LEN: usize = 8 + 32 + 32 + 8 + 8 + 8 + 1;
}

fn interval_to_seconds(interval_days: u64) -> Result<i64> {
    let interval_days_i64 = i64::try_from(interval_days).map_err(|_| DeadSwitchError::MathOverflow)?;

    interval_days_i64
        .checked_mul(SECONDS_PER_DAY)
        .ok_or_else(|| error!(DeadSwitchError::MathOverflow))
}

#[error_code]
pub enum DeadSwitchError {
    #[msg("The claim interval has not elapsed yet.")]
    IntervalNotElapsed,
    #[msg("Only the configured backup wallet may claim this vault.")]
    UnauthorizedBackup,
    #[msg("The vault is expired and can no longer be canceled by the owner.")]
    VaultExpired,
    #[msg("The owner and backup wallet must be different.")]
    OwnerIsBackup,
    #[msg("Deposit amount must be greater than zero.")]
    ZeroDeposit,
    #[msg("Math overflow occurred while calculating time interval.")]
    MathOverflow,
}
