use anchor_lang::prelude::*;
use herdcall_core::curve::{CurveState, MarketParams, Phase as CorePhase};
use herdcall_core::fees::Kind;
use herdcall_core::names::LaunchStatus;
use herdcall_core::stampede::LobbyParams;

pub const CONFIG_SEED: &[u8] = b"config";
pub const PROTOCOL_SEED: &[u8] = b"protocol";
pub const REVENUE_SEED: &[u8] = b"revenue";
pub const REKT_BACK_SEED: &[u8] = b"rekt_back";
pub const LAUNCH_SEED: &[u8] = b"launch";
pub const SOL_VAULT_SEED: &[u8] = b"sol_vault";
pub const CREATOR_FEES_SEED: &[u8] = b"creator_fees";
pub const CREATOR_ESCROW_SEED: &[u8] = b"creator_escrow";
pub const DEPOSIT_SEED: &[u8] = b"deposit";
pub const INTERFACE_SEED: &[u8] = b"interface";
pub const NAME_SEED: &[u8] = b"name";
pub const TICKER_SEED: &[u8] = b"ticker";
/// System-owned PDA, the only pool creator allowed by the HerdCall DAMM v2 config. It pays the pool's rent and holds the
/// LP tokens and SOL for the length of a breakout; it is empty before and after.
pub const MIGRATOR_SEED: &[u8] = b"migrator";
pub const MIGRATOR_TOKEN_SEED: &[u8] = b"migrator_token";
pub const MIGRATOR_WSOL_SEED: &[u8] = b"migrator_wsol";
pub const POSITION_NFT_SEED: &[u8] = b"position_nft";
/// Temporary wrapped SOL account the locked position's fees are claimed into.
pub const LP_FEE_WSOL_SEED: &[u8] = b"lp_fee_wsol";
/// Temporary accounts of a reinvest: the SOL being reinvested and the tokens it buys.
pub const REINVEST_WSOL_SEED: &[u8] = b"reinvest_wsol";
pub const REINVEST_TOKEN_SEED: &[u8] = b"reinvest_token";

pub const TOKEN_DECIMALS: u8 = 6;
/// Metaplex limit for the metadata URI (JSON with the image and description).
pub const MAX_URI_LEN: usize = 200;
pub const MAX_LOBBY_SECS: i64 = 3_600;
/// Lamports the breakout caller fronts to the migrator so the pool's rent is covered whatever it costs (about
/// 0.021 SOL on a real cluster). Returned in the same instruction, with the migration fee left after rent.
pub const BREAKOUT_FLOAT: u64 = 50_000_000;

/// Parameters every new launch copies. Changing them never affects an existing launch.
#[account]
#[derive(InitSpace)]
pub struct Config {
    pub admin: Pubkey,
    pub params: LaunchParams,
    /// Meteora DAMM v2 config breakouts create their pool with (checked at breakout, see `damm::read_config`).
    pub dex_config: Pubkey,
    /// The only caller of `reinvest_lp`. If it stops, the share waits as SOL and the admin appoints another keeper.
    pub reinvest_keeper: Pubkey,
    pub bump: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq, InitSpace)]
pub struct LaunchParams {
    pub v0: u64,
    pub t0: u64,
    pub total_supply: u64,
    pub curve_supply: u64,
    pub lp_supply: u64,
    pub fee_bps: u16,
    pub min_deposit: u64,
    /// The curve opens only if the Lobby reaches this; below it the launch is cancelled, everyone is refunded and the
    /// name and ticker are released. This is what keeps a name from being held with an empty Lobby.
    pub min_total: u64,
    pub max_total: u64,
    /// 0 = no per-wallet cap.
    pub max_per_wallet: u64,
    pub migration_fee: u64,
    pub settle_window: i64,
    /// Refundable reservation bond (lamports) for the name and ticker, held during the Lobby only: back to the creator
    /// when the curve opens, or when a cancelled launch's name is released.
    pub name_bond: u64,
}

