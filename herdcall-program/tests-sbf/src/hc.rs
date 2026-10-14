//! HerdCall instruction builders and account mirrors for SBF tests.
//!
//! The program crate (anchor 0.30 / solana 1.18) cannot share a dependency graph with LiteSVM
//! (agave 3.x), so instructions are encoded by hand: Anchor discriminator
//! `sha256("global:<name>")[..8]` + borsh args, account metas in the exact order of the
//! `#[derive(Accounts)]` structs. Account data is decoded through borsh mirrors whose field order
//! follows `programs/herdcall/src/state/*.rs`; `launch_roundtrip_matches_space` guards the layout.

use borsh::{BorshDeserialize, BorshSerialize};
use sha2::{Digest, Sha256};
use solana_address::Address;
use solana_instruction::{AccountMeta, Instruction};

use crate::env::HERDCALL_ID;

pub const SYSTEM: Address = Address::from_str_const("11111111111111111111111111111111");
pub const TOKEN: Address = Address::from_str_const("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
pub const TOKEN_2022: Address = Address::from_str_const("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb");
pub const ATA_PROGRAM: Address = Address::from_str_const("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");
pub const WSOL: Address = Address::from_str_const("So11111111111111111111111111111111111111112");
pub const BPF_UPGRADEABLE: Address = Address::from_str_const("BPFLoaderUpgradeab1e11111111111111111111111");

pub const SOL: u64 = 1_000_000_000;
pub const TOTAL_SUPPLY: u64 = 1_000_000_000 * 1_000_000;
pub const LP_TOKENS: u64 = TOTAL_SUPPLY / 5; // LP_RESERVE_BPS = 2000

pub fn disc(name: &str) -> [u8; 8] {
    Sha256::digest(format!("global:{name}").as_bytes())[..8].try_into().unwrap()
}

pub fn pda(seeds: &[&[u8]]) -> Address {
    Address::find_program_address(seeds, &HERDCALL_ID).0
}
pub fn config_pda() -> Address { pda(&[b"config"]) }
pub fn rekt_pda() -> Address { pda(&[b"rekt_pool"]) }
pub fn vault_pda() -> Address { pda(&[b"herd_vault"]) }
pub fn protocol_pda() -> Address { pda(&[b"protocol_pool"]) }
pub fn dao_pda() -> Address { pda(&[b"dao_pool"]) }
pub fn launch_pda(mint: &Address) -> Address { pda(&[b"launch", mint.as_ref()]) }
pub fn order_pda(launch: &Address, owner: &Address) -> Address {
    pda(&[b"order", launch.as_ref(), owner.as_ref()])
}
pub fn breakout_auth_pda(launch: &Address) -> Address { pda(&[b"breakout_auth", launch.as_ref()]) }
/// Global DAMM pool creator (C5 mitigation). Must equal the private config's pool_creator_authority.
pub fn damm_pool_creator_pda() -> Address { pda(&[b"damm_pool_creator"]) }
pub fn program_data_pda() -> Address {
    Address::find_program_address(&[HERDCALL_ID.as_ref()], &BPF_UPGRADEABLE).0
}
pub fn ata(owner: &Address, mint: &Address) -> Address {
    spl_associated_token_account_interface::address::get_associated_token_address(owner, mint)
}

fn w(k: Address) -> AccountMeta { AccountMeta::new(k, false) }
fn ws(k: Address) -> AccountMeta { AccountMeta::new(k, true) }
fn r(k: Address) -> AccountMeta { AccountMeta::new_readonly(k, false) }
fn rs(k: Address) -> AccountMeta { AccountMeta::new_readonly(k, true) }

fn ix(name: &str, args: &impl BorshSerialize, accounts: Vec<AccountMeta>) -> Instruction {
    let mut data = disc(name).to_vec();
    args.serialize(&mut data).unwrap();
    Instruction { program_id: HERDCALL_ID, accounts, data }
}

#[derive(BorshSerialize, Clone)]
pub struct ParamsArgs {
    pub rekt_select_bps: u16,
    pub rekt_payout_cap_bps: u16,
    pub rekt_reserve_floor: u64,
    pub min_stampede_lamports: u64,
    pub stampede_cap_lamports: u64,
    pub claim_fee_bps: u16,
    pub damm_config: [u8; 32],
    pub breakout_cost_lamports: u64,
    pub min_creator_commit_bps: u16,
    pub paused: bool,
}

pub fn default_params(damm_config: Address) -> ParamsArgs {
    ParamsArgs {
        rekt_select_bps: 385,
        rekt_payout_cap_bps: 5_000,
        rekt_reserve_floor: 0,
        min_stampede_lamports: SOL,
        stampede_cap_lamports: 2 * SOL,
        claim_fee_bps: 100,
        damm_config: damm_config.to_bytes(),
        // budget for DAMM rents (pool, position, vaults, NFT): 0.2 SOL upper bound in Config
        breakout_cost_lamports: 100_000_000,
        // creator never commits in these tests => creator fee share accrues to burn_reserve
        min_creator_commit_bps: 100,
        paused: false,
    }
}

#[derive(BorshSerialize)]
struct InitArgs {
    treasury: [u8; 32],
    builders_fund: [u8; 32],
    ops_wallet: [u8; 32],
    params: ParamsArgs,
}

pub fn ix_initialize_config(authority: &Address, wallets: [Address; 3], params: ParamsArgs) -> Instruction {
    ix(
        "initialize_config",
        &InitArgs {
            treasury: wallets[0].to_bytes(),
            builders_fund: wallets[1].to_bytes(),
            ops_wallet: wallets[2].to_bytes(),
            params,
        },
        vec![
            ws(*authority),
            r(program_data_pda()),
            w(config_pda()),
            w(rekt_pda()),
            w(vault_pda()),
            w(protocol_pda()),
            w(dao_pda()),
            r(SYSTEM),
        ],
    )
}

pub fn ix_create_launch(creator: &Address, mint: &Address) -> Instruction {
    let launch = launch_pda(mint);
    ix(
        "create_launch",
        &(15i64, false),
        vec![
            ws(*creator),
            r(config_pda()),
            ws(*mint),
            w(launch),
            w(ata(&launch, mint)),
            r(TOKEN),
            r(ATA_PROGRAM),
            r(SYSTEM),
        ],
    )
}

pub fn ix_lobby_deposit(owner: &Address, mint: &Address, amount: u64) -> Instruction {
    let launch = launch_pda(mint);
    ix(
        "lobby_deposit",
        &amount,
        vec![ws(*owner), r(config_pda()), w(launch), w(order_pda(&launch, owner)), r(SYSTEM)],
    )
}

pub fn ix_settle(caller: &Address, mint: &Address) -> Instruction {
    let launch = launch_pda(mint);
    ix(
        "settle_stampede",
        &(),
        vec![rs(*caller), w(launch), w(*mint), w(ata(&launch, mint)), r(TOKEN)],
    )
}

pub fn ix_claim_stampede(owner: &Address, mint: &Address) -> Instruction {
    let launch = launch_pda(mint);
    ix(
        "claim_stampede",
        &(),
        vec![
            ws(*owner),
            w(launch),
            w(*mint),
            w(order_pda(&launch, owner)),
            w(ata(owner, mint)),
            w(ata(&launch, mint)),
            w(protocol_pda()),
            r(TOKEN),
            r(ATA_PROGRAM),
            r(SYSTEM),
        ],
    )
}

pub fn ix_buy(trader: &Address, mint: &Address, creator: &Address, ui: &Address, gross_in: u64) -> Instruction {
    let launch = launch_pda(mint);
    ix(
        "buy",
        &(gross_in, 0u64),
        vec![
            ws(*trader),
            r(config_pda()),
            w(launch),
            w(*mint),
            w(ata(trader, mint)),
            w(*creator),
            w(*ui),
            w(rekt_pda()),
            w(protocol_pda()),
            r(TOKEN),
            r(ATA_PROGRAM),
            r(SYSTEM),
        ],
    )
}

/// DAMM accounts a breakout needs (derived by `damm::breakout_accounts`).
#[derive(Clone, Copy, Debug)]
pub struct DammBreakoutAccounts {
    pub config: Address,
    pub position_nft_mint: Address,
    pub position_nft_account: Address,
    pub pool: Address,
    pub position: Address,
    pub token_a_vault: Address,
    pub token_b_vault: Address,
}

/// Account order of `BreakoutLaunch` (v8 layout: breakout_authority is DAMM payer + creator).
pub fn ix_breakout(caller: &Address, mint: &Address, d: &DammBreakoutAccounts, liquidity: u128, sqrt_price: u128) -> Instruction {
    let launch = launch_pda(mint);
    let auth = breakout_auth_pda(&launch);
    ix(
        "breakout",
        &(liquidity, sqrt_price),
        vec![
            ws(*caller),
            r(config_pda()),
            w(launch),
            w(*mint),
            w(protocol_pda()),
            w(auth),
            r(WSOL),
            w(ata(&auth, mint)),
            w(ata(&auth, &WSOL)),
            r(d.config),
            r(crate::damm::DAMM_ID),
            r(crate::damm::POOL_AUTHORITY),
            r(crate::damm::event_authority()),
            ws(d.position_nft_mint),
            w(d.position_nft_account),
            w(d.pool),
            w(d.position),
            w(d.token_a_vault),
            w(d.token_b_vault),
            r(TOKEN),
            r(TOKEN_2022),
            r(ATA_PROGRAM),
            r(SYSTEM),
        ],
    )
}

pub fn ix_buyback_burn(caller: &Address, mint: &Address, pool: &Address, a_vault: &Address, b_vault: &Address, amount: u64) -> Instruction {
    let launch = launch_pda(mint);
    let auth = breakout_auth_pda(&launch);
    ix(
        "buyback_burn",
        &amount,
        vec![
            ws(*caller),
            w(launch),
            w(*mint),
            r(auth),
            r(WSOL),
            w(ata(&auth, mint)),
            w(ata(&auth, &WSOL)),
            w(*pool),
            w(*a_vault),
            w(*b_vault),
            r(crate::damm::DAMM_ID),
            r(crate::damm::POOL_AUTHORITY),
            r(crate::damm::event_authority()),
            r(TOKEN),
            r(ATA_PROGRAM),
            r(SYSTEM),
        ],
    )
}

pub fn ix_claim_position_fee(
    caller: &Address,
    mint: &Address,
    payout_wallet: &Address,
    pool: &Address,
    position: &Address,
    a_vault: &Address,
    b_vault: &Address,
    nft_mint: &Address,
) -> Instruction {
    let launch = launch_pda(mint);
    let auth = breakout_auth_pda(&launch);
    ix(
        "claim_position_fee",
        &(),
        vec![
            ws(*caller),
            w(launch),
            w(*mint),
            w(protocol_pda()),
            w(dao_pda()),
            r(auth),
            r(WSOL),
            w(ata(&auth, mint)),
            w(ata(&auth, &WSOL)),
            w(*payout_wallet),
            w(ata(payout_wallet, mint)),
            w(ata(&protocol_pda(), mint)),
            w(ata(&dao_pda(), mint)),
            r(*pool),
            w(*position),
            w(*a_vault),
            w(*b_vault),
            r(crate::damm::position_nft_account_pda(nft_mint)),
            r(crate::damm::DAMM_ID),
            r(crate::damm::POOL_AUTHORITY),
            r(crate::damm::event_authority()),
            r(TOKEN),
            r(ATA_PROGRAM),
            r(SYSTEM),
        ],
    )
}

// ------------------------------------------------------------------ account mirrors

pub mod state {
    pub const LOBBY: u8 = 0;
    pub const CURVE: u8 = 1;
    pub const FAILED: u8 = 2;
    pub const CURVE_FULL: u8 = 3;
    pub const BREAKOUT: u8 = 4;
}

/// Mirror of `state::launch::Launch` (same field order; borsh 0.10 and 1.x share the wire format).
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, PartialEq)]
pub struct Launch {
    pub creator: [u8; 32],
    pub mint: [u8; 32],
    pub state: u8,
    pub bonded: bool,
    pub lobby_start: i64,
    pub lobby_end: i64,
    pub min_lamports: u64,
    pub cap_lamports: u64,
    pub claim_fee_bps: u16,
    pub total_deposited: u64,
    pub depositors: u32,
    pub accepted_total: u64,
    pub stampede_tokens: u64,
    pub virtual_sol: u64,
    pub virtual_token: u64,
    pub virtual_token_floor: u64,
    pub initial_virtual_sol: u64,
    pub dex_pool: [u8; 32],
    pub dex_position: [u8; 32],
    pub position_nft_mint: [u8; 32],
    pub creator_payout: [u8; 32],
    pub creator_burn_bps: u16,
    pub committed_tokens: u64,
    pub min_commit_tokens: u64,
    pub creator_forfeited: bool,
    pub burn_reserve: u64,
    pub total_bought_back: u64,
    pub bump: u8,
}

