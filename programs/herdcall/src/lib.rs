//! HerdCall launchpad program (document/STAMPEDE_SPEC.md, document/CURVE_SPEC.md).
//!
//! All math comes from `herdcall-core`, which is tested unit-for-unit against the JS reference model. This crate only
//! moves lamports and tokens and enforces the phase and time rules.
#![allow(unexpected_cfgs)]

use anchor_lang::prelude::*;
use anchor_lang::system_program;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{self, spl_token::instruction::AuthorityType, Burn, Mint, MintTo, SetAuthority, Token, TokenAccount, Transfer};
use herdcall_core::curve::Market;
use herdcall_core::fees::{split_fee, FeeParts};
use herdcall_core::names::{buys_open, is_canonical_ticker, releasable};
use herdcall_core::stampede::{claim_tokens, settle as settle_lobby};
use herdcall_core::BPS;

pub mod errors;
pub mod state;

use errors::{core_err, HerdError};
use state::*;

declare_id!("EPh2hoECkDTsSeHCriDwsF8DhVGhCjxwrbtC3qqyoJ8K");

#[program]
pub mod herdcall {
    use super::*;

    /// One-time setup: default parameters for new launches and the protocol / Rekt-back vaults.
    pub fn initialize(ctx: Context<Initialize>, params: LaunchParams) -> Result<()> {
        validate_params(&params)?;
        let c = &mut ctx.accounts.config;
        c.admin = ctx.accounts.admin.key();
        c.params = params;
        c.bump = ctx.bumps.config;
        ctx.accounts.protocol.bump = ctx.bumps.protocol;
        ctx.accounts.rekt_back.bump = ctx.bumps.rekt_back;
        Ok(())
    }

