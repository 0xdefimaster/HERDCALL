//! HerdCall launchpad program (document/STAMPEDE_SPEC.md, document/CURVE_SPEC.md).
//!
//! All math comes from `herdcall-core`, which is tested unit-for-unit against the JS reference model. This crate only
//! moves lamports and tokens and enforces the phase and time rules.
#![allow(unexpected_cfgs)]

use anchor_lang::prelude::*;
use anchor_lang::system_program;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::metadata::mpl_token_metadata::types::DataV2;
use anchor_spl::metadata::{create_metadata_accounts_v3, CreateMetadataAccountsV3, Metadata};
use anchor_spl::token::{self, spl_token::instruction::AuthorityType, Burn, Mint, MintTo, SetAuthority, Token, TokenAccount, Transfer};
use anchor_spl::token::spl_token::native_mint;
use anchor_spl::token::{CloseAccount, SyncNative};
use anchor_spl::token_2022::Token2022;
use herdcall_core::curve::Market;
use herdcall_core::dex;
use herdcall_core::fees::{split_fee, split_lp_fees, FeeParts};
use herdcall_core::names::{is_canonical_ticker, releasable};
use herdcall_core::stampede::{claim_tokens, settle as settle_lobby};
use herdcall_core::BPS;

pub mod damm;
pub mod errors;
pub mod state;

use errors::{core_err, HerdError};
use state::*;

declare_id!("EPh2hoECkDTsSeHCriDwsF8DhVGhCjxwrbtC3qqyoJ8K");

#[program]
pub mod herdcall {
    use super::*;

    /// One-time setup: default parameters for new launches, the DAMM v2 config for breakouts and the protocol /
    /// Rekt-back vaults.
    pub fn initialize(ctx: Context<Initialize>, params: LaunchParams, dex_config: Pubkey) -> Result<()> {
        validate_params(&params)?;
        let c = &mut ctx.accounts.config;
        c.admin = ctx.accounts.admin.key();
        c.params = params;
        c.dex_config = dex_config;
        c.reinvest_keeper = ctx.accounts.admin.key();
        c.bump = ctx.bumps.config;
        ctx.accounts.protocol.bump = ctx.bumps.protocol;
        ctx.accounts.rekt_back.bump = ctx.bumps.rekt_back;
        Ok(())
    }

