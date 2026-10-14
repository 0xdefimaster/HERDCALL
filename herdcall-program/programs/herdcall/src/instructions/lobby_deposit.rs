use anchor_lang::prelude::*;
use anchor_lang::system_program::{transfer, Transfer};

use crate::constants::*;
use crate::errors::HerdError;
use crate::events::*;
use crate::state::*;

#[derive(Accounts)]
pub struct LobbyDeposit<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, Config>,

    #[account(mut, seeds = [LAUNCH_SEED, launch.mint.as_ref()], bump = launch.bump)]
    pub launch: Account<'info, Launch>,

    #[account(
        init_if_needed,
        payer = owner,
        space = 8 + LobbyOrder::INIT_SPACE,
        seeds = [ORDER_SEED, launch.key().as_ref(), owner.key().as_ref()],
        bump
    )]
    pub order: Account<'info, LobbyOrder>,

    pub system_program: Program<'info, System>,
}

pub fn lobby_deposit(ctx: Context<LobbyDeposit>, amount: u64) -> Result<()> {
    require!(!ctx.accounts.config.paused, HerdError::Paused);
    require!(amount >= MIN_DEPOSIT_LAMPORTS, HerdError::DepositTooSmall);

    let now = Clock::get()?.unix_timestamp;
    let launch = &mut ctx.accounts.launch;
    require!(launch.state == LaunchState::Lobby, HerdError::LobbyNotOpen);
    require!(now < launch.lobby_end, HerdError::LobbyNotOpen);

    // SOL launch PDA'sina gider (Lobby escrow'u). Iptal/iade claim'de.
    transfer(
        CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            Transfer {
                from: ctx.accounts.owner.to_account_info(),
                to: launch.to_account_info(),
            },
        ),
        amount,
    )?;

    let order = &mut ctx.accounts.order;
    let first = order.owner == Pubkey::default();
    if first {
        order.launch = launch.key();
        order.owner = ctx.accounts.owner.key();
        order.bump = ctx.bumps.order;
        launch.depositors = launch.depositors.checked_add(1).ok_or(HerdError::MathOverflow)?;
    }
    order.amount = order.amount.checked_add(amount).ok_or(HerdError::MathOverflow)?;
    launch.total_deposited = launch
        .total_deposited
        .checked_add(amount)
        .ok_or(HerdError::MathOverflow)?;

    emit!(LobbyDeposited {
        launch: launch.key(),
        owner: order.owner,
        amount,
        order_total: order.amount,
        launch_total: launch.total_deposited,
        first_deposit: first,
    });
    Ok(())
}