    /// Creates the token (fixed supply, no mint or freeze authority), reserves its name and ticker against the
    /// refundable bond and opens its Lobby for `lobby_secs` seconds. Fails if another launch holds the name or ticker.
    /// `name_hash` is `name_seed(name)`, computed by the client to address the name record.
    pub fn create_launch(ctx: Context<CreateLaunch>, bonded: bool, lobby_secs: i64, ticker: String, name: String, name_hash: [u8; 32]) -> Result<()> {
        require!(lobby_secs > 0 && lobby_secs <= MAX_LOBBY_SECS, HerdError::InvalidParams);
        require!(is_canonical_ticker(ticker.as_bytes()), HerdError::InvalidTicker);
        require!(name_seed(&name).ok_or(HerdError::InvalidName)? == name_hash, HerdError::NameHashMismatch);
        let now = Clock::get()?.unix_timestamp;
        let params = ctx.accounts.config.params;
        let mint_key = ctx.accounts.mint.key();
        {
            let l = &mut ctx.accounts.launch;
            l.creator = ctx.accounts.creator.key();
            l.mint = mint_key;
            l.bonded = bonded;
            l.phase = Phase::Lobby;
            l.params = params;
            l.close_at = now.checked_add(lobby_secs).ok_or(HerdError::MathOverflow)?;
            l.settle_deadline = l.close_at.checked_add(params.settle_window).ok_or(HerdError::MathOverflow)?;
            l.curve_deadline = l.close_at.checked_add(params.curve_lifetime_secs).ok_or(HerdError::MathOverflow)?;
            l.ticker = ticker.clone();
            l.name = name.clone();
            l.name_bond = params.name_bond;
            l.v = params.v0;
            l.t = params.t0;
            l.k = (params.v0 as u128) * (params.t0 as u128);
            l.lock_ratio_bps = BPS as u16;
            l.bump = ctx.bumps.launch;
            l.sol_vault_bump = ctx.bumps.sol_vault;
            l.creator_fees_bump = ctx.bumps.creator_fees;
            l.creator_escrow_bump = ctx.bumps.creator_escrow;
        }
        let launch_key = ctx.accounts.launch.key();
        ctx.accounts.ticker_record.launch = launch_key;
        ctx.accounts.ticker_record.bump = ctx.bumps.ticker_record;
        ctx.accounts.name_record.launch = launch_key;
        ctx.accounts.name_record.bump = ctx.bumps.name_record;
        pay_from_wallet(&ctx.accounts.creator.to_account_info(), &ctx.accounts.ticker_record.to_account_info(), &ctx.accounts.system_program, params.name_bond)?;
        ctx.accounts.sol_vault.bump = ctx.bumps.sol_vault;
        ctx.accounts.creator_fees.bump = ctx.bumps.creator_fees;
        ctx.accounts.creator_escrow.bump = ctx.bumps.creator_escrow;

        let bump = [ctx.bumps.launch];
        let seeds: &[&[u8]] = &[LAUNCH_SEED, mint_key.as_ref(), &bump];
        let signer = &[seeds];
        token::mint_to(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.key(),
                MintTo {
                    mint: ctx.accounts.mint.to_account_info(),
                    to: ctx.accounts.token_vault.to_account_info(),
                    authority: ctx.accounts.launch.to_account_info(),
                },
                signer,
            ),
            params.total_supply,
        )?;
        token::set_authority(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.key(),
                SetAuthority { current_authority: ctx.accounts.launch.to_account_info(), account_or_mint: ctx.accounts.mint.to_account_info() },
                signer,
            ),
            AuthorityType::MintTokens,
            None,
        )?;
        emit!(LaunchCreated { launch: ctx.accounts.launch.key(), mint: mint_key, creator: ctx.accounts.creator.key(), bonded, close_at: ctx.accounts.launch.close_at, ticker, name });
        Ok(())
    }

    /// Adds SOL to the Lobby pool. No trading, no withdrawals while the Lobby is open.
    pub fn deposit(ctx: Context<DepositCtx>, amount: u64) -> Result<()> {
        let now = Clock::get()?.unix_timestamp;
        let l = &mut ctx.accounts.launch;
        require!(l.phase == Phase::Lobby && now < l.close_at, HerdError::LobbyClosed);
        require!(amount >= l.params.min_deposit, HerdError::BelowMinimum);
        let total = l.total_deposits.checked_add(amount).ok_or(HerdError::MathOverflow)?;
        require!(total <= l.params.max_total, HerdError::LobbyCap);
        let d = &mut ctx.accounts.deposit;
        let mine = d.amount.checked_add(amount).ok_or(HerdError::MathOverflow)?;
        require!(l.params.max_per_wallet == 0 || mine <= l.params.max_per_wallet, HerdError::WalletCap);
        if d.amount == 0 {
            d.launch = l.key();
            d.owner = ctx.accounts.user.key();
            d.bump = ctx.bumps.deposit;
            l.depositors = l.depositors.checked_add(1).ok_or(HerdError::MathOverflow)?;
        }
        d.amount = mine;
        l.total_deposits = total;
        system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.key(),
                system_program::Transfer { from: ctx.accounts.user.to_account_info(), to: ctx.accounts.sol_vault.to_account_info() },
            ),
            amount,
        )?;
        emit!(Deposited { launch: l.key(), user: ctx.accounts.user.key(), amount, total });
        Ok(())
    }

    /// Permissionless. The pool makes one buy at the price the curve opens at (STAMPEDE_SPEC §4).
    pub fn settle(ctx: Context<Settle>) -> Result<()> {
        let now = Clock::get()?.unix_timestamp;
        let l = &mut ctx.accounts.launch;
        require!(l.phase == Phase::Lobby, HerdError::LobbyClosed);
        require!(now >= l.close_at, HerdError::LobbyOpen);
        require!(now < l.settle_deadline, HerdError::SettleDeadlinePassed);
        let (s, curve) = settle_lobby(&l.params.lobby(), l.total_deposits as u128).map_err(core_err)?;
        l.settled_empty = s.empty;
        l.lobby_tokens = to_u64(s.x)?;
        l.store_curve(&curve)?;
        l.phase = Phase::Curve;
        let parts = split_fee(s.f, l.kind(), false, l.lock_ratio_bps as u128).map_err(core_err)?;
        let sol_vault = ctx.accounts.sol_vault.to_account_info();
        pay_parts_from_vault(
            &sol_vault,
            &parts,
            &FeeAccounts {
                creator_fees: ctx.accounts.creator_fees.to_account_info(),
                creator_escrow: ctx.accounts.creator_escrow.to_account_info(),
                rekt_back: ctx.accounts.rekt_back.to_account_info(),
                protocol: ctx.accounts.protocol.to_account_info(),
                interface: None,
            },
        )?;
        emit!(Settled { launch: l.key(), total: l.total_deposits, lobby_tokens: l.lobby_tokens, net: to_u64(s.n)?, fee: to_u64(s.f)?, empty: s.empty });
        Ok(())
    }

    /// Pull claim after the Stampede: tokens pro rata (or the full deposit back if the Lobby was empty).
    pub fn claim(ctx: Context<Claim>) -> Result<()> {
        let l = &mut ctx.accounts.launch;
        require!(l.phase != Phase::Lobby && l.phase != Phase::Cancelled, HerdError::NotSettled);
        let d = &mut ctx.accounts.deposit;
        require!(!d.withdrawn, HerdError::AlreadyWithdrawn);
        d.withdrawn = true;
        if l.settled_empty {
            pay_from_vault(&ctx.accounts.sol_vault.to_account_info(), &ctx.accounts.user.to_account_info(), d.amount)?;
            return Ok(());
        }
        let q = to_u64(claim_tokens(d.amount as u128, l.total_deposits as u128, l.lobby_tokens as u128).map_err(core_err)?)?;
        l.lobby_tokens_claimed = l.lobby_tokens_claimed.checked_add(q).ok_or(HerdError::MathOverflow)?;
        let mint_key = l.mint;
        let bump = [l.bump];
        let seeds: &[&[u8]] = &[LAUNCH_SEED, mint_key.as_ref(), &bump];
        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.key(),
                Transfer { from: ctx.accounts.token_vault.to_account_info(), to: ctx.accounts.user_ata.to_account_info(), authority: l.to_account_info() },
                &[seeds],
            ),
            q,
        )?;
        Ok(())
    }

    /// After the settlement window without a settlement: the Lobby is cancelled and everyone takes the deposit back.
    pub fn refund(ctx: Context<Refund>) -> Result<()> {
        let now = Clock::get()?.unix_timestamp;
        let l = &mut ctx.accounts.launch;
        if l.phase == Phase::Lobby {
            require!(now >= l.settle_deadline, HerdError::CannotCancelYet);
            l.phase = Phase::Cancelled;
        }
        require!(l.phase == Phase::Cancelled, HerdError::NotSettled);
        let d = &mut ctx.accounts.deposit;
        require!(!d.withdrawn, HerdError::AlreadyWithdrawn);
        d.withdrawn = true;
        pay_from_vault(&ctx.accounts.sol_vault.to_account_info(), &ctx.accounts.user.to_account_info(), d.amount)?;
        Ok(())
    }

    /// Buys with exactly `gross` lamports (fee included); unused lamports never leave the wallet.
    pub fn buy_exact_in(ctx: Context<Trade>, gross: u64, min_tokens: u64) -> Result<()> {
        require_buys_open(&ctx.accounts.launch)?;
        let has_iface = ctx.accounts.interface.is_some();
        let (mut m, kind, lock) = market(&ctx.accounts.launch)?;
        let q = m.buy_exact_in(gross as u128, min_tokens as u128, has_iface).map_err(core_err)?;
        settle_buy(ctx, &m, kind, lock, to_u64(q.tokens)?, to_u64(q.cost)?, q.fee, has_iface)
    }

    /// Buys exactly `tokens` (clamped to what is left on the curve), paying at most `max_gross` lamports.
    pub fn buy_exact_out(ctx: Context<Trade>, tokens: u64, max_gross: u64) -> Result<()> {
        require_buys_open(&ctx.accounts.launch)?;
        let has_iface = ctx.accounts.interface.is_some();
        let (mut m, kind, lock) = market(&ctx.accounts.launch)?;
        let q = m.buy_exact_out(tokens as u128, max_gross as u128, has_iface).map_err(core_err)?;
        settle_buy(ctx, &m, kind, lock, to_u64(q.tokens)?, to_u64(q.cost)?, q.fee, has_iface)
    }

    /// Sells `tokens`, receiving at least `min_received` lamports after the fee. Open after the curve deadline too.
    pub fn sell(ctx: Context<Trade>, tokens: u64, min_received: u64) -> Result<()> {
        let has_iface = ctx.accounts.interface.is_some();
        let (mut m, kind, lock) = market(&ctx.accounts.launch)?;
        let q = m.sell(tokens as u128, min_received as u128, has_iface).map_err(core_err)?;
        let parts = split_fee(q.fee, kind, has_iface, lock).map_err(core_err)?;
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.key(),
                Transfer {
                    from: ctx.accounts.trader_ata.to_account_info(),
                    to: ctx.accounts.token_vault.to_account_info(),
                    authority: ctx.accounts.trader.to_account_info(),
                },
            ),
            tokens,
        )?;
        let sol_vault = ctx.accounts.sol_vault.to_account_info();
        pay_from_vault(&sol_vault, &ctx.accounts.trader.to_account_info(), to_u64(q.received)?)?;
        pay_parts_from_vault(&sol_vault, &parts, &fee_accounts(ctx.accounts))?;
        let l = &mut ctx.accounts.launch;
        l.store_curve(&m.curve)?;
        l.store_phase(m.phase);
        emit!(Traded { launch: l.key(), trader: ctx.accounts.trader.key(), buy: false, tokens, native: to_u64(q.out)?, fee: to_u64(q.fee)?, v: l.v, t: l.t });
        Ok(())
    }

    /// Permissionless, once. Pays the keeper, burns the LP tokens the pool does not need and sets the rest aside for
    /// the Meteora DAMM v2 pool at the curve's final price (CURVE_SPEC §5.4).
    pub fn breakout(ctx: Context<Breakout>) -> Result<()> {
        require!(ctx.accounts.launch.phase == Phase::BreakingOut, HerdError::NotBreakingOut);
        let (mut m, _, _) = market(&ctx.accounts.launch)?;
        let mig = m.breakout().map_err(core_err)?;
        pay_from_vault(&ctx.accounts.sol_vault.to_account_info(), &ctx.accounts.keeper.to_account_info(), to_u64(mig.keeper_fee)?)?;
        let l = &ctx.accounts.launch;
        let mint_key = l.mint;
        let bump = [l.bump];
        let seeds: &[&[u8]] = &[LAUNCH_SEED, mint_key.as_ref(), &bump];
        if mig.burned > 0 {
            token::burn(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.key(),
                    Burn { mint: ctx.accounts.mint.to_account_info(), from: ctx.accounts.token_vault.to_account_info(), authority: l.to_account_info() },
                    &[seeds],
                ),
                to_u64(mig.burned)?,
            )?;
        }
        // Name and ticker become permanent; the bond goes back to the creator.
        ctx.accounts.ticker_record.permanent = true;
        ctx.accounts.name_record.permanent = true;
        let bond = ctx.accounts.launch.name_bond;
        pay_from_vault(&ctx.accounts.ticker_record.to_account_info(), &ctx.accounts.creator.to_account_info(), bond)?;
        let l = &mut ctx.accounts.launch;
        l.lp_native = to_u64(mig.lp_native)?;
        l.lp_tokens = to_u64(mig.lp_tokens)?;
        l.burned = to_u64(mig.burned)?;
        l.name_bond = 0;
        l.store_curve(&m.curve)?;
        l.phase = Phase::Migrating;
        emit!(BrokeOut { launch: l.key(), keeper: ctx.accounts.keeper.key(), lp_native: l.lp_native, lp_tokens: l.lp_tokens, burned: l.burned });
        Ok(())
    }

    /// Permissionless. Frees the name and ticker of a launch that can no longer break out: its Lobby expired
    /// unsettled, or its curve passed the deadline (sells only from then on). Both records close; their rent and the
    /// bond go back to the creator.
    pub fn release_name(ctx: Context<ReleaseName>) -> Result<()> {
        let now = Clock::get()?.unix_timestamp;
        let l = &mut ctx.accounts.launch;
        require!(
            !ctx.accounts.ticker_record.permanent && releasable(l.name_status(now), now, l.curve_deadline),
            HerdError::NameNotReleasable
        );
        l.name_bond = 0;
        emit!(NameReleased { launch: l.key(), ticker: l.ticker.clone(), name: l.name.clone() });
        Ok(())
    }

    /// Registers a trading interface that can receive the interface share of fees.
    pub fn register_interface(ctx: Context<RegisterInterface>) -> Result<()> {
        let i = &mut ctx.accounts.interface;
        i.owner = ctx.accounts.owner.key();
        i.bump = ctx.bumps.interface;
        Ok(())
    }

    /// The interface owner withdraws accumulated fees.
    pub fn withdraw_interface_fees(ctx: Context<WithdrawInterface>) -> Result<()> {
        let acc = ctx.accounts.interface.to_account_info();
        let amount = withdrawable(&acc)?;
        pay_from_vault(&acc, &ctx.accounts.owner.to_account_info(), amount)
    }

    /// The creator withdraws the immediately payable part of their fees (escrow rules: Bonded Creator spec).
    pub fn withdraw_creator_fees(ctx: Context<WithdrawCreatorFees>) -> Result<()> {
        let acc = ctx.accounts.creator_fees.to_account_info();
        let amount = withdrawable(&acc)?;
        pay_from_vault(&acc, &ctx.accounts.creator.to_account_info(), amount)
    }
}

