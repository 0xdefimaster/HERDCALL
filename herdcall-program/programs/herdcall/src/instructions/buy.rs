use anchor_lang::prelude::*;
use anchor_lang::system_program::{transfer, Transfer};
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{mint_to, Mint, MintTo, Token, TokenAccount};

use crate::constants::*;
use crate::curve;
use crate::errors::HerdError;
use crate::events::*;
use crate::fees::{split_fee, FeeSplit};
use crate::state::*;
use crate::trade::{assert_launch_rent, route_fees};

#[derive(Accounts)]
pub struct Buy<'info> {
    #[account(mut)]
    pub trader: Signer<'info>,

    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, Config>,

    #[account(mut, seeds = [LAUNCH_SEED, launch.mint.as_ref()], bump = launch.bump)]
    pub launch: Account<'info, Launch>,

    #[account(mut, address = launch.mint)]
    pub mint: Account<'info, Mint>,

    #[account(
        init_if_needed,
        payer = trader,
        associated_token::mint = mint,
        associated_token::authority = trader
    )]
    pub trader_token: Account<'info, TokenAccount>,

    /// CHECK: creator cuzdani; adres launch'tan dogrulanir
    #[account(mut, address = launch.creator)]
    pub creator: UncheckedAccount<'info>,

    /// CHECK: islemi getiren arayuz / terminal / referans cuzdani (§5: %0.10)
    #[account(mut, constraint = ui_receiver.key() != launch.key())]
    pub ui_receiver: UncheckedAccount<'info>,

    #[account(mut, seeds = [REKT_POOL_SEED], bump = rekt_pool.bump)]
    pub rekt_pool: Account<'info, RektPool>,

    #[account(mut, seeds = [PROTOCOL_POOL_SEED], bump = protocol_pool.bump)]
    pub protocol_pool: Account<'info, ProtocolPool>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn buy(ctx: Context<Buy>, gross_in: u64, min_tokens_out: u64) -> Result<()> {
    require!(!ctx.accounts.config.paused, HerdError::Paused);
    require!(gross_in >= MIN_TRADE_LAMPORTS, HerdError::TradeTooSmall);
    let l = &ctx.accounts.launch;
    require!(l.state == LaunchState::Curve, HerdError::InvalidState);

    let q = curve::quote_buy(l.virtual_sol, l.virtual_token, l.virtual_token_floor, gross_in)?;
    require!(q.tokens_out > 0 && q.tokens_out >= min_tokens_out, HerdError::SlippageExceeded);

    // Ucret bolmesi: brut uzerinden %1; yuvarlama artigi protokolde. (q.fee ile ayni toplam)
    let split: FeeSplit = split_fee(q.gross, l.bonded)?;
    debug_assert_eq!(split.total, q.fee);

    // Brut SOL launch PDA'sina; ucretler oradan dagitilir, net curve rezervinde kalir.
    transfer(
        CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            Transfer {
                from: ctx.accounts.trader.to_account_info(),
                to: ctx.accounts.launch.to_account_info(),
            },
        ),
        q.gross,
    )?;

    let mint_key = ctx.accounts.mint.key();
    let bump = ctx.accounts.launch.bump;
    let seeds: &[&[u8]] = &[LAUNCH_SEED, mint_key.as_ref(), &[bump]];
    mint_to(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            MintTo {
                mint: ctx.accounts.mint.to_account_info(),
                to: ctx.accounts.trader_token.to_account_info(),
                authority: ctx.accounts.launch.to_account_info(),
            },
            &[seeds],
        ),
        q.tokens_out,
    )?;

    {
        let l = &mut ctx.accounts.launch;
        l.virtual_sol = q.new_vs;
        l.virtual_token = q.new_vt;
        if q.filled {
            l.state = LaunchState::CurveFull;
        }
    }

    // Creator geliri hak etmiyorsa (taahhut yok / hak kaybi) payi odenmez; launch PDA'sinda
    // kalir ve buyback-burn rezervine yazilir (curve rezervi durumdan hesaplanir, etkilenmez).
    let eligible = ctx.accounts.launch.creator_eligible();
    let mut pay_split = split;
    if !eligible {
        pay_split.creator = 0;
    }
    route_fees(
        &ctx.accounts.launch,
        &ctx.accounts.creator.to_account_info(),
        &ctx.accounts.ui_receiver.to_account_info(),
        &mut ctx.accounts.rekt_pool,
        &mut ctx.accounts.protocol_pool,
        &pay_split,
    )?;
    if !eligible {
        let l = &mut ctx.accounts.launch;
        l.burn_reserve = l.burn_reserve.checked_add(split.creator).ok_or(HerdError::MathOverflow)?;
    }
    assert_launch_rent(&ctx.accounts.launch)?;

    let l = &ctx.accounts.launch;
    emit!(Traded {
        launch: l.key(),
        trader: ctx.accounts.trader.key(),
        is_buy: true,
        sol_gross: q.gross,
        tokens: q.tokens_out,
        fee_creator: split.creator,
        fee_ui: split.ui,
        fee_rekt: split.rekt,
        fee_protocol: split.protocol,
        virtual_sol: l.virtual_sol,
        virtual_token: l.virtual_token,
        curve_full: q.filled,
    });
    Ok(())
}