/// 8 (disc) + Launch::INIT_SPACE (computed from the struct above; enum = 1 byte).
pub const LAUNCH_SPACE: usize = 8 + 32 * 2 + 1 + 1 + 8 * 2 + 8 * 2 + 2 + 8 + 4 + 8 * 2 + 8 * 4 + 32 * 4 + 2 + 8 * 2 + 1 + 8 * 2 + 1;

impl Launch {
    pub fn decode(data: &[u8]) -> Launch {
        Launch::deserialize(&mut &data[8..]).expect("decode Launch")
    }
    pub fn encode_into(&self, data: &mut [u8]) {
        let mut buf = Vec::new();
        self.serialize(&mut buf).unwrap();
        data[8..8 + buf.len()].copy_from_slice(&buf);
    }
    /// Same formula as `breakout::breakout_sweep_sol` on the program side.
    pub fn refund_rounding_buffer(&self) -> u64 {
        if self.total_deposited > self.cap_lamports { self.depositors as u64 } else { 0 }
    }
    pub fn breakout_sweep_sol(&self) -> u64 {
        self.accepted_total + (self.virtual_sol - self.initial_virtual_sol) - self.refund_rounding_buffer()
    }
}

#[derive(BorshDeserialize, Debug)]
pub struct Pool3 {
    pub total_in: u64,
    pub total_out: u64,
    pub bump: u8,
}
impl Pool3 {
    pub fn decode(data: &[u8]) -> Pool3 { Pool3::deserialize(&mut &data[8..]).unwrap() }
}