// ------------------------------------------------------------------ helpers

fn validate_params(p: &LaunchParams) -> Result<()> {
    let ok = p.curve_supply.checked_add(p.lp_supply) == Some(p.total_supply)
        && p.v0 > 0
        && p.t0 > 0
        && p.t0 <= p.total_supply
        && (p.curve_supply as u128) * 2 >= p.t0 as u128
        && (p.fee_bps as u128) < BPS
        && p.min_deposit > 0
        && p.min_total >= p.min_deposit
        && p.max_total >= p.min_total
        && p.settle_window > 0
        && p.curve_lifetime_secs > p.settle_window;
    require!(ok, HerdError::InvalidParams);
    Ok(())
}

fn require_buys_open(l: &Launch) -> Result<()> {
    require!(buys_open(Clock::get()?.unix_timestamp, l.curve_deadline), HerdError::BuysClosed);
    Ok(())
}

fn market(l: &Launch) -> Result<(Market, herdcall_core::fees::Kind, u128)> {
    require!(l.phase == Phase::Curve || l.phase == Phase::BreakingOut, HerdError::TradingClosed);
    let m = Market::new(l.curve(), l.params.market(l.kind())).map_err(core_err)?;
    Ok((m, l.kind(), l.lock_ratio_bps as u128))
}