    /// Creates the token (fixed supply, no mint or freeze authority, immutable Metaplex metadata: name, ticker as
    /// symbol, `uri`), reserves its name and ticker against the refundable bond and opens its Lobby for `lobby_secs`
    /// seconds. Fails if another launch holds the name or ticker. `name_hash` is `name_seed(name)`, computed by the
    /// client to address the name record.
    #[allow(clippy::too_many_arguments)]
    pub fn create_launch(
        ctx: Context<CreateLaunch>,
        bonded: bool,
        lobby_secs: i64,
        ticker: String,
        name: String,
        name_hash: [u8; 32],
        uri: String,
    ) -> Result<()> {
        require!(lobby_secs > 0 && lobby_secs <= MAX_LOBBY_SECS, HerdError::InvalidParams);
        require!(is_canonical_ticker(ticker.as_bytes()), HerdError::InvalidTicker);
        require!(name_seed(&name).ok_or(HerdError::InvalidName)? == name_hash, HerdError::NameHashMismatch);
        require!(uri.len() <= MAX_URI_LEN, HerdError::InvalidUri);
        let name = name.trim().to_string();
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
        // Metadata needs the mint authority, so it comes before the authority is dropped. Immutable: nobody can change
        // the name, symbol or image later, and there is no update authority to hand over.
        create_metadata_accounts_v3(
            CpiContext::new_with_signer(
                ctx.accounts.token_metadata_program.key(),
                CreateMetadataAccountsV3 {
                    metadata: ctx.accounts.metadata.to_account_info(),
                    mint: ctx.accounts.mint.to_account_info(),
                    mint_authority: ctx.accounts.launch.to_account_info(),
                    payer: ctx.accounts.creator.to_account_info(),
                    update_authority: ctx.accounts.launch.to_account_info(),
                    system_program: ctx.accounts.system_program.to_account_info(),
                    rent: ctx.accounts.rent.to_account_info(),
                },
                signer,
            ),
            DataV2 { name: name.clone(), symbol: ticker.clone(), uri, seller_fee_basis_points: 0, creators: None, collection: None, uses: None },
            false,
            true,
            None,
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

    /// Permissionless. The pool makes one buy at the price the curve opens at (STAMPEDE_SPEC §4); the name and ticker
    /// become permanent and the bond goes back to the creator. A Lobby below the minimum opens no curve: the launch is
    /// cancelled, deposits are refunded and the name and ticker can be released.
    pub fn settle(ctx: Context<Settle>) -> Result<()> {
        let now = Clock::get()?.unix_timestamp;
        let l = &mut ctx.accounts.launch;
        require!(l.phase == Phase::Lobby, HerdError::LobbyClosed);
        require!(now >= l.close_at, HerdError::LobbyOpen);
        require!(now < l.settle_deadline, HerdError::SettleDeadlinePassed);
        let (s, curve) = settle_lobby(&l.params.lobby(), l.total_deposits as u128).map_err(core_err)?;
        if s.empty {
            l.phase = Phase::Cancelled;
            emit!(Settled { launch: l.key(), total: l.total_deposits, lobby_tokens: 0, net: 0, fee: 0, empty: true });
            return Ok(());
        }
        // The curve opens: name and ticker belong to this launch for good, and the bond has done its job.
        ctx.accounts.ticker_record.permanent = true;
        ctx.accounts.name_record.permanent = true;
        pay_from_vault(&ctx.accounts.ticker_record.to_account_info(), &ctx.accounts.creator.to_account_info(), l.name_bond)?;
        l.name_bond = 0;
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

    /// Pull claim after the Stampede: tokens pro rata.
    pub fn claim(ctx: Context<Claim>) -> Result<()> {
        let l = &mut ctx.accounts.launch;
        require!(l.phase != Phase::Lobby && l.phase != Phase::Cancelled, HerdError::NotSettled);
        let d = &mut ctx.accounts.deposit;
        require!(!d.withdrawn, HerdError::AlreadyWithdrawn);
        d.withdrawn = true;
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

    /// A cancelled Lobby (below the minimum, or not settled before its deadline): everyone takes the deposit back.
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
        let has_iface = ctx.accounts.interface.is_some();
        let (mut m, kind, lock) = market(&ctx.accounts.launch)?;
        let q = m.buy_exact_in(gross as u128, min_tokens as u128, has_iface).map_err(core_err)?;
        settle_buy(ctx, &m, kind, lock, to_u64(q.tokens)?, to_u64(q.cost)?, q.fee, has_iface)
    }

    /// Buys exactly `tokens` (clamped to what is left on the curve), paying at most `max_gross` lamports.
    pub fn buy_exact_out(ctx: Context<Trade>, tokens: u64, max_gross: u64) -> Result<()> {
        let has_iface = ctx.accounts.interface.is_some();
        let (mut m, kind, lock) = market(&ctx.accounts.launch)?;
        let q = m.buy_exact_out(tokens as u128, max_gross as u128, has_iface).map_err(core_err)?;
        settle_buy(ctx, &m, kind, lock, to_u64(q.tokens)?, to_u64(q.cost)?, q.fee, has_iface)
    }

    /// Sells `tokens`, receiving at least `min_received` lamports after the fee.
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

    /// Points future breakouts to another DAMM v2 config (for instance if Meteora retires one). It must still pass the
    /// checks in `damm::read_config` at breakout, so the admin cannot pick a public config or a fee above 2%.
    pub fn set_dex_config(ctx: Context<AdminOnly>, dex_config: Pubkey) -> Result<()> {
        ctx.accounts.config.dex_config = dex_config;
        Ok(())
    }

    /// Sets the keeper that runs `reinvest_lp` (a protocol service; see CURVE_SPEC §5.7). This is how reinvesting keeps
    /// going if a keeper stops: there is no permissionless fallback.
    pub fn set_reinvest_keeper(ctx: Context<AdminOnly>, keeper: Pubkey) -> Result<()> {
        ctx.accounts.config.reinvest_keeper = keeper;
        Ok(())
    }

    /// Permissionless, once. Opens the Meteora DAMM v2 pool at the curve's final price with the reserve and the LP
    /// tokens, locks the whole position for good (it keeps earning fees; its NFT stays with the launch PDA) and burns
    /// the LP tokens the pool does not need (CURVE_SPEC §5.4). The migration fee pays the pool's rent and the keeper
    /// keeps the rest; the keeper fronts `BREAKOUT_FLOAT` for the length of the call and gets it back.
    pub fn breakout(ctx: Context<Breakout>) -> Result<()> {
        require!(ctx.accounts.launch.phase == Phase::BreakingOut, HerdError::NotBreakingOut);
        let migrator_key = ctx.accounts.migrator.key();
        let cfg = damm::read_config(&ctx.accounts.dex_config.to_account_info(), &migrator_key)?;
        let (mut m, _, _) = market(&ctx.accounts.launch)?;
        let mig = m.breakout().map_err(core_err)?;
        let (v, t) = mig.final_spot;
        let sqrt_price = dex::sqrt_price_ceil(v, t).map_err(core_err)?;
        let (lp_tokens, lp_native, keeper_fee) = (to_u64(mig.lp_tokens)?, to_u64(mig.lp_native)?, to_u64(mig.keeper_fee)?);
        let dep = dex::fit(lp_tokens, lp_native, cfg.sqrt_min_price, sqrt_price, cfg.sqrt_max_price).map_err(core_err)?;

        let a = &ctx.accounts;
        let launch_key = a.launch.key();
        let mint_key = a.launch.mint;
        let launch_bump = [a.launch.bump];
        let launch_seeds: &[&[u8]] = &[LAUNCH_SEED, mint_key.as_ref(), &launch_bump];
        let migrator_bump = [ctx.bumps.migrator];
        let migrator_seeds: &[&[u8]] = &[MIGRATOR_SEED, &migrator_bump];
        let nft_bump = [ctx.bumps.position_nft_mint];
        let nft_seeds: &[&[u8]] = &[POSITION_NFT_SEED, launch_key.as_ref(), &nft_bump];
        let token_program = a.token_program.key();
        let launch_info = a.launch.to_account_info();
        let migrator = a.migrator.to_account_info();
        let sol_vault = a.sol_vault.to_account_info();

        // LP tokens the pool does not need.
        if mig.burned > 0 {
            token::burn(
                CpiContext::new_with_signer(
                    token_program,
                    Burn { mint: a.mint.to_account_info(), from: a.token_vault.to_account_info(), authority: launch_info.clone() },
                    &[launch_seeds],
                ),
                to_u64(mig.burned)?,
            )?;
        }
        // The migrator holds the LP tokens and wrapped SOL, and the migration fee to pay the pool's rent.
        token::transfer(
            CpiContext::new_with_signer(
                token_program,
                Transfer { from: a.token_vault.to_account_info(), to: a.migrator_token.to_account_info(), authority: launch_info.clone() },
                &[launch_seeds],
            ),
            lp_tokens,
        )?;
        // A CPI must see both sides of a direct lamport move: the zero transfer back records the vault's debit and the
        // migrator's credit together; the migrator then wraps the LP SOL itself.
        let system = a.system_program.key();
        pay_from_vault(&sol_vault, &migrator, lp_native.checked_add(keeper_fee).ok_or(HerdError::MathOverflow)?)?;
        transfer_signed(system, &migrator, &sol_vault, 0, migrator_seeds)?;
        transfer_signed(system, &migrator, &a.migrator_wsol.to_account_info(), lp_native, migrator_seeds)?;
        // The keeper fronts a float so the pool's rent is covered even if it grows; it comes back below.
        system_program::transfer(
            CpiContext::new(system, system_program::Transfer { from: a.keeper.to_account_info(), to: migrator.clone() }),
            BREAKOUT_FLOAT,
        )?;
        token::sync_native(CpiContext::new(token_program, SyncNative { account: a.migrator_wsol.to_account_info() }))?;

        // Pool at the curve's final price; the launch PDA owns the position NFT.
        damm::initialize_pool(
            &damm::InitializePool {
                creator: &launch_info,
                position_nft_mint: &a.position_nft_mint.to_account_info(),
                position_nft_account: &a.position_nft_account.to_account_info(),
                payer: &migrator,
                config: &a.dex_config.to_account_info(),
                pool_authority: &a.pool_authority.to_account_info(),
                pool: &a.pool.to_account_info(),
                position: &a.position.to_account_info(),
                token_a_mint: &a.mint.to_account_info(),
                token_b_mint: &a.native_mint.to_account_info(),
                token_a_vault: &a.pool_token_vault.to_account_info(),
                token_b_vault: &a.pool_wsol_vault.to_account_info(),
                payer_token_a: &a.migrator_token.to_account_info(),
                payer_token_b: &a.migrator_wsol.to_account_info(),
                token_program: &a.token_program.to_account_info(),
                token_2022_program: &a.token_2022_program.to_account_info(),
                system_program: &a.system_program.to_account_info(),
                event_authority: &a.dex_event_authority.to_account_info(),
                program: &a.dex_program.to_account_info(),
            },
            dep.liquidity,
            dep.sqrt_price,
            &[migrator_seeds, nft_seeds],
        )?;
        damm::permanent_lock_position(
            &a.pool.to_account_info(),
            &a.position.to_account_info(),
            &a.position_nft_account.to_account_info(),
            &launch_info,
            &a.dex_event_authority.to_account_info(),
            &a.dex_program.to_account_info(),
            dep.liquidity,
            &[launch_seeds],
        )?;

        // Leftovers: tokens the pool did not take are burned; the few lamports of SOL go to the protocol; the rest of
        // the migration fee and the temporary accounts' rent go to the keeper. The migrator ends empty.
        let left_tokens = lp_tokens - dep.a;
        if left_tokens > 0 {
            token::burn(
                CpiContext::new_with_signer(
                    token_program,
                    Burn { mint: a.mint.to_account_info(), from: a.migrator_token.to_account_info(), authority: migrator.clone() },
                    &[migrator_seeds],
                ),
                left_tokens,
            )?;
        }
        token::close_account(CpiContext::new_with_signer(
            token_program,
            CloseAccount { account: a.migrator_token.to_account_info(), destination: a.keeper.to_account_info(), authority: migrator.clone() },
            &[migrator_seeds],
        ))?;
        token::close_account(CpiContext::new_with_signer(
            token_program,
            CloseAccount { account: a.migrator_wsol.to_account_info(), destination: migrator.clone(), authority: migrator.clone() },
            &[migrator_seeds],
        ))?;
        let left_sol = lp_native - dep.b;
        if left_sol > 0 {
            transfer_signed(system, &migrator, &a.protocol.to_account_info(), left_sol, migrator_seeds)?;
        }
        let rest = migrator.lamports();
        if rest > 0 {
            transfer_signed(system, &migrator, &a.keeper.to_account_info(), rest, migrator_seeds)?;
        }

        let pool = a.pool.key();
        let position = a.position.key();
        let keeper = a.keeper.key();
        let l = &mut ctx.accounts.launch;
        l.lp_native = dep.b;
        l.lp_tokens = dep.a;
        l.lp_liquidity = dep.liquidity;
        l.burned = to_u64(mig.burned)?.checked_add(left_tokens).ok_or(HerdError::MathOverflow)?;
        l.dex_pool = pool;
        l.dex_position = position;
        l.store_curve(&m.curve)?;
        l.phase = Phase::Migrated;
        emit!(BrokeOut { launch: l.key(), keeper, pool, lp_native: l.lp_native, lp_tokens: l.lp_tokens, burned: l.burned });
        Ok(())
    }

    /// Permissionless. Claims the locked position's fees (SOL only: the pool collects fees in token B) and splits them
    /// as CURVE_SPEC §5.8 says. Creator, Rekt-back and protocol are paid now; the "back to liquidity" share stays in
    /// the launch's SOL vault (`lp_reinvest_pending`) until it is added to the position. The caller pays the temporary
    /// wrapped SOL account's rent and gets it back.
    pub fn claim_lp_fees(ctx: Context<ClaimLpFees>) -> Result<()> {
        require!(ctx.accounts.launch.phase == Phase::Migrated, HerdError::NotMigrated);
        let a = &ctx.accounts;
        let mint_key = a.launch.mint;
        let bump = [a.launch.bump];
        let launch_seeds: &[&[u8]] = &[LAUNCH_SEED, mint_key.as_ref(), &bump];
        let launch_info = a.launch.to_account_info();
        let token_before = a.token_vault.amount;
        damm::claim_position_fee(
            &damm::ClaimPositionFee {
                pool_authority: &a.pool_authority.to_account_info(),
                pool: &a.pool.to_account_info(),
                position: &a.position.to_account_info(),
                token_a_account: &a.token_vault.to_account_info(),
                token_b_account: &a.fee_wsol.to_account_info(),
                token_a_vault: &a.pool_token_vault.to_account_info(),
                token_b_vault: &a.pool_wsol_vault.to_account_info(),
                token_a_mint: &a.mint.to_account_info(),
                token_b_mint: &a.native_mint.to_account_info(),
                position_nft_account: &a.position_nft_account.to_account_info(),
                owner: &launch_info,
                token_program: &a.token_program.to_account_info(),
                event_authority: &a.dex_event_authority.to_account_info(),
                program: &a.dex_program.to_account_info(),
            },
            &[launch_seeds],
        )?;
        let mut fee_wsol = a.fee_wsol.clone();
        fee_wsol.reload()?;
        let mut token_vault = a.token_vault.clone();
        token_vault.reload()?;
        // Fees in token A would mix with unclaimed Lobby tokens; the config only allows fees in SOL.
        require!(token_vault.amount == token_before, HerdError::InvalidDexConfig);
        let fee = fee_wsol.amount;
        require!(fee > 0, HerdError::NoFees);

        // Unwrap into the SOL vault, give the caller the account's rent back, then split.
        let sol_vault = a.sol_vault.to_account_info();
        let rent_back = a.fee_wsol.to_account_info().lamports().checked_sub(fee).ok_or(HerdError::MathOverflow)?;
        token::close_account(CpiContext::new_with_signer(
            a.token_program.key(),
            CloseAccount { account: a.fee_wsol.to_account_info(), destination: sol_vault.clone(), authority: launch_info.clone() },
            &[launch_seeds],
        ))?;
        pay_from_vault(&sol_vault, &a.caller.to_account_info(), rent_back)?;
        let parts = split_lp_fees(fee as u128, a.launch.kind(), a.launch.lock_ratio_bps as u128).map_err(core_err)?;
        pay_from_vault(&sol_vault, &a.creator_fees.to_account_info(), to_u64(parts.creator)?)?;
        pay_from_vault(&sol_vault, &a.rekt_back.to_account_info(), to_u64(parts.rekt_back)?)?;
        pay_from_vault(&sol_vault, &a.protocol.to_account_info(), to_u64(parts.protocol)?)?;

        let l = &mut ctx.accounts.launch;
        l.lp_fees_claimed = l.lp_fees_claimed.checked_add(fee).ok_or(HerdError::MathOverflow)?;
        l.lp_reinvest_pending = l.lp_reinvest_pending.checked_add(to_u64(parts.liquidity)?).ok_or(HerdError::MathOverflow)?;
        emit!(LpFeesClaimed {
            launch: l.key(),
            fee,
            liquidity: to_u64(parts.liquidity)?,
            creator: to_u64(parts.creator)?,
            rekt_back: to_u64(parts.rekt_back)?,
            protocol: to_u64(parts.protocol)?,
            forfeited: to_u64(parts.forfeited)?,
        });
        Ok(())
    }

    /// Adds the "back to liquidity" share of the claimed fees to the locked position (CURVE_SPEC §5.7): half of the
    /// amount buys tokens in the pool, then SOL and tokens are added and the new liquidity is locked for good.
    ///
    /// DAMM v2 has no TWAP or oracle, so the program cannot tell a fair price from one moved just before the call. The
    /// keeper (`Config::reinvest_keeper`) signs and passes a price band (`min_sqrt_price`..`max_sqrt_price`, at most 2%
    /// wide) from its own observations, and the pool price must be inside it. Nobody else can reinvest: without a
    /// keeper the share waits as SOL until the admin appoints one (`set_reinvest_keeper`). Limits checked here: nothing
    /// below 0.01 SOL; per call at
    /// most min(0.5%, pool fee / 2) of the pool's SOL; the swap returns at least the spot amount less the pool fee and
    /// 0.5%, moves the price by at most 0.6% and buys at most 0.5% of the pool's tokens. They bound what one call
    /// puts at risk; they are not a guarantee.
    pub fn reinvest_lp(ctx: Context<ReinvestLp>, min_sqrt_price: u128, max_sqrt_price: u128) -> Result<()> {
        require!(ctx.accounts.launch.phase == Phase::Migrated, HerdError::NotMigrated);
        let a = &ctx.accounts;
        require_keys_eq!(a.caller.key(), a.config.reinvest_keeper, HerdError::KeeperOnly);
        require!(dex::band_ok(min_sqrt_price, max_sqrt_price), HerdError::BandTooWide);
        let before = damm::read_pool(&a.pool.to_account_info())?;
        require!(min_sqrt_price <= before.sqrt_price && before.sqrt_price <= max_sqrt_price, HerdError::PriceOutOfBand);
        let (pool_sol, pool_tokens) =
            dex::reserves(before.liquidity, before.sqrt_min_price, before.sqrt_price, before.sqrt_max_price).map_err(core_err)?;
        let cap_bps = dex::reinvest_cap_bps(before.fee_numerator);
        let amount = dex::reinvest_amount(a.launch.lp_reinvest_pending, pool_sol, cap_bps).ok_or(HerdError::NothingToReinvest)?;
        let swap_in = amount / 2;
        let min_out = dex::reinvest_min_out(swap_in, before.sqrt_price, before.fee_numerator).map_err(core_err)?;

        let mint_key = a.launch.mint;
        let bump = [a.launch.bump];
        let launch_seeds: &[&[u8]] = &[LAUNCH_SEED, mint_key.as_ref(), &bump];
        let migrator_bump = [ctx.bumps.migrator];
        let migrator_seeds: &[&[u8]] = &[MIGRATOR_SEED, &migrator_bump];
        let launch_info = a.launch.to_account_info();
        let migrator = a.migrator.to_account_info();
        let sol_vault = a.sol_vault.to_account_info();
        let system = a.system_program.key();
        let token_program = a.token_program.key();
        let wsol = a.reinvest_wsol.to_account_info();
        let tokens = a.reinvest_token.to_account_info();

        // Wrap the amount through the migrator so a CPI sees both sides of the vault's lamport move (see breakout).
        pay_from_vault(&sol_vault, &migrator, amount)?;
        transfer_signed(system, &migrator, &sol_vault, 0, migrator_seeds)?;
        transfer_signed(system, &migrator, &wsol, amount, migrator_seeds)?;
        token::sync_native(CpiContext::new(token_program, SyncNative { account: wsol.clone() }))?;

        let pool_accounts = damm::PoolAccounts {
            pool_authority: &a.pool_authority.to_account_info(),
            pool: &a.pool.to_account_info(),
            token_a_vault: &a.pool_token_vault.to_account_info(),
            token_b_vault: &a.pool_wsol_vault.to_account_info(),
            token_a_mint: &a.mint.to_account_info(),
            token_b_mint: &a.native_mint.to_account_info(),
            token_program: &a.token_program.to_account_info(),
            event_authority: &a.dex_event_authority.to_account_info(),
            program: &a.dex_program.to_account_info(),
        };
        damm::swap(&pool_accounts, &wsol, &tokens, &launch_info, swap_in, min_out, &[launch_seeds])?;
        let mut bought_acc = a.reinvest_token.clone();
        bought_acc.reload()?;
        let bought = bought_acc.amount;
        let after = damm::read_pool(&a.pool.to_account_info())?;
        require!(
            bought as u128 <= pool_tokens * dex::REINVEST_MAX_TOKEN_BPS / BPS && dex::price_move_ok(before.sqrt_price, after.sqrt_price),
            HerdError::ReinvestLimit
        );

        // Add what fits at the new price and lock it with the rest of the position.
        let sol_left = amount - swap_in;
        let dep = dex::fit(bought, sol_left, after.sqrt_min_price, after.sqrt_price, after.sqrt_max_price).map_err(core_err)?;
        damm::add_liquidity(
            &pool_accounts,
            &a.position.to_account_info(),
            &tokens,
            &wsol,
            &a.position_nft_account.to_account_info(),
            &launch_info,
            dep.liquidity,
            bought,
            sol_left,
            &[launch_seeds],
        )?;
        damm::permanent_lock_position(
            &a.pool.to_account_info(),
            &a.position.to_account_info(),
            &a.position_nft_account.to_account_info(),
            &launch_info,
            &a.dex_event_authority.to_account_info(),
            &a.dex_program.to_account_info(),
            dep.liquidity,
            &[launch_seeds],
        )?;

        // Leftovers: a few token units are burned; unused SOL goes back to the vault and keeps waiting; the caller gets
        // the temporary accounts' rent back.
        let left_tokens = bought - dep.a;
        if left_tokens > 0 {
            token::burn(
                CpiContext::new_with_signer(
                    token_program,
                    Burn { mint: a.mint.to_account_info(), from: tokens.clone(), authority: launch_info.clone() },
                    &[launch_seeds],
                ),
                left_tokens,
            )?;
        }
        token::close_account(CpiContext::new_with_signer(
            token_program,
            CloseAccount { account: tokens.clone(), destination: a.caller.to_account_info(), authority: launch_info.clone() },
            &[launch_seeds],
        ))?;
        let left_sol = sol_left - dep.b;
        let wsol_rent = wsol.lamports().checked_sub(left_sol).ok_or(HerdError::MathOverflow)?;
        token::close_account(CpiContext::new_with_signer(
            token_program,
            CloseAccount { account: wsol.clone(), destination: sol_vault.clone(), authority: launch_info.clone() },
            &[launch_seeds],
        ))?;
        pay_from_vault(&sol_vault, &a.caller.to_account_info(), wsol_rent)?;

        let used = swap_in.checked_add(dep.b).ok_or(HerdError::MathOverflow)?;
        let l = &mut ctx.accounts.launch;
        l.lp_reinvest_pending = l.lp_reinvest_pending.checked_sub(used).ok_or(HerdError::MathOverflow)?;
        l.lp_reinvested = l.lp_reinvested.checked_add(used).ok_or(HerdError::MathOverflow)?;
        l.lp_liquidity = l.lp_liquidity.checked_add(dep.liquidity).ok_or(HerdError::MathOverflow)?;
        l.burned = l.burned.checked_add(left_tokens).ok_or(HerdError::MathOverflow)?;
        emit!(LpReinvested { launch: l.key(), sol: used, swapped: swap_in, tokens: dep.a, liquidity: dep.liquidity, pending: l.lp_reinvest_pending });
        Ok(())
    }

    /// Permissionless. Frees the name and ticker of a cancelled launch (Lobby below the minimum, or not settled before
    /// its deadline). Once the curve opens they are permanent. Both records close; their rent and the bond go back to
    /// the creator.
    pub fn release_name(ctx: Context<ReleaseName>) -> Result<()> {
        let now = Clock::get()?.unix_timestamp;
        let l = &mut ctx.accounts.launch;
        require!(
            !ctx.accounts.ticker_record.permanent && releasable(l.name_status(now)),
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
        && p.settle_window > 0;
    require!(ok, HerdError::InvalidParams);
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

/// System transfer from a system-owned PDA that signs with `seeds`.
fn transfer_signed<'info>(system: Pubkey, from: &AccountInfo<'info>, to: &AccountInfo<'info>, amount: u64, seeds: &[&[u8]]) -> Result<()> {
    system_program::transfer(
        CpiContext::new_with_signer(system, system_program::Transfer { from: from.clone(), to: to.clone() }, &[seeds]),
        amount,
    )
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
#[instruction(bonded: bool, lobby_secs: i64, ticker: String, name: String, name_hash: [u8; 32], uri: String)]
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
    /// CHECK: the mint's Metaplex metadata PDA, created by the Token Metadata program.
    #[account(mut, seeds = [b"metadata", token_metadata_program.key().as_ref(), mint.key().as_ref()], bump, seeds::program = token_metadata_program.key())]
    pub metadata: UncheckedAccount<'info>,
    pub token_metadata_program: Program<'info, Metadata>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
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
    #[account(mut, seeds = [TICKER_SEED, launch.ticker.as_bytes()], bump = ticker_record.bump, has_one = launch @ HerdError::WrongAccount)]
    pub ticker_record: Account<'info, NameRecord>,
    #[account(mut, seeds = [NAME_SEED, name_seed(&launch.name).ok_or(HerdError::InvalidName)?.as_ref()], bump = name_record.bump, has_one = launch @ HerdError::WrongAccount)]
    pub name_record: Account<'info, NameRecord>,
    /// CHECK: receives the bond back when the curve opens; must be the launch creator.
    #[account(mut, address = launch.creator @ HerdError::WrongAccount)]
    pub creator: UncheckedAccount<'info>,
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
pub struct AdminOnly<'info> {
    pub admin: Signer<'info>,
    #[account(mut, seeds = [CONFIG_SEED], bump = config.bump, has_one = admin)]
    pub config: Account<'info, Config>,
}

#[derive(Accounts)]
pub struct Breakout<'info> {
    #[account(mut)]
    pub keeper: Signer<'info>,
    #[account(mut, seeds = [LAUNCH_SEED, launch.mint.as_ref()], bump = launch.bump)]
    pub launch: Box<Account<'info, Launch>>,
    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Box<Account<'info, Config>>,
    #[account(mut, address = launch.mint)]
    pub mint: Box<Account<'info, Mint>>,
    #[account(mut, associated_token::mint = mint, associated_token::authority = launch)]
    pub token_vault: Box<Account<'info, TokenAccount>>,
    #[account(mut, seeds = [SOL_VAULT_SEED, launch.key().as_ref()], bump = launch.sol_vault_bump)]
    pub sol_vault: Box<Account<'info, Vault>>,
    #[account(mut, seeds = [PROTOCOL_SEED], bump = protocol.bump)]
    pub protocol: Box<Account<'info, Vault>>,
    /// CHECK: owner, layout and pool creator checked by `damm::read_config`.
    #[account(address = config.dex_config @ HerdError::InvalidDexConfig)]
    pub dex_config: UncheckedAccount<'info>,
    /// CHECK: system-owned PDA without data; it signs as the pool's payer.
    #[account(mut, seeds = [MIGRATOR_SEED], bump)]
    pub migrator: UncheckedAccount<'info>,
    #[account(init, payer = keeper, seeds = [MIGRATOR_TOKEN_SEED, launch.key().as_ref()], bump, token::mint = mint, token::authority = migrator)]
    pub migrator_token: Box<Account<'info, TokenAccount>>,
    #[account(init, payer = keeper, seeds = [MIGRATOR_WSOL_SEED, launch.key().as_ref()], bump, token::mint = native_mint, token::authority = migrator)]
    pub migrator_wsol: Box<Account<'info, TokenAccount>>,
    #[account(address = native_mint::ID)]
    pub native_mint: Box<Account<'info, Mint>>,
    /// CHECK: the position NFT mint DAMM creates; this PDA signs for it.
    #[account(mut, seeds = [POSITION_NFT_SEED, launch.key().as_ref()], bump)]
    pub position_nft_mint: UncheckedAccount<'info>,
    /// CHECK: created and checked by DAMM (seeds from the NFT mint).
    #[account(mut)]
    pub position_nft_account: UncheckedAccount<'info>,
    /// CHECK: created and checked by DAMM (seeds from config and mints).
    #[account(mut)]
    pub pool: UncheckedAccount<'info>,
    /// CHECK: created and checked by DAMM.
    #[account(mut)]
    pub position: UncheckedAccount<'info>,
    /// CHECK: created and checked by DAMM.
    #[account(mut)]
    pub pool_token_vault: UncheckedAccount<'info>,
    /// CHECK: created and checked by DAMM.
    #[account(mut)]
    pub pool_wsol_vault: UncheckedAccount<'info>,
    /// CHECK: DAMM's pool authority.
    #[account(address = damm::POOL_AUTHORITY)]
    pub pool_authority: UncheckedAccount<'info>,
    /// CHECK: checked by DAMM (event CPI).
    pub dex_event_authority: UncheckedAccount<'info>,
    /// CHECK: the real DAMM v2 program; nothing else may receive the reserve.
    #[account(address = damm::PROGRAM_ID, executable)]
    pub dex_program: UncheckedAccount<'info>,
    pub token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ClaimLpFees<'info> {
    #[account(mut)]
    pub caller: Signer<'info>,
    #[account(mut, seeds = [LAUNCH_SEED, launch.mint.as_ref()], bump = launch.bump)]
    pub launch: Box<Account<'info, Launch>>,
    #[account(address = launch.mint)]
    pub mint: Box<Account<'info, Mint>>,
    #[account(address = native_mint::ID)]
    pub native_mint: Box<Account<'info, Mint>>,
    #[account(mut, associated_token::mint = mint, associated_token::authority = launch)]
    pub token_vault: Box<Account<'info, TokenAccount>>,
    #[account(mut, seeds = [SOL_VAULT_SEED, launch.key().as_ref()], bump = launch.sol_vault_bump)]
    pub sol_vault: Box<Account<'info, Vault>>,
    #[account(mut, seeds = [CREATOR_FEES_SEED, launch.key().as_ref()], bump = launch.creator_fees_bump)]
    pub creator_fees: Box<Account<'info, Vault>>,
    #[account(mut, seeds = [REKT_BACK_SEED], bump = rekt_back.bump)]
    pub rekt_back: Box<Account<'info, Vault>>,
    #[account(mut, seeds = [PROTOCOL_SEED], bump = protocol.bump)]
    pub protocol: Box<Account<'info, Vault>>,
    #[account(init, payer = caller, seeds = [LP_FEE_WSOL_SEED, launch.key().as_ref()], bump, token::mint = native_mint, token::authority = launch)]
    pub fee_wsol: Box<Account<'info, TokenAccount>>,
    /// CHECK: the launch's pool, recorded at breakout; DAMM checks the rest.
    #[account(address = launch.dex_pool @ HerdError::WrongAccount)]
    pub pool: UncheckedAccount<'info>,
    /// CHECK: the launch's position, recorded at breakout.
    #[account(mut, address = launch.dex_position @ HerdError::WrongAccount)]
    pub position: UncheckedAccount<'info>,
    /// CHECK: checked by DAMM (holds the position NFT, owned by the launch PDA).
    pub position_nft_account: UncheckedAccount<'info>,
    /// CHECK: checked by DAMM (`has_one` on the pool).
    #[account(mut)]
    pub pool_token_vault: UncheckedAccount<'info>,
    /// CHECK: checked by DAMM (`has_one` on the pool).
    #[account(mut)]
    pub pool_wsol_vault: UncheckedAccount<'info>,
    /// CHECK: DAMM's pool authority.
    #[account(address = damm::POOL_AUTHORITY)]
    pub pool_authority: UncheckedAccount<'info>,
    /// CHECK: checked by DAMM (event CPI).
    pub dex_event_authority: UncheckedAccount<'info>,
    /// CHECK: the real DAMM v2 program.
    #[account(address = damm::PROGRAM_ID, executable)]
    pub dex_program: UncheckedAccount<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ReinvestLp<'info> {
    #[account(mut)]
    pub caller: Signer<'info>,
    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Box<Account<'info, Config>>,
    #[account(mut, seeds = [LAUNCH_SEED, launch.mint.as_ref()], bump = launch.bump)]
    pub launch: Box<Account<'info, Launch>>,
    #[account(mut, address = launch.mint)]
    pub mint: Box<Account<'info, Mint>>,
    #[account(address = native_mint::ID)]
    pub native_mint: Box<Account<'info, Mint>>,
    #[account(mut, seeds = [SOL_VAULT_SEED, launch.key().as_ref()], bump = launch.sol_vault_bump)]
    pub sol_vault: Box<Account<'info, Vault>>,
    /// CHECK: system-owned PDA without data, empty before and after.
    #[account(mut, seeds = [MIGRATOR_SEED], bump)]
    pub migrator: UncheckedAccount<'info>,
    #[account(init, payer = caller, seeds = [REINVEST_WSOL_SEED, launch.key().as_ref()], bump, token::mint = native_mint, token::authority = launch)]
    pub reinvest_wsol: Box<Account<'info, TokenAccount>>,
    #[account(init, payer = caller, seeds = [REINVEST_TOKEN_SEED, launch.key().as_ref()], bump, token::mint = mint, token::authority = launch)]
    pub reinvest_token: Box<Account<'info, TokenAccount>>,
    /// CHECK: the launch's pool, recorded at breakout; owner and layout checked by `damm::read_pool`.
    #[account(mut, address = launch.dex_pool @ HerdError::WrongAccount)]
    pub pool: UncheckedAccount<'info>,
    /// CHECK: the launch's position, recorded at breakout.
    #[account(mut, address = launch.dex_position @ HerdError::WrongAccount)]
    pub position: UncheckedAccount<'info>,
    /// CHECK: checked by DAMM (holds the position NFT, owned by the launch PDA).
    pub position_nft_account: UncheckedAccount<'info>,
    /// CHECK: checked by DAMM (`has_one` on the pool).
    #[account(mut)]
    pub pool_token_vault: UncheckedAccount<'info>,
    /// CHECK: checked by DAMM (`has_one` on the pool).
    #[account(mut)]
    pub pool_wsol_vault: UncheckedAccount<'info>,
    /// CHECK: DAMM's pool authority.
    #[account(address = damm::POOL_AUTHORITY)]
    pub pool_authority: UncheckedAccount<'info>,
    /// CHECK: checked by DAMM (event CPI).
    pub dex_event_authority: UncheckedAccount<'info>,
    /// CHECK: the real DAMM v2 program.
    #[account(address = damm::PROGRAM_ID, executable)]
    pub dex_program: UncheckedAccount<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
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
pub struct LpFeesClaimed {
    pub launch: Pubkey,
    pub fee: u64,
    pub liquidity: u64,
    pub creator: u64,
    pub rekt_back: u64,
    pub protocol: u64,
    /// Part of the creator share lost to a lower lock ratio (already counted in `rekt_back`).
    pub forfeited: u64,
}

#[event]
pub struct LpReinvested {
    pub launch: Pubkey,
    /// SOL taken from the pending share: the part swapped plus the part added.
    pub sol: u64,
    pub swapped: u64,
    pub tokens: u64,
    pub liquidity: u128,
    /// Still waiting after this call.
    pub pending: u64,
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
    pub pool: Pubkey,
    pub lp_native: u64,
    pub lp_tokens: u64,
    pub burned: u64,
}
