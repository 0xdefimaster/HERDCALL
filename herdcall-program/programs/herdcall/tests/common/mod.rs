//! Instruction-level test harness (solana-program-test, NATIVE execution).
//!
//! The HerdCall program runs as a native processor (not SBF). SPL Token / ATA run as the
//! BPF builds bundled with solana-program-test. Meteora DAMM v2 is NOT available here, so
//! `breakout`, `claim_position_fee` and `buyback_burn` CPIs cannot be exercised; where a test
//! needs the post-breakout state it is SIMULATED and the test says so.
#![allow(dead_code)]

use anchor_lang::{AccountDeserialize, AccountSerialize, InstructionData, ToAccountMetas};
use herdcall::state::*;
use solana_program_test::*;
use solana_sdk::{
    account::{Account, AccountSharedData, ReadableAccount},
    bpf_loader_upgradeable::{self, UpgradeableLoaderState},
    clock::Clock,
    instruction::{Instruction, InstructionError},
    program_pack::Pack,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    system_program,
    transaction::{Transaction, TransactionError},
};

pub const SOL: u64 = 1_000_000_000;

fn process(
    program_id: &Pubkey,
    accounts: &[anchor_lang::prelude::AccountInfo],
    data: &[u8],
) -> anchor_lang::solana_program::entrypoint::ProgramResult {
    // anchor 0.30 `entry` wants `&'info [AccountInfo<'info>]`; processor! gives a shorter lifetime.
    let accounts: &[anchor_lang::prelude::AccountInfo] = unsafe { std::mem::transmute(accounts) };
    herdcall::entry(program_id, accounts, data)
}

pub fn pda(seeds: &[&[u8]]) -> Pubkey {
    Pubkey::find_program_address(seeds, &herdcall::ID).0
}
pub fn config_pda() -> Pubkey { pda(&[herdcall::constants::CONFIG_SEED]) }
pub fn rekt_pda() -> Pubkey { pda(&[herdcall::constants::REKT_POOL_SEED]) }
pub fn vault_pda() -> Pubkey { pda(&[herdcall::constants::HERD_VAULT_SEED]) }
pub fn protocol_pda() -> Pubkey { pda(&[herdcall::constants::PROTOCOL_POOL_SEED]) }
pub fn dao_pda() -> Pubkey { pda(&[herdcall::constants::DAO_POOL_SEED]) }
pub fn launch_pda(mint: &Pubkey) -> Pubkey { pda(&[herdcall::constants::LAUNCH_SEED, mint.as_ref()]) }
pub fn order_pda(launch: &Pubkey, owner: &Pubkey) -> Pubkey {
    pda(&[herdcall::constants::ORDER_SEED, launch.as_ref(), owner.as_ref()])
}
pub fn ata(owner: &Pubkey, mint: &Pubkey) -> Pubkey {
    spl_associated_token_account::get_associated_token_address(owner, mint)
}
pub fn program_data_pda() -> Pubkey {
    Pubkey::find_program_address(&[herdcall::ID.as_ref()], &bpf_loader_upgradeable::id()).0
}

pub struct Env {
    pub ctx: ProgramTestContext,
    /// Upgrade authority recorded in the (synthetic) ProgramData account.
    pub admin: Keypair,
    pub builders: Pubkey,
    pub ops: Pubkey,
    pub treasury: Pubkey,
}

/// `upgrade_authority`: what to put in ProgramData. `Some(admin)` = normal upgradeable program,
/// `None` = immutable program.
pub async fn start_with(upgrade_authority_is_admin: bool) -> Env {
    let mut pt = ProgramTest::new("herdcall", herdcall::ID, processor!(process));
    pt.prefer_bpf(false);
    let admin = Keypair::new();
    pt.add_account(admin.pubkey(), Account { lamports: 100 * SOL, owner: system_program::id(), ..Account::default() });

    // Native processors have no ProgramData account; add a synthetic one so the
    // upgrade-authority gate on initialize_config can be exercised.
    let state = UpgradeableLoaderState::ProgramData {
        slot: 0,
        upgrade_authority_address: if upgrade_authority_is_admin { Some(admin.pubkey()) } else { None },
    };
    let data = bincode::serialize(&state).unwrap();
    pt.add_account(
        program_data_pda(),
        Account { lamports: 10 * SOL, data, owner: bpf_loader_upgradeable::id(), executable: false, rent_epoch: 0 },
    );

    let builders = Pubkey::new_unique();
    let ops = Pubkey::new_unique();
    let treasury = Pubkey::new_unique();
    for w in [builders, ops, treasury] {
        // fund destination wallets so credits never trip rent-exemption checks
        pt.add_account(w, Account { lamports: SOL, owner: system_program::id(), ..Account::default() });
    }
    let ctx = pt.start_with_context().await;
    Env { ctx, admin, builders, ops, treasury }
}