impl LaunchParams {
    pub fn lobby(&self) -> LobbyParams {
        LobbyParams {
            v0: self.v0 as u128,
            t0: self.t0 as u128,
            curve_supply: self.curve_supply as u128,
            fee_bps: self.fee_bps as u128,
            min_deposit: self.min_deposit as u128,
            min_total: self.min_total as u128,
        }
    }

    pub fn market(&self, kind: Kind) -> MarketParams {
        MarketParams {
            v0: self.v0 as u128,
            t0: self.t0 as u128,
            total_supply: self.total_supply as u128,
            curve_supply: self.curve_supply as u128,
            lp_supply: self.lp_supply as u128,
            fee_bps: self.fee_bps as u128,
            migration_fee: self.migration_fee as u128,
            kind,
        }
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq, InitSpace)]
pub enum Phase {
    Lobby,
    Curve,
    BreakingOut,
    /// Broke out: the Meteora DAMM v2 pool holds the liquidity, locked for good (CURVE_SPEC §5.4).
    Migrated,
    Cancelled,
}

#[account]
#[derive(InitSpace)]
pub struct Launch {
    pub creator: Pubkey,
    pub mint: Pubkey,
    pub bonded: bool,
    pub phase: Phase,
    pub params: LaunchParams,
    #[max_len(10)]
    pub ticker: String,
    /// As given by the creator, without leading and trailing whitespace; uniqueness is checked on its canonical form
    /// (`herdcall_core::names::canonical_name`).
    #[max_len(32)]
    pub name: String,
    pub close_at: i64,
    pub settle_deadline: i64,
    /// Reservation bond still held in the ticker record (0 once it is paid back at settlement or release).
    pub name_bond: u64,
    // Lobby
    pub total_deposits: u64,
    pub depositors: u32,
    pub lobby_tokens: u64,
    pub lobby_tokens_claimed: u64,
    // Curve (CURVE_SPEC §2)
    pub v: u64,
    pub t: u64,
    pub k: u128,
    pub reserve: u64,
    pub sold: u64,
    /// Creator lock ratio ("hold to earn"); 10_000 until the Bonded Creator vault is wired in.
    pub lock_ratio_bps: u16,
    // Breakout
    /// SOL and tokens in the pool, and its locked liquidity.
    pub lp_native: u64,
    pub lp_tokens: u64,
    pub lp_liquidity: u128,
    /// LP tokens the pool did not need, burned.
    pub burned: u64,
    pub dex_pool: Pubkey,
    pub dex_position: Pubkey,
    /// SOL fees claimed from the locked position so far (CURVE_SPEC §5.7).
    pub lp_fees_claimed: u64,
    /// The "back to liquidity" share of those fees, held in the SOL vault until it is added to the position.
    pub lp_reinvest_pending: u64,
    /// SOL of that share already added to the position.
    pub lp_reinvested: u64,
    pub bump: u8,
    pub sol_vault_bump: u8,
    pub creator_fees_bump: u8,
    pub creator_escrow_bump: u8,
}

impl Launch {
    pub fn kind(&self) -> Kind {
        if self.bonded {
            Kind::Bonded
        } else {
            Kind::Unbonded
        }
    }

    pub fn name_status(&self, now: i64) -> LaunchStatus {
        match self.phase {
            Phase::Lobby if now >= self.settle_deadline => LaunchStatus::Cancelled,
            Phase::Lobby => LaunchStatus::Lobby,
            Phase::Cancelled => LaunchStatus::Cancelled,
            Phase::Curve => LaunchStatus::Curve,
            Phase::BreakingOut | Phase::Migrated => LaunchStatus::BrokeOut,
        }
    }

    pub fn curve(&self) -> CurveState {
        CurveState { v: self.v as u128, t: self.t as u128, k: self.k, reserve: self.reserve as u128, sold: self.sold as u128 }
    }

    pub fn store_curve(&mut self, c: &CurveState) -> Result<()> {
        self.v = to_u64(c.v)?;
        self.t = to_u64(c.t)?;
        self.k = c.k;
        self.reserve = to_u64(c.reserve)?;
        self.sold = to_u64(c.sold)?;
        Ok(())
    }