#[allow(clippy::too_many_arguments)]
fn settle_buy(ctx: Context<Trade>, m: &Market, kind: herdcall_core::fees::Kind, lock: u128, tokens: u64, cost: u64, fee: u128, has_iface: bool) -> Result<()> {
    let parts = split_fee(fee, kind, has_iface, lock).map_err(core_err)?;
    let a = &ctx.accounts;
    let trader = a.trader.to_account_info();
    pay_from_wallet(&trader, &a.sol_vault.to_account_info(), &a.system_program, cost)?;
    for (to, amount) in targets(&fee_accounts(a), &parts)? {
        pay_from_wallet(&trader, &to, &a.system_program, amount)?;
    }
    let mint_key = a.launch.mint;
    let bump = [a.launch.bump];
    let seeds: &[&[u8]] = &[LAUNCH_SEED, mint_key.as_ref(), &bump];
    token::transfer(
        CpiContext::new_with_signer(
            a.token_program.key(),
            Transfer { from: a.token_vault.to_account_info(), to: a.trader_ata.to_account_info(), authority: a.launch.to_account_info() },
            &[seeds],
        ),
        tokens,
    )?;
    let l = &mut ctx.accounts.launch;
    l.store_curve(&m.curve)?;
    l.store_phase(m.phase);
    emit!(Traded { launch: l.key(), trader: ctx.accounts.trader.key(), buy: true, tokens, native: cost, fee: to_u64(fee)?, v: l.v, t: l.t });
    Ok(())
}