pub async fn start() -> Env { start_with(true).await }

impl Env {
    pub fn payer(&self) -> Keypair { self.ctx.payer.insecure_clone() }

    pub async fn send(&mut self, ixs: &[Instruction], signers: &[&Keypair]) -> Result<(), BanksClientError> {
        let bh = self.ctx.banks_client.get_latest_blockhash().await.unwrap();
        let bh = self.ctx.banks_client.get_new_latest_blockhash(&bh).await.unwrap_or(bh);
        let payer = self.ctx.payer.insecure_clone();
        let mut all: Vec<&Keypair> = vec![&payer];
        all.extend_from_slice(signers);
        let tx = Transaction::new_signed_with_payer(ixs, Some(&payer.pubkey()), &all, bh);
        self.ctx.banks_client.process_transaction(tx).await
    }

    pub async fn funded(&mut self, lamports: u64) -> Keypair {
        let k = Keypair::new();
        self.ctx.set_account(&k.pubkey(), &AccountSharedData::new(lamports, 0, &system_program::id()));
        k
    }

    pub async fn lamports(&mut self, k: &Pubkey) -> u64 {
        self.ctx.banks_client.get_balance(*k).await.unwrap()
    }

    pub async fn account(&mut self, k: &Pubkey) -> Option<Account> {
        self.ctx.banks_client.get_account(*k).await.unwrap()
    }

    pub async fn token_balance(&mut self, k: &Pubkey) -> u64 {
        match self.account(k).await {
            Some(a) => spl_token::state::Account::unpack(&a.data).unwrap().amount,
            None => 0,
        }
    }

    pub async fn anchor<T: AccountDeserialize>(&mut self, k: &Pubkey) -> T {
        let a = self.account(k).await.expect("account missing");
        T::try_deserialize(&mut &a.data[..]).unwrap()
    }

    pub async fn write_anchor<T: AccountSerialize>(&mut self, k: &Pubkey, v: &T, lamports: Option<u64>) {
        let mut a = self.account(k).await.unwrap();
        let mut buf = Vec::with_capacity(a.data.len());
        v.try_serialize(&mut buf).unwrap();
        a.data[..buf.len()].copy_from_slice(&buf);
        if let Some(l) = lamports {
            a.lamports = l;
        }
        self.ctx.set_account(k, &AccountSharedData::from(a));
    }

    pub async fn warp_secs(&mut self, secs: i64) {
        let mut c: Clock = self.ctx.banks_client.get_sysvar().await.unwrap();
        c.unix_timestamp += secs;
        self.ctx.set_sysvar(&c);
    }

    pub async fn rent_min(&mut self, len: usize) -> u64 {
        let r = self.ctx.banks_client.get_rent().await.unwrap();
        r.minimum_balance(len)
    }
}

pub fn default_params() -> ParamsArgs {
    ParamsArgs {
        rekt_select_bps: 385,
        rekt_payout_cap_bps: 5_000,
        rekt_reserve_floor: 0,
        min_stampede_lamports: SOL,
        stampede_cap_lamports: 2 * SOL,
        claim_fee_bps: 100,
        damm_config: Pubkey::new_unique(),
        breakout_cost_lamports: 5_000_000,
        min_creator_commit_bps: 0,
        paused: false,
    }
}

pub fn custom_code(e: &BanksClientError) -> Option<u32> {
    match e {
        BanksClientError::TransactionError(TransactionError::InstructionError(_, InstructionError::Custom(c)))
        | BanksClientError::SimulationError { err: TransactionError::InstructionError(_, InstructionError::Custom(c)), .. } => Some(*c),
        _ => None,
    }
}

pub fn herr(e: herdcall::errors::HerdError) -> u32 { u32::from(e) }

// ------------------------------------------------------------------ instruction builders

pub fn ix(accounts: impl ToAccountMetas, data: impl InstructionData) -> Instruction {
    Instruction { program_id: herdcall::ID, accounts: accounts.to_account_metas(None), data: data.data() }
}

