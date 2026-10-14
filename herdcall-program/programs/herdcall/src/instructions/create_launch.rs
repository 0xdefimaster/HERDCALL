use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{Mint, Token, TokenAccount};

use crate::constants::*;
use crate::errors::HerdError;
use crate::events::*;
use crate::state::*;

#[derive(Accounts)]
pub struct CreateLaunch<'info> {
    #[account(mut)]
    pub creator: Signer<'info>,

    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, Config>,

    /// Mint authority = launch PDA. Freeze authority HIC atanmaz (None):
    /// dokumandaki "breakout'ta kaldirilir" kuralindan daha siki, ilk andan itibaren yok.
    #[account(
        init,
        payer = creator,
        mint::decimals = TOKEN_DECIMALS,
        mint::authority = launch,
    )]
    pub mint: Account<'info, Mint>,

    #[account(
        init,
        payer = creator,
        space = 8 + Launch::INIT_SPACE,
        seeds = [LAUNCH_SEED, mint.key().as_ref()],
        bump
    )]
    pub launch: Account<'info, Launch>,

    /// Stampede token'larinin escrow'u (launch PDA'sinin ATA'si). Token'lar settle'da
    /// buraya basilir; claim'de buradan transfer edilir. Creator rent'i oder.
    #[account(
        init,
        payer = creator,
        associated_token::mint = mint,
        associated_token::authority = launch
    )]
    pub stampede_escrow: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn create_launch(ctx: Context<CreateLaunch>, lobby_secs: i64, bonded: bool) -> Result<()> {
    let cfg = &ctx.accounts.config;
    require!(!cfg.paused, HerdError::Paused);
    require!(LOBBY_DURATIONS.contains(&lobby_secs), HerdError::InvalidLobbyDuration);
    // Bond + vesting + escrow yol haritasi 3. adimda; o zamana kadar bonded acilamaz,
    // yoksa "Bellwether" rozeti dayanaksiz kalir.
    require!(!bonded, HerdError::BondedNotYetSupported);

    let now = Clock::get()?.unix_timestamp;
    let stampede_tokens = ((TOTAL_SUPPLY as u128) * (STAMPEDE_SUPPLY_BPS as u128)
        / BPS_DENOM as u128) as u64;

    let l = &mut ctx.accounts.launch;
    l.creator = ctx.accounts.creator.key();
    l.mint = ctx.accounts.mint.key();
    l.state = LaunchState::Lobby;
    l.bonded = bonded;
    l.lobby_start = now;
    l.lobby_end = now.checked_add(lobby_secs).ok_or(HerdError::MathOverflow)?;
    l.min_lamports = cfg.min_stampede_lamports;
    l.cap_lamports = cfg.stampede_cap_lamports;
    l.claim_fee_bps = cfg.claim_fee_bps;
    l.total_deposited = 0;
    l.depositors = 0;
    l.accepted_total = 0;
    l.stampede_tokens = stampede_tokens;
    l.virtual_sol = 0;
    l.virtual_token = 0;
    l.virtual_token_floor = 0;
    l.initial_virtual_sol = 0;
    l.dex_pool = Pubkey::default();
    l.dex_position = Pubkey::default();
    l.position_nft_mint = Pubkey::default();
    l.creator_payout = l.creator;
    l.creator_burn_bps = 0;
    l.committed_tokens = 0;
    l.min_commit_tokens = u64::try_from(
        (TOTAL_SUPPLY as u128) * (cfg.min_creator_commit_bps as u128) / BPS_DENOM as u128,
    )
    .map_err(|_| HerdError::MathOverflow)?;
    l.creator_forfeited = false;
    l.burn_reserve = 0;
    l.total_bought_back = 0;
    l.bump = ctx.bumps.launch;

    emit!(LaunchCreated {
        launch: l.key(),
        mint: l.mint,
        creator: l.creator,
        bonded,
        lobby_end: l.lobby_end,
    });
    Ok(())
}