struct FeeAccounts<'info> {
    creator_fees: AccountInfo<'info>,
    creator_escrow: AccountInfo<'info>,
    rekt_back: AccountInfo<'info>,
    protocol: AccountInfo<'info>,
    interface: Option<AccountInfo<'info>>,
}

fn fee_accounts<'info>(a: &Trade<'info>) -> FeeAccounts<'info> {
    FeeAccounts {
        creator_fees: a.creator_fees.to_account_info(),
        creator_escrow: a.creator_escrow.to_account_info(),
        rekt_back: a.rekt_back.to_account_info(),
        protocol: a.protocol.to_account_info(),
        interface: a.interface.as_ref().map(|i| i.to_account_info()),
    }
}

fn targets<'info>(f: &FeeAccounts<'info>, p: &FeeParts) -> Result<Vec<(AccountInfo<'info>, u64)>> {
    let mut out = vec![
        (f.creator_fees.clone(), to_u64(p.creator_now)?),
        (f.creator_escrow.clone(), to_u64(p.creator_escrow)?),
        (f.rekt_back.clone(), to_u64(p.rekt_back)?),
        (f.protocol.clone(), to_u64(p.protocol)?),
    ];
    if p.interface > 0 {
        let i = f.interface.clone().ok_or(HerdError::WrongAccount)?;
        out.push((i, to_u64(p.interface)?));
    }
    Ok(out)
}

fn pay_parts_from_vault<'info>(vault: &AccountInfo<'info>, p: &FeeParts, f: &FeeAccounts<'info>) -> Result<()> {
    for (to, amount) in targets(f, p)? {
        pay_from_vault(vault, &to, amount)?;
    }
    Ok(())
}

