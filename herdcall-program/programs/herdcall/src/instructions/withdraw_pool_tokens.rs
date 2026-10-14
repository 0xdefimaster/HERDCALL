//! DaoPool / ProtocolPool'un token ATA'larindan cekim. Sadece `Config.authority`.
//! LP ucretinin token tarafi (`claim_position_fee`) bu ATA'larda birikir; bakiyeler herkese acik,
//! her cekim event'lenir.

use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{transfer, Mint, Token, TokenAccount, Transfer};

use crate::constants::*;
use crate::errors::HerdError;
use crate::events::*;
use crate::state::*;

#[derive(Accounts)]
pub struct WithdrawDaoTokens<'info> {
    pub authority: Signer<'info>,

    #[account(seeds = [CONFIG_SEED], bump = config.bump, has_one = authority)]
    pub config: Account<'info, Config>,

    #[account(seeds = [DAO_POOL_SEED], bump = dao_pool.bump)]
    pub dao_pool: Account<'info, DaoPool>,

    pub mint: Account<'info, Mint>,

    #[account(
        mut,
        associated_token::mint = mint, associated_token::authority = dao_pool
    )]
    pub dao_token: Account<'info, TokenAccount>,

    #[account(mut, token::mint = mint, constraint = destination.key() != dao_token.key())]
    pub destination: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

pub fn withdraw_dao_tokens(ctx: Context<WithdrawDaoTokens>, amount: u64) -> Result<()> {
    require!(amount > 0, HerdError::TradeTooSmall);
    require!(amount <= ctx.accounts.dao_token.amount, HerdError::InsufficientPoolTokens);
    let bump = ctx.accounts.dao_pool.bump;
    let seeds: &[&[u8]] = &[DAO_POOL_SEED, &[bump]];
    transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.dao_token.to_account_info(),
                to: ctx.accounts.destination.to_account_info(),
                authority: ctx.accounts.dao_pool.to_account_info(),
            },
            &[seeds],
        ),
        amount,
    )?;
    emit!(PoolTokensWithdrawn {
        authority: ctx.accounts.authority.key(),
        pool: ctx.accounts.dao_pool.key(),
        destination: ctx.accounts.destination.key(),
        amount,
    });
    Ok(())
}

#[derive(Accounts)]
pub struct WithdrawProtocolTokens<'info> {
    pub authority: Signer<'info>,

    #[account(seeds = [CONFIG_SEED], bump = config.bump, has_one = authority)]
    pub config: Account<'info, Config>,

    #[account(seeds = [PROTOCOL_POOL_SEED], bump = protocol_pool.bump)]
    pub protocol_pool: Account<'info, ProtocolPool>,

    pub mint: Account<'info, Mint>,

    #[account(
        mut,
        associated_token::mint = mint, associated_token::authority = protocol_pool
    )]
    pub protocol_token: Account<'info, TokenAccount>,

    #[account(mut, token::mint = mint, constraint = destination.key() != protocol_token.key())]
    pub destination: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

pub fn withdraw_protocol_tokens(ctx: Context<WithdrawProtocolTokens>, amount: u64) -> Result<()> {
    require!(amount > 0, HerdError::TradeTooSmall);
    require!(amount <= ctx.accounts.protocol_token.amount, HerdError::InsufficientPoolTokens);
    let bump = ctx.accounts.protocol_pool.bump;
    let seeds: &[&[u8]] = &[PROTOCOL_POOL_SEED, &[bump]];
    transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.protocol_token.to_account_info(),
                to: ctx.accounts.destination.to_account_info(),
                authority: ctx.accounts.protocol_pool.to_account_info(),
            },
            &[seeds],
        ),
        amount,
    )?;
    emit!(PoolTokensWithdrawn {
        authority: ctx.accounts.authority.key(),
        pool: ctx.accounts.protocol_pool.key(),
        destination: ctx.accounts.destination.key(),
        amount,
    });
    Ok(())
}
