use anchor_lang::prelude::*;

use crate::constants::*;
use crate::errors::HerdError;
use crate::events::*;
use crate::state::*;

/// DAO havuzundan SOL cikarir. Sadece `Config.authority` (multisig, sonra DAO).
/// Havuz rent-muafiyetinin altina inemez. Hedef serbest; her cekim olayla (event) yayinlanir.
#[derive(Accounts)]
pub struct WithdrawDao<'info> {
    pub authority: Signer<'info>,

    #[account(seeds = [CONFIG_SEED], bump = config.bump, has_one = authority)]
    pub config: Account<'info, Config>,

    #[account(mut, seeds = [DAO_POOL_SEED], bump = dao_pool.bump)]
    pub dao_pool: Account<'info, DaoPool>,

    /// CHECK: hedef cuzdan; authority belirler, adres event'te yayinlanir.
    #[account(mut, constraint = destination.key() != dao_pool.key())]
    pub destination: UncheckedAccount<'info>,
}

pub fn withdraw_dao(ctx: Context<WithdrawDao>, amount: u64) -> Result<()> {
    require!(amount > 0, HerdError::TradeTooSmall);

    let pool_info = ctx.accounts.dao_pool.to_account_info();
    let min = Rent::get()?.minimum_balance(8 + DaoPool::INIT_SPACE);
    let after = pool_info
        .lamports()
        .checked_sub(amount)
        .ok_or(HerdError::InsufficientDaoPool)?;
    require!(after >= min, HerdError::InsufficientDaoPool);

    let dest_info = ctx.accounts.destination.to_account_info();
    **pool_info.try_borrow_mut_lamports()? = after;
    **dest_info.try_borrow_mut_lamports()? = dest_info
        .lamports()
        .checked_add(amount)
        .ok_or(HerdError::MathOverflow)?;

    let p = &mut ctx.accounts.dao_pool;
    p.total_withdrawn = p.total_withdrawn.checked_add(amount).ok_or(HerdError::MathOverflow)?;

    emit!(DaoWithdrawn {
        authority: ctx.accounts.authority.key(),
        destination: ctx.accounts.destination.key(),
        amount,
    });
    Ok(())
}