pub fn ix_create_launch(creator: &Pubkey, mint: &Pubkey) -> Instruction {
    let launch = launch_pda(mint);
    ix(
        herdcall::accounts::CreateLaunch {
            creator: *creator,
            config: config_pda(),
            mint: *mint,
            launch,
            stampede_escrow: ata(&launch, mint),
            token_program: spl_token::id(),
            associated_token_program: spl_associated_token_account::id(),
            system_program: system_program::id(),
        },
        herdcall::instruction::CreateLaunch { lobby_secs: 15, bonded: false },
    )
}

pub fn ix_deposit(owner: &Pubkey, mint: &Pubkey, amount: u64) -> Instruction {
    let launch = launch_pda(mint);
    ix(
        herdcall::accounts::LobbyDeposit {
            owner: *owner,
            config: config_pda(),
            launch,
            order: order_pda(&launch, owner),
            system_program: system_program::id(),
        },
        herdcall::instruction::LobbyDeposit { amount },
    )
}

pub fn ix_settle(caller: &Pubkey, mint: &Pubkey) -> Instruction {
    let launch = launch_pda(mint);
    ix(
        herdcall::accounts::SettleStampede {
            caller: *caller,
            launch,
            mint: *mint,
            stampede_escrow: ata(&launch, mint),
            token_program: spl_token::id(),
        },
        herdcall::instruction::SettleStampede {},
    )
}

pub fn ix_claim(owner: &Pubkey, mint: &Pubkey) -> Instruction {
    let launch = launch_pda(mint);
    ix(
        herdcall::accounts::ClaimStampede {
            owner: *owner,
            launch,
            mint: *mint,
            order: order_pda(&launch, owner),
            owner_token: ata(owner, mint),
            stampede_escrow: ata(&launch, mint),
            protocol_pool: protocol_pda(),
            token_program: spl_token::id(),
            associated_token_program: spl_associated_token_account::id(),
            system_program: system_program::id(),
        },
        herdcall::instruction::ClaimStampede {},
    )
}

pub fn ix_buy(trader: &Pubkey, mint: &Pubkey, creator: &Pubkey, ui: &Pubkey, gross_in: u64) -> Instruction {
    let launch = launch_pda(mint);
    ix(
        herdcall::accounts::Buy {
            trader: *trader,
            config: config_pda(),
            launch,
            mint: *mint,
            trader_token: ata(trader, mint),
            creator: *creator,
            ui_receiver: *ui,
            rekt_pool: rekt_pda(),
            protocol_pool: protocol_pda(),
            token_program: spl_token::id(),
            associated_token_program: spl_associated_token_account::id(),
            system_program: system_program::id(),
        },
        herdcall::instruction::Buy { gross_in, min_tokens_out: 0 },
    )
}

/// Simulates the on-chain effects of a successful `breakout` that this harness cannot run
/// (Meteora DAMM v2 is not loaded): state -> Breakout, the swept SOL leaves the launch PDA
/// using the SAME accounting function the instruction uses, and the mint authority is revoked.
pub async fn simulate_breakout(env: &mut Env, mint: &Pubkey) -> u64 {
    let launch_key = launch_pda(mint);
    let mut l: Launch = env.anchor(&launch_key).await;
    assert_eq!(l.state, LaunchState::CurveFull, "simulate_breakout expects CurveFull");
    let swept = herdcall::breakout::breakout_sweep_sol(&l).unwrap();
    let lamports = env.lamports(&launch_key).await;
    l.state = LaunchState::Breakout;
    env.write_anchor(&launch_key, &l, Some(lamports - swept)).await;

    let mut m = env.account(mint).await.unwrap();
    let mut mint_state = spl_token::state::Mint::unpack(&m.data).unwrap();
    mint_state.mint_authority = solana_sdk::program_option::COption::None;
    spl_token::state::Mint::pack(mint_state, &mut m.data).unwrap();
    env.ctx.set_account(mint, &AccountSharedData::from(m));
    swept
}

pub fn lamports_of(a: &Account) -> u64 { a.lamports() }

pub async fn init_config(env: &mut Env) -> Result<(), BanksClientError> {
    let admin = env.admin.insecure_clone();
    let args = herdcall::instructions::InitializeConfigArgs {
        treasury: env.treasury,
        builders_fund: env.builders,
        ops_wallet: env.ops,
        params: default_params(),
    };
    let i = ix(
        herdcall::accounts::InitializeConfig {
            authority: admin.pubkey(),
            config: config_pda(),
            rekt_pool: rekt_pda(),
            herd_vault: vault_pda(),
            protocol_pool: protocol_pda(),
            dao_pool: dao_pda(),
            system_program: system_program::id(),
        },
        herdcall::instruction::InitializeConfig { args },
    );
    env.send(&[i], &[&admin]).await
}