    pub fn store_phase(&mut self, p: CorePhase) {
        self.phase = match p {
            CorePhase::Curve => Phase::Curve,
            CorePhase::BreakingOut => Phase::BreakingOut,
            CorePhase::Migrated => Phase::Migrated,
        };
    }
}

/// Running totals of what the protocol vault received, by source (HERDCALL.md §7, REKT_MINING.md §7.1). Revenue is
/// only the protocol's nominal share of curve, Stampede and LP fees; split remainders and other inflows are kept apart
/// and are not revenue. Each lamport is recorded once, where it enters the vault. Totals never decrease: a weekly
/// distribution will take their growth since the previous one.
#[account]
#[derive(InitSpace)]
pub struct RevenueLedger {
    pub curve: u64,
    pub stampede: u64,
    pub lp: u64,
    /// Rounding remainders of fee splits that went to the protocol vault.
    pub rounding: u64,
    /// Other inflows: the few lamports a breakout pool could not take.
    pub other: u64,
    pub bump: u8,
}

#[derive(Clone, Copy)]
pub enum RevenueSource {
    Curve,
    Stampede,
    Lp,
}

impl RevenueLedger {
    /// `received`: lamports the protocol vault got from one fee split; `revenue`: the nominal share in it.
    pub fn record(&mut self, source: RevenueSource, received: u128, revenue: u128) -> Result<()> {
        let overflow = || error!(crate::errors::HerdError::MathOverflow);
        let rounding = received.checked_sub(revenue).ok_or_else(overflow)?;
        let revenue = to_u64(revenue)?;
        let counter = match source {
            RevenueSource::Curve => &mut self.curve,
            RevenueSource::Stampede => &mut self.stampede,
            RevenueSource::Lp => &mut self.lp,
        };
        *counter = counter.checked_add(revenue).ok_or_else(overflow)?;
        self.rounding = self.rounding.checked_add(to_u64(rounding)?).ok_or_else(overflow)?;
        Ok(())
    }

    pub fn record_other(&mut self, lamports: u64) -> Result<()> {
        self.other = self.other.checked_add(lamports).ok_or(crate::errors::HerdError::MathOverflow)?;
        Ok(())
    }

    /// Revenue so far, all sources.
    pub fn revenue(&self) -> Option<u64> {
        self.curve.checked_add(self.stampede)?.checked_add(self.lp)
    }
}

/// Program-owned account that only holds lamports.
#[account]
#[derive(InitSpace)]
pub struct Vault {
    pub bump: u8,
}

/// Lobby deposit of one wallet. Deposits of the same wallet add up.
#[account]
#[derive(InitSpace)]
pub struct Deposit {
    pub launch: Pubkey,
    pub owner: Pubkey,
    pub amount: u64,
    pub withdrawn: bool,
    pub bump: u8,
}

/// Reservation of one canonical name or ticker. Held by `launch` from its creation, permanent once its curve opens;
/// anyone can close it when the launch is cancelled (rent and bond go back to the creator).
#[account]
#[derive(InitSpace)]
pub struct NameRecord {
    pub launch: Pubkey,
    pub permanent: bool,
    pub bump: u8,
}

/// A trading interface (terminal, bot, referral) that receives the interface share of the fee.
#[account]
#[derive(InitSpace)]
pub struct TradingInterface {
    pub owner: Pubkey,
    pub bump: u8,
}

/// PDA seed of a name: sha256 of its canonical form. `None` if the name is not valid.
pub fn name_seed(name: &str) -> Option<[u8; 32]> {
    herdcall_core::names::canonical_name(name).map(|c| solana_sha256_hasher::hash(c.as_bytes()).to_bytes())
}

pub fn to_u64(x: u128) -> Result<u64> {
    u64::try_from(x).map_err(|_| error!(crate::errors::HerdError::MathOverflow))
}
