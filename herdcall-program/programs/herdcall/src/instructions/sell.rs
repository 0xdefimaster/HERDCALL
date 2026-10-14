use anchor_lang::prelude::*;
use anchor_spl::token::{burn, Burn, Mint, Token, TokenAccount};

use crate::constants::*;
use crate::curve;
use crate::errors::HerdError;
use crate::events::*;
use crate::fees::split_fee;
use crate::state::*;
use crate::trade::{assert_launch_rent, credit, debit, route_fees};

#[derive(Accounts)]
pub struct Sell<'info> {
    #[account(mut)]
    pub trader: Signer<'info>,

    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, Config>,

    #[account(mut, seeds = [LAUNCH_SEED, launch.mint.as_ref()], bump = launch.bump)]
    pub launch: Account<'info, Launch>,

    #[account(mut, address = launch.mint)]
    pub mint: Account<'info, Mint>,

    #[account(mut, token::mint = mint, token::authority = trader)]
    pub trader_token: Account<'info, TokenAccount>,

    /// CHECK: creator cuzdani; adres launch'tan dogrulanir
    #[account(mut, address = launch.creator)]
    pub creator: UncheckedAccount<'info>,

    /// CHECK: arayuz / referans cuzdani
    #[account(mut, constraint = ui_receiver.key() != launch.key())]
    pub ui_receiver: UncheckedAccount<'info>,

    #[account(mut, seeds = [REKT_POOL_SEED], bump = rekt_pool.bump)]
    pub rekt_pool: Account<'info, RektPool>,

    #[account(mut, seeds = [PROTOCOL_POOL_SEED], bump = protocol_pool.bump)]
    pub protocol_pool: Account<'info, ProtocolPool>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

pub fn sell(ctx: Context<Sell>, tokens_in: u64, min_sol_out: u64) -> Result<()> {
    require!(!ctx.accounts.config.paused, HerdError::Paused);
    let l = &ctx.accounts.launch;
    require!(l.state == LaunchState::Curve, HerdError::InvalidState);

    let q = curve::quote_sell(l.virtual_sol, l.virtual_token, tokens_in)?;
    require!(q.gross_out > 0, HerdError::TradeTooSmall);

    // Solvency: odeme, curve'in gercek SOL rezervini asamaz (iade bekleyen Stampede fazlasi ayri).
    let real = curve::real_sol(l.accepted_total, l.virtual_sol, l.initial_virtual_sol)
        - l.refund_rounding_buffer() as i128;
    require!(real >= q.gross_out as i128, HerdError::InsufficientCurveReserve);

    let split = split_fee(q.gross_out, l.bonded)?;
    let payout = q.gross_out.checked_sub(split.total).ok_or(HerdError::MathOverflow)?;
    require!(payout >= min_sol_out, HerdError::SlippageExceeded);

    // Token'lar yakilir: curve disindaki (Stampede) token'lar da buraya satilabilir.
    burn(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Burn {
                mint: ctx.accounts.mint.to_account_info(),
                from: ctx.accounts.trader_token.to_account_info(),
                authority: ctx.accounts.trader.to_account_info(),
            },
        ),
        tokens_in,
    )?;

    {
        let l = &mut ctx.accounts.launch;
        l.virtual_sol = q.new_vs;
        l.virtual_token = q.new_vt;
    }

    // Net odeme saticiya, ucretler dagitim hesaplarina; hepsi launch PDA'sindan.
    let launch_info = ctx.accounts.launch.to_account_info();
    debit(&launch_info, payout)?;
    credit(&ctx.accounts.trader.to_account_info(), payout)?;

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
        is_buy: false,
        sol_gross: q.gross_out,
        tokens: tokens_in,
        fee_creator: split.creator,
        fee_ui: split.ui,
        fee_rekt: split.rekt,
        fee_protocol: split.protocol,
        virtual_sol: l.virtual_sol,
        virtual_token: l.virtual_token,
        curve_full: false,
    });
    Ok(())
}
