//! Creator taahhudu: gelir hakki, creator'in token'i program escrow'una kilitlemesine baglidir.
//!
//! Neden escrow? Program, DAMM'deki satislari ve cuzdan hareketlerini goremez; "creator sattimi"
//! cuzdan bakiyesinden guvenilir cikarilamaz (baska cuzdana tasima, claim oncesi geri getirme).
//! Zincir uzerinde dogrulanabilen tek sey escrow'un duruyor olmasidir:
//!  * `commit_tokens`: token escrow'a girer, `committed_tokens` artar.
//!  * `withdraw_commit`: token cikar (satmak/transfer etmek icin tek yol). Taahhut esigin
//!    (`min_commit_tokens`) altina inerse `creator_forfeited` KALICI true olur: creator payi
//!    (islem ucreti + LP ucreti) artik creator'a degil buyback-burn rezervine gider.
//! Taahhut disinda piyasadan alinan token'lar sade holder bakiyesidir, kurala dahil degildir.

use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{transfer, Mint, Token, TokenAccount, Transfer};

use crate::constants::*;
use crate::errors::HerdError;
use crate::events::*;
use crate::state::*;

#[derive(Accounts)]
pub struct CommitTokens<'info> {
    #[account(mut)]
    pub creator: Signer<'info>,

    #[account(
        mut,
        seeds = [LAUNCH_SEED, launch.mint.as_ref()], bump = launch.bump,
        has_one = creator,
        constraint = matches!(
            launch.state,
            LaunchState::Curve | LaunchState::CurveFull | LaunchState::Breakout
        ) @ HerdError::InvalidState
    )]
    pub launch: Box<Account<'info, Launch>>,

    #[account(address = launch.mint)]
    pub mint: Box<Account<'info, Mint>>,

    #[account(mut, token::mint = mint, token::authority = creator)]
    pub creator_token: Box<Account<'info, TokenAccount>>,

    /// CHECK: escrow'un sahibi, veri tasimayan PDA.
    #[account(seeds = [COMMIT_AUTH_SEED, launch.key().as_ref()], bump)]
    pub commit_authority: UncheckedAccount<'info>,

    #[account(
        init_if_needed, payer = creator,
        associated_token::mint = mint, associated_token::authority = commit_authority
    )]
    pub commit_vault: Box<Account<'info, TokenAccount>>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn commit_tokens(ctx: Context<CommitTokens>, amount: u64) -> Result<()> {
    require!(amount > 0, HerdError::TradeTooSmall);
    require!(!ctx.accounts.launch.creator_forfeited, HerdError::CreatorForfeited);

    transfer(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.creator_token.to_account_info(),
                to: ctx.accounts.commit_vault.to_account_info(),
                authority: ctx.accounts.creator.to_account_info(),
            },
        ),
        amount,
    )?;

    let l = &mut ctx.accounts.launch;
    l.committed_tokens = l.committed_tokens.checked_add(amount).ok_or(HerdError::MathOverflow)?;
    emit!(CreatorCommitted {
        launch: l.key(),
        creator: ctx.accounts.creator.key(),
        amount,
        total_committed: l.committed_tokens,
        eligible: l.creator_eligible(),
    });
    Ok(())
}

#[derive(Accounts)]
pub struct WithdrawCommit<'info> {
    pub creator: Signer<'info>,

    #[account(
        mut,
        seeds = [LAUNCH_SEED, launch.mint.as_ref()], bump = launch.bump,
        has_one = creator
    )]
    pub launch: Box<Account<'info, Launch>>,

    #[account(address = launch.mint)]
    pub mint: Box<Account<'info, Mint>>,

    #[account(mut, token::mint = mint, token::authority = creator)]
    pub creator_token: Box<Account<'info, TokenAccount>>,

    /// CHECK: escrow'un sahibi PDA
    #[account(seeds = [COMMIT_AUTH_SEED, launch.key().as_ref()], bump)]
    pub commit_authority: UncheckedAccount<'info>,

    #[account(
        mut,
        associated_token::mint = mint, associated_token::authority = commit_authority
    )]
    pub commit_vault: Box<Account<'info, TokenAccount>>,

    pub token_program: Program<'info, Token>,
}

pub fn withdraw_commit(ctx: Context<WithdrawCommit>, amount: u64) -> Result<()> {
    require!(amount > 0, HerdError::TradeTooSmall);
    let l = &ctx.accounts.launch;
    require!(amount <= l.committed_tokens, HerdError::InsufficientCommit);

    let launch_key = l.key();
    let bump = ctx.bumps.commit_authority;
    let seeds: &[&[u8]] = &[COMMIT_AUTH_SEED, launch_key.as_ref(), &[bump]];
    transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.commit_vault.to_account_info(),
                to: ctx.accounts.creator_token.to_account_info(),
                authority: ctx.accounts.commit_authority.to_account_info(),
            },
            &[seeds],
        ),
        amount,
    )?;

    let l = &mut ctx.accounts.launch;
    l.committed_tokens = l.committed_tokens.checked_sub(amount).ok_or(HerdError::MathOverflow)?;
    let mut forfeited_now = false;
    if !l.creator_forfeited && l.committed_tokens < l.min_commit_tokens {
        l.creator_forfeited = true; // geri donusu yok
        forfeited_now = true;
    }
    emit!(CreatorCommitWithdrawn {
        launch: l.key(),
        creator: ctx.accounts.creator.key(),
        amount,
        total_committed: l.committed_tokens,
        forfeited_now,
    });
    Ok(())
}
