//! Her kritik adim bir olay yayinlar: indexer hem seffaflik panelini
//! hem (gizli) XP hesabini bu olaylardan besler. XP kontrata yazilmaz.
use anchor_lang::prelude::*;

#[event]
pub struct LaunchCreated {
    pub launch: Pubkey,
    pub mint: Pubkey,
    pub creator: Pubkey,
    pub bonded: bool,
    pub lobby_end: i64,
}

#[event]
pub struct LobbyDeposited {
    pub launch: Pubkey,
    pub owner: Pubkey,
    pub amount: u64,
    pub order_total: u64,
    pub launch_total: u64,
    pub first_deposit: bool,
}

#[event]
pub struct StampedeSettled {
    pub launch: Pubkey,
    pub success: bool,
    pub total_deposited: u64,
    pub accepted_total: u64,
    pub stampede_tokens: u64,
    pub depositors: u32,
}

#[event]
pub struct StampedeClaimed {
    pub launch: Pubkey,
    pub owner: Pubkey,
    pub accepted_lamports: u64,
    pub refund_lamports: u64,
    pub fee_lamports: u64,
    pub tokens: u64,
}

#[event]
pub struct BreakoutDone {
    pub launch: Pubkey,
    pub pool: Pubkey,
    pub position: Pubkey,
    pub sol_in_pool: u64,
    pub tokens_in_pool: u64,
    pub locked_liquidity: u128,
    pub leftover_to_protocol: u64,
}

#[event]
pub struct DaoWithdrawn {
    pub authority: Pubkey,
    pub destination: Pubkey,
    pub amount: u64,
}

#[event]
pub struct ConfigUpdated {
    pub authority: Pubkey,
}

#[event]
pub struct AuthorityProposed {
    pub current: Pubkey,
    pub pending: Pubkey,
}

#[event]
pub struct AuthorityAccepted {
    pub new_authority: Pubkey,
}

#[event]
pub struct Traded {
    pub launch: Pubkey,
    pub trader: Pubkey,
    pub is_buy: bool,
    /// Brut SOL (alista odenen, satista curve'den cikan)
    pub sol_gross: u64,
    pub tokens: u64,
    pub fee_creator: u64,
    pub fee_ui: u64,
    pub fee_rekt: u64,
    pub fee_protocol: u64,
    pub virtual_sol: u64,
    pub virtual_token: u64,
    pub curve_full: bool,
}

#[event]
pub struct PositionFeeClaimed {
    pub launch: Pubkey,
    pub caller: Pubkey,
    /// DAMM'den toplanan brut ucret (token = launch token'i, sol = wSOL)
    pub claimed_token: u64,
    pub claimed_sol: u64,
    /// Creator geliri alabilir miydi (taahhut tamam + hak kaybedilmemis)
    pub creator_eligible: bool,
    /// Creator SOL payi: cuzdana giden / rent altinda kaldigi icin protokole yonlenen /
    /// buyback-burn rezervine giden (gonullu yakim veya hak kaybi)
    pub creator_sol_paid: u64,
    pub creator_sol_redirected: u64,
    pub creator_sol_to_burn_reserve: u64,
    /// Creator token payi: cuzdana giden / yakilan
    pub creator_token_paid: u64,
    pub creator_token_burned: u64,
    pub protocol_sol: u64,
    pub dao_sol: u64,
    /// Protokol ve DAO'nun token payi: kendi havuzlarinin ATA'larinda birikir
    pub protocol_token: u64,
    pub dao_token: u64,
}

#[event]
pub struct FeePrefsUpdated {
    pub launch: Pubkey,
    pub creator: Pubkey,
    pub payout_wallet: Pubkey,
    pub burn_bps: u16,
}

#[event]
pub struct CreatorCommitted {
    pub launch: Pubkey,
    pub creator: Pubkey,
    pub amount: u64,
    pub total_committed: u64,
    pub eligible: bool,
}

#[event]
pub struct CreatorCommitWithdrawn {
    pub launch: Pubkey,
    pub creator: Pubkey,
    pub amount: u64,
    pub total_committed: u64,
    /// Bu cekimle hak KALICI olarak kaybedildiyse true
    pub forfeited_now: bool,
}

#[event]
pub struct BuybackBurned {
    pub launch: Pubkey,
    pub caller: Pubkey,
    pub sol_spent: u64,
    pub tokens_burned: u64,
    pub reserve_left: u64,
}

#[event]
pub struct PoolTokensWithdrawn {
    pub authority: Pubkey,
    pub pool: Pubkey,
    pub destination: Pubkey,
    pub amount: u64,
}

#[event]
pub struct ProtocolDistributed {
    pub authority: Pubkey,
    pub amount: u64,
    pub to_herd_vault: u64,
    pub to_builders: u64,
    pub to_ops: u64,
    pub pool_total_distributed: u64,
}

#[event]
pub struct WalletsUpdated {
    pub authority: Pubkey,
    pub treasury: Pubkey,
    pub builders_fund: Pubkey,
    pub ops_wallet: Pubkey,
}