/// Moves lamports out of a program-owned vault, never below its rent-exempt minimum.
fn pay_from_vault(vault: &AccountInfo, to: &AccountInfo, amount: u64) -> Result<()> {
    if amount == 0 {
        return Ok(());
    }
    let floor = Rent::get()?.minimum_balance(vault.data_len());
    let left = vault.lamports().checked_sub(amount).ok_or(HerdError::VaultUnderfunded)?;
    require!(left >= floor, HerdError::VaultUnderfunded);
    **vault.try_borrow_mut_lamports()? = left;
    **to.try_borrow_mut_lamports()? = to.lamports().checked_add(amount).ok_or(HerdError::MathOverflow)?;
    Ok(())
}

fn pay_from_wallet<'info>(from: &AccountInfo<'info>, to: &AccountInfo<'info>, system: &Program<'info, System>, amount: u64) -> Result<()> {
    if amount == 0 {
        return Ok(());
    }
    system_program::transfer(CpiContext::new(system.key(), system_program::Transfer { from: from.clone(), to: to.clone() }), amount)
}

fn withdrawable(acc: &AccountInfo) -> Result<u64> {
    let floor = Rent::get()?.minimum_balance(acc.data_len());
    Ok(acc.lamports().saturating_sub(floor))
}

// ------------------------------------------------------------------ accounts

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,
    #[account(init, payer = admin, space = 8 + Config::INIT_SPACE, seeds = [CONFIG_SEED], bump)]
    pub config: Account<'info, Config>,
    #[account(init, payer = admin, space = 8 + Vault::INIT_SPACE, seeds = [PROTOCOL_SEED], bump)]
    pub protocol: Account<'info, Vault>,
    #[account(init, payer = admin, space = 8 + Vault::INIT_SPACE, seeds = [REKT_BACK_SEED], bump)]
    pub rekt_back: Account<'info, Vault>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(bonded: bool, lobby_secs: i64, ticker: String, name: String, name_hash: [u8; 32])]
pub struct CreateLaunch<'info> {
    #[account(mut)]
    pub creator: Signer<'info>,
    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, Config>,
    #[account(init, payer = creator, mint::decimals = TOKEN_DECIMALS, mint::authority = launch)]
    pub mint: Account<'info, Mint>,
    #[account(init, payer = creator, space = 8 + Launch::INIT_SPACE, seeds = [LAUNCH_SEED, mint.key().as_ref()], bump)]
    pub launch: Account<'info, Launch>,
    #[account(init, payer = creator, associated_token::mint = mint, associated_token::authority = launch)]
    pub token_vault: Account<'info, TokenAccount>,
    #[account(init, payer = creator, space = 8 + Vault::INIT_SPACE, seeds = [SOL_VAULT_SEED, launch.key().as_ref()], bump)]
    pub sol_vault: Account<'info, Vault>,
    #[account(init, payer = creator, space = 8 + Vault::INIT_SPACE, seeds = [CREATOR_FEES_SEED, launch.key().as_ref()], bump)]
    pub creator_fees: Account<'info, Vault>,
    #[account(init, payer = creator, space = 8 + Vault::INIT_SPACE, seeds = [CREATOR_ESCROW_SEED, launch.key().as_ref()], bump)]
    pub creator_escrow: Account<'info, Vault>,
    #[account(init, payer = creator, space = 8 + NameRecord::INIT_SPACE, seeds = [TICKER_SEED, ticker.as_bytes()], bump)]
    pub ticker_record: Account<'info, NameRecord>,
    #[account(init, payer = creator, space = 8 + NameRecord::INIT_SPACE, seeds = [NAME_SEED, name_hash.as_ref()], bump)]
    pub name_record: Account<'info, NameRecord>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct DepositCtx<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(mut, seeds = [LAUNCH_SEED, launch.mint.as_ref()], bump = launch.bump)]
    pub launch: Account<'info, Launch>,
    #[account(mut, seeds = [SOL_VAULT_SEED, launch.key().as_ref()], bump = launch.sol_vault_bump)]
    pub sol_vault: Account<'info, Vault>,
    #[account(init_if_needed, payer = user, space = 8 + Deposit::INIT_SPACE, seeds = [DEPOSIT_SEED, launch.key().as_ref(), user.key().as_ref()], bump)]
    pub deposit: Account<'info, Deposit>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Settle<'info> {
    #[account(mut, seeds = [LAUNCH_SEED, launch.mint.as_ref()], bump = launch.bump)]
    pub launch: Account<'info, Launch>,
    #[account(mut, seeds = [SOL_VAULT_SEED, launch.key().as_ref()], bump = launch.sol_vault_bump)]
    pub sol_vault: Account<'info, Vault>,
    #[account(mut, seeds = [CREATOR_FEES_SEED, launch.key().as_ref()], bump = launch.creator_fees_bump)]
    pub creator_fees: Account<'info, Vault>,
    #[account(mut, seeds = [CREATOR_ESCROW_SEED, launch.key().as_ref()], bump = launch.creator_escrow_bump)]
    pub creator_escrow: Account<'info, Vault>,
    #[account(mut, seeds = [REKT_BACK_SEED], bump = rekt_back.bump)]
    pub rekt_back: Account<'info, Vault>,
    #[account(mut, seeds = [PROTOCOL_SEED], bump = protocol.bump)]
    pub protocol: Account<'info, Vault>,
}

