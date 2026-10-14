use anchor_lang::prelude::*;
use anchor_spl::token::{mint_to, Mint, MintTo, Token, TokenAccount};

use crate::constants::*;
use crate::errors::HerdError;
use crate::events::*;
use crate::state::*;

/// Herkes cagirabilir (permissionless). Sadece toplamlari hesaplar;
/// kullanici basina dagitim `claim_stampede` ile cekilir (pull modeli,
/// binlerce emri tek islemde dolasmak mumkun degil).
#[derive(Accounts)]
pub struct SettleStampede<'info> {
    pub caller: Signer<'info>,
    #[account(mut, seeds = [LAUNCH_SEED, launch.mint.as_ref()], bump = launch.bump)]
    pub launch: Account<'info, Launch>,

    #[account(mut, address = launch.mint)]
    pub mint: Account<'info, Mint>,

    /// Launch PDA'sinin ATA'si (create_launch'ta acildi).
    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = launch
    )]
    pub stampede_escrow: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

pub fn settle_stampede(ctx: Context<SettleStampede>) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    let l = &mut ctx.accounts.launch;
    require!(l.state == LaunchState::Lobby, HerdError::InvalidState);
    require!(now >= l.lobby_end, HerdError::LobbyNotEnded);

    let success = l.total_deposited >= l.min_lamports;
    if success {
        // Tek clearing price = accepted_total / stampede_tokens.
        // Talep cap'i asarsa fazlasi claim'de orantili iade edilir.
        l.accepted_total = l.total_deposited.min(l.cap_lamports);
        let curve_tokens = u64::try_from(
            (TOTAL_SUPPLY as u128) * (CURVE_SUPPLY_BPS as u128) / BPS_DENOM as u128,
        )
        .map_err(|_| HerdError::MathOverflow)?;
        // Curve baslangic fiyati = Stampede clearing price (ilk alicilar aninda zarara girmez).
        let (vs0, vt0, floor) =
            crate::curve::init_reserves(l.accepted_total, l.stampede_tokens, curve_tokens)?;
        l.virtual_sol = vs0;
        l.virtual_token = vt0;
        l.virtual_token_floor = floor;
        l.initial_virtual_sol = vs0;
        l.state = LaunchState::Curve;
    } else {
        l.accepted_total = 0;
        l.state = LaunchState::Failed;
    }

    // Basarili ise Stampede token'larinin TAMAMI escrow'a basilir. Boylece claim'ler mint
    // yetkisine bagli degildir; breakout'ta mint yetkisi claim'lerden bagimsiz iptal edilebilir.
    if success {
        let mint_key = ctx.accounts.mint.key();
        let bump = ctx.accounts.launch.bump;
        let amount = ctx.accounts.launch.stampede_tokens;
        let seeds: &[&[u8]] = &[LAUNCH_SEED, mint_key.as_ref(), &[bump]];
        mint_to(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                MintTo {
                    mint: ctx.accounts.mint.to_account_info(),
                    to: ctx.accounts.stampede_escrow.to_account_info(),
                    authority: ctx.accounts.launch.to_account_info(),
                },
                &[seeds],
            ),
            amount,
        )?;
    }
    let l = &ctx.accounts.launch;

    emit!(StampedeSettled {
        launch: l.key(),
        success,
        total_deposited: l.total_deposited,
        accepted_total: l.accepted_total,
        stampede_tokens: l.stampede_tokens,
        depositors: l.depositors,
    });
    Ok(())
}
