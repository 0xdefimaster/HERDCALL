use anchor_lang::prelude::*;
use herdcall_core::curve::{CurveState, MarketParams, Phase as CorePhase};
use herdcall_core::fees::Kind;
use herdcall_core::names::LaunchStatus;
use herdcall_core::stampede::LobbyParams;

pub const CONFIG_SEED: &[u8] = b"config";
pub const PROTOCOL_SEED: &[u8] = b"protocol";
pub const REKT_BACK_SEED: &[u8] = b"rekt_back";
pub const LAUNCH_SEED: &[u8] = b"launch";
pub const SOL_VAULT_SEED: &[u8] = b"sol_vault";
pub const CREATOR_FEES_SEED: &[u8] = b"creator_fees";
pub const CREATOR_ESCROW_SEED: &[u8] = b"creator_escrow";
pub const DEPOSIT_SEED: &[u8] = b"deposit";
pub const INTERFACE_SEED: &[u8] = b"interface";
pub const NAME_SEED: &[u8] = b"name";
pub const TICKER_SEED: &[u8] = b"ticker";

pub const TOKEN_DECIMALS: u8 = 6;
pub const MAX_LOBBY_SECS: i64 = 3_600;

/// Parameters every new launch copies. Changing them never affects an existing launch.
#[account]
#[derive(InitSpace)]
pub struct Config {
    pub admin: Pubkey,
    pub params: LaunchParams,
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
    pub min_total: u64,
    pub max_total: u64,
    /// 0 = no per-wallet cap.
    pub max_per_wallet: u64,
    pub migration_fee: u64,
    pub settle_window: i64,
    /// Buys stay open this long after the Lobby closes. A token that has not broken out by then only takes sells,
    /// can never break out, and its name and ticker can be released.
    pub curve_lifetime_secs: i64,
    /// Refundable reservation bond (lamports) for the name and ticker; back to the creator at breakout or release.
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
    /// Breakout done: the keeper is paid, extra LP tokens are burned, `lp_native` and `lp_tokens` are set aside for
    /// the Meteora DAMM v2 pool. The pool creation is the next step of the program (CURVE_SPEC §5.4).
    Migrating,
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
    /// As given by the creator; uniqueness is checked on its canonical form (`herdcall_core::names::canonical_name`).
    #[max_len(32)]
    pub name: String,
    pub close_at: i64,
    pub settle_deadline: i64,
    /// No buys from this time on unless the token has already reached breakout.
    pub curve_deadline: i64,
    /// Reservation bond still held in the ticker record (0 once it is paid back).
    pub name_bond: u64,
    // Lobby
    pub total_deposits: u64,
    pub depositors: u32,
    pub settled_empty: bool,
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
    pub lp_native: u64,
    pub lp_tokens: u64,
    pub burned: u64,
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
            Phase::Lobby if now >= self.settle_deadline => LaunchStatus::LobbyExpired,
            Phase::Lobby => LaunchStatus::Lobby,
            Phase::Cancelled => LaunchStatus::LobbyExpired,
            Phase::Curve => LaunchStatus::Curve,
            Phase::BreakingOut | Phase::Migrating | Phase::Migrated => LaunchStatus::BrokeOut,
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
            CorePhase::Migrated => Phase::Migrating,
        };
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

/// Reservation of one canonical name or ticker. Held by `launch` while it can still break out, permanent once it does;
/// otherwise anyone can close it when the launch can no longer break out (rent and bond go back to the creator).
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