#[derive(Accounts)]
pub struct Claim<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(mut, seeds = [LAUNCH_SEED, launch.mint.as_ref()], bump = launch.bump)]
    pub launch: Account<'info, Launch>,
    #[account(address = launch.mint)]
    pub mint: Account<'info, Mint>,
    #[account(mut, seeds = [DEPOSIT_SEED, launch.key().as_ref(), user.key().as_ref()], bump = deposit.bump, has_one = owner @ HerdError::WrongAccount)]
    pub deposit: Account<'info, Deposit>,
    /// CHECK: equals `user`, enforced by `has_one`.
    #[account(address = user.key())]
    pub owner: UncheckedAccount<'info>,
    #[account(mut, seeds = [SOL_VAULT_SEED, launch.key().as_ref()], bump = launch.sol_vault_bump)]
    pub sol_vault: Account<'info, Vault>,
    #[account(mut, associated_token::mint = mint, associated_token::authority = launch)]
    pub token_vault: Account<'info, TokenAccount>,
    #[account(init_if_needed, payer = user, associated_token::mint = mint, associated_token::authority = user)]
    pub user_ata: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Refund<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(mut, seeds = [LAUNCH_SEED, launch.mint.as_ref()], bump = launch.bump)]
    pub launch: Account<'info, Launch>,
    #[account(mut, seeds = [DEPOSIT_SEED, launch.key().as_ref(), user.key().as_ref()], bump = deposit.bump)]
    pub deposit: Account<'info, Deposit>,
    #[account(mut, seeds = [SOL_VAULT_SEED, launch.key().as_ref()], bump = launch.sol_vault_bump)]
    pub sol_vault: Account<'info, Vault>,
}

#[derive(Accounts)]
pub struct Trade<'info> {
    #[account(mut)]
    pub trader: Signer<'info>,
    #[account(mut, seeds = [LAUNCH_SEED, launch.mint.as_ref()], bump = launch.bump)]
    pub launch: Account<'info, Launch>,
    #[account(address = launch.mint)]
    pub mint: Account<'info, Mint>,
    #[account(mut, seeds = [SOL_VAULT_SEED, launch.key().as_ref()], bump = launch.sol_vault_bump)]
    pub sol_vault: Account<'info, Vault>,
    #[account(mut, associated_token::mint = mint, associated_token::authority = launch)]
    pub token_vault: Account<'info, TokenAccount>,
    #[account(init_if_needed, payer = trader, associated_token::mint = mint, associated_token::authority = trader)]
    pub trader_ata: Account<'info, TokenAccount>,
    #[account(mut, seeds = [CREATOR_FEES_SEED, launch.key().as_ref()], bump = launch.creator_fees_bump)]
    pub creator_fees: Account<'info, Vault>,
    #[account(mut, seeds = [CREATOR_ESCROW_SEED, launch.key().as_ref()], bump = launch.creator_escrow_bump)]
    pub creator_escrow: Account<'info, Vault>,
    #[account(mut, seeds = [REKT_BACK_SEED], bump = rekt_back.bump)]
    pub rekt_back: Account<'info, Vault>,
    #[account(mut, seeds = [PROTOCOL_SEED], bump = protocol.bump)]
    pub protocol: Account<'info, Vault>,
    #[account(mut, seeds = [INTERFACE_SEED, interface.owner.as_ref()], bump = interface.bump)]
    pub interface: Option<Account<'info, TradingInterface>>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Breakout<'info> {
    #[account(mut)]
    pub keeper: Signer<'info>,
    #[account(mut, seeds = [LAUNCH_SEED, launch.mint.as_ref()], bump = launch.bump)]
    pub launch: Account<'info, Launch>,
    #[account(mut, address = launch.mint)]
    pub mint: Account<'info, Mint>,
    #[account(mut, associated_token::mint = mint, associated_token::authority = launch)]
    pub token_vault: Account<'info, TokenAccount>,
    #[account(mut, seeds = [SOL_VAULT_SEED, launch.key().as_ref()], bump = launch.sol_vault_bump)]
    pub sol_vault: Account<'info, Vault>,
    // A launch that can break out still holds both records: they are only released once it cannot.
    #[account(mut, seeds = [TICKER_SEED, launch.ticker.as_bytes()], bump = ticker_record.bump, has_one = launch @ HerdError::WrongAccount)]
    pub ticker_record: Account<'info, NameRecord>,
    #[account(mut, seeds = [NAME_SEED, name_seed(&launch.name).ok_or(HerdError::InvalidName)?.as_ref()], bump = name_record.bump, has_one = launch @ HerdError::WrongAccount)]
    pub name_record: Account<'info, NameRecord>,
    /// CHECK: receives the bond back; must be the launch creator.
    #[account(mut, address = launch.creator @ HerdError::WrongAccount)]
    pub creator: UncheckedAccount<'info>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct ReleaseName<'info> {
    #[account(mut, seeds = [TICKER_SEED, launch.ticker.as_bytes()], bump = ticker_record.bump, has_one = launch @ HerdError::WrongAccount, close = creator)]
    pub ticker_record: Account<'info, NameRecord>,
    #[account(mut, seeds = [NAME_SEED, name_seed(&launch.name).ok_or(HerdError::InvalidName)?.as_ref()], bump = name_record.bump, has_one = launch @ HerdError::WrongAccount, close = creator)]
    pub name_record: Account<'info, NameRecord>,
    #[account(mut, seeds = [LAUNCH_SEED, launch.mint.as_ref()], bump = launch.bump, has_one = creator @ HerdError::WrongAccount)]
    pub launch: Account<'info, Launch>,
    /// CHECK: receives rent and bond; must be the launch creator (`has_one` above).
    #[account(mut)]
    pub creator: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct RegisterInterface<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,
    #[account(init, payer = owner, space = 8 + TradingInterface::INIT_SPACE, seeds = [INTERFACE_SEED, owner.key().as_ref()], bump)]
    pub interface: Account<'info, TradingInterface>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct WithdrawInterface<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,
    #[account(mut, seeds = [INTERFACE_SEED, owner.key().as_ref()], bump = interface.bump, has_one = owner)]
    pub interface: Account<'info, TradingInterface>,
}

#[derive(Accounts)]
pub struct WithdrawCreatorFees<'info> {
    #[account(mut)]
    pub creator: Signer<'info>,
    #[account(seeds = [LAUNCH_SEED, launch.mint.as_ref()], bump = launch.bump, has_one = creator)]
    pub launch: Account<'info, Launch>,
    #[account(mut, seeds = [CREATOR_FEES_SEED, launch.key().as_ref()], bump = launch.creator_fees_bump)]
    pub creator_fees: Account<'info, Vault>,
}

// ------------------------------------------------------------------ events

#[event]
pub struct LaunchCreated {
    pub launch: Pubkey,
    pub mint: Pubkey,
    pub creator: Pubkey,
    pub bonded: bool,
    pub close_at: i64,
    pub ticker: String,
    pub name: String,
}

#[event]
pub struct NameReleased {
    pub launch: Pubkey,
    pub ticker: String,
    pub name: String,
}

#[event]
pub struct Deposited {
    pub launch: Pubkey,
    pub user: Pubkey,
    pub amount: u64,
    pub total: u64,
}

#[event]
pub struct Settled {
    pub launch: Pubkey,
    pub total: u64,
    pub lobby_tokens: u64,
    pub net: u64,
    pub fee: u64,
    pub empty: bool,
}

#[event]
pub struct Traded {
    pub launch: Pubkey,
    pub trader: Pubkey,
    pub buy: bool,
    pub tokens: u64,
    pub native: u64,
    pub fee: u64,
    pub v: u64,
    pub t: u64,
}

#[event]
pub struct BrokeOut {
    pub launch: Pubkey,
    pub keeper: Pubkey,
    pub lp_native: u64,
    pub lp_tokens: u64,
    pub burned: u64,
}
