//! Shared harness: loads the compiled program into LiteSVM and builds instructions from Anchor's generated types.
#![allow(dead_code)]

use anchor_lang::prelude::Pubkey;
use anchor_lang::{AccountDeserialize, InstructionData, ToAccountMetas};
use herdcall::state::*;
use litesvm::LiteSVM;
use solana_address::Address;
use solana_clock::Clock;
use solana_instruction::{AccountMeta, Instruction};
use solana_keypair::Keypair;
use solana_signer::Signer;
use solana_transaction::Transaction;

pub const SOL: u64 = 1_000_000_000;
pub const TOKEN: u64 = 1_000_000;

pub fn addr(p: &Pubkey) -> Address {
    Address::new_from_array(p.to_bytes())
}

pub fn pk(a: &Address) -> Pubkey {
    Pubkey::new_from_array(a.to_bytes())
}

pub fn example_params() -> LaunchParams {
    LaunchParams {
        v0: 30 * SOL,
        t0: 1_000_000_000 * TOKEN,
        total_supply: 1_000_000_000 * TOKEN,
        curve_supply: 800_000_000 * TOKEN,
        lp_supply: 200_000_000 * TOKEN,
        fee_bps: 100,
        min_deposit: SOL / 100,
        min_total: SOL,
        max_total: 10_000 * SOL,
        max_per_wallet: 0,
        migration_fee: SOL / 50,
        settle_window: 300,
        name_bond: SOL / 4,
    }
}

pub fn pda(seeds: &[&[u8]]) -> Pubkey {
    Pubkey::find_program_address(seeds, &herdcall::ID).0
}

pub fn ata(owner: &Pubkey, mint: &Pubkey) -> Pubkey {
    anchor_spl::associated_token::get_associated_token_address(owner, mint)
}

pub struct Env {
    pub svm: LiteSVM,
    pub admin: Keypair,
    pub now: i64,
    launches: u32,
}

pub struct LaunchKeys {
    pub mint: Pubkey,
    pub launch: Pubkey,
    pub token_vault: Pubkey,
    pub sol_vault: Pubkey,
    pub creator_fees: Pubkey,
    pub creator_escrow: Pubkey,
    pub ticker_record: Pubkey,
    pub name_record: Pubkey,
    pub ticker: String,
    pub name: String,
    pub creator: Keypair,
}

pub fn ticker_pda(ticker: &str) -> Pubkey {
    pda(&[TICKER_SEED, ticker.as_bytes()])
}

/// Record address of a name; for an invalid name, a dummy hash so the program's own check can reject it.
pub fn name_hash(name: &str) -> [u8; 32] {
    name_seed(name).unwrap_or([7; 32])
}

pub fn name_pda(name: &str) -> Pubkey {
    pda(&[NAME_SEED, name_hash(name).as_ref()])
}

impl Env {
    pub fn new() -> Self {
        let mut svm = LiteSVM::new();
        let so = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../target/deploy/herdcall.so"))
            .expect("build the program first: cargo build-sbf --manifest-path programs/herdcall/Cargo.toml");
        svm.add_program(addr(&herdcall::ID), &so).unwrap();
        let admin = Keypair::new();
        svm.airdrop(&admin.pubkey(), 1_000 * SOL).unwrap();
        let mut env = Self { svm, admin, now: 1_800_000_000, launches: 0 };
        env.set_time(env.now);
        env
    }

    pub fn set_time(&mut self, t: i64) {
        self.now = t;
        let mut c: Clock = self.svm.get_sysvar();
        c.unix_timestamp = t;
        self.svm.set_sysvar(&c);
        self.svm.expire_blockhash();
    }

    pub fn user(&mut self, sol: u64) -> Keypair {
        let k = Keypair::new();
        self.svm.airdrop(&k.pubkey(), sol * SOL).unwrap();
        k
    }

    pub fn send(&mut self, ix: Instruction, signers: &[&Keypair]) -> Result<(), String> {
        let payer = signers[0].pubkey();
        let tx = Transaction::new_signed_with_payer(&[ix], Some(&payer), signers, self.svm.latest_blockhash());
        let r = self.svm.send_transaction(tx).map(|_| ()).map_err(|e| format!("{:?}\n{}", e.err, e.meta.logs.join("\n")));
        self.svm.expire_blockhash();
        r
    }

    pub fn lamports(&self, k: &Pubkey) -> u64 {
        self.svm.get_account(&addr(k)).map(|a| a.lamports).unwrap_or(0)
    }

    pub fn data(&self, k: &Pubkey) -> Vec<u8> {
        self.svm.get_account(&addr(k)).map(|a| a.data).unwrap_or_default()
    }

    pub fn account<T: AccountDeserialize>(&self, k: &Pubkey) -> T {
        T::try_deserialize(&mut self.data(k).as_slice()).expect("deserialize")
    }

    pub fn launch(&self, k: &LaunchKeys) -> Launch {
        self.account(&k.launch)
    }

    /// SPL token account balance (amount at offset 64).
    pub fn token_balance(&self, account: &Pubkey) -> u64 {
        let d = self.data(account);
        if d.len() < 72 {
            return 0;
        }
        u64::from_le_bytes(d[64..72].try_into().unwrap())
    }

    /// SPL mint: (has mint authority, supply).
    pub fn mint_info(&self, mint: &Pubkey) -> (bool, u64) {
        let d = self.data(mint);
        (d[0..4] != [0, 0, 0, 0], u64::from_le_bytes(d[36..44].try_into().unwrap()))
    }

    pub fn ix(&self, metas: Vec<anchor_lang::prelude::AccountMeta>, data: Vec<u8>) -> Instruction {
        Instruction {
            program_id: addr(&herdcall::ID),
            accounts: metas.into_iter().map(|m| AccountMeta { pubkey: addr(&m.pubkey), is_signer: m.is_signer, is_writable: m.is_writable }).collect(),
            data,
        }
    }

    pub fn initialize(&mut self, params: LaunchParams) -> Result<(), String> {
        let admin = self.admin.insecure_clone();
        let metas = herdcall::accounts::Initialize {
            admin: pk(&admin.pubkey()),
            config: pda(&[CONFIG_SEED]),
            protocol: pda(&[PROTOCOL_SEED]),
            rekt_back: pda(&[REKT_BACK_SEED]),
            system_program: anchor_lang::system_program::ID,
        }
        .to_account_metas(None);
        let ix = self.ix(metas, herdcall::instruction::Initialize { params }.data());
        self.send(ix, &[&admin])
    }

    /// A launch with a fresh ticker (T0, T1, ...).
    pub fn create_launch(&mut self, bonded: bool, lobby_secs: i64) -> Result<LaunchKeys, String> {
        self.launches += 1;
        let ticker = format!("T{}", self.launches);
        self.create_launch_named(bonded, lobby_secs, &ticker)
    }

    /// A launch with the given ticker and the name "<ticker> Coin".
    pub fn create_launch_named(&mut self, bonded: bool, lobby_secs: i64, ticker: &str) -> Result<LaunchKeys, String> {
        self.create_launch_full(bonded, lobby_secs, ticker, &format!("{ticker} Coin"))
    }

    pub fn create_launch_full(&mut self, bonded: bool, lobby_secs: i64, ticker: &str, name: &str) -> Result<LaunchKeys, String> {
        let creator = self.user(10);
        let mint_kp = Keypair::new();
        let mint = pk(&mint_kp.pubkey());
        let launch = pda(&[LAUNCH_SEED, mint.as_ref()]);
        let keys = LaunchKeys {
            mint,
            launch,
            token_vault: ata(&launch, &mint),
            sol_vault: pda(&[SOL_VAULT_SEED, launch.as_ref()]),
            creator_fees: pda(&[CREATOR_FEES_SEED, launch.as_ref()]),
            creator_escrow: pda(&[CREATOR_ESCROW_SEED, launch.as_ref()]),
            ticker_record: ticker_pda(ticker),
            name_record: name_pda(name),
            ticker: ticker.to_string(),
            name: name.to_string(),
            creator,
        };
        let metas = herdcall::accounts::CreateLaunch {
            creator: pk(&keys.creator.pubkey()),
            config: pda(&[CONFIG_SEED]),
            mint,
            launch,
            token_vault: keys.token_vault,
            sol_vault: keys.sol_vault,
            creator_fees: keys.creator_fees,
            creator_escrow: keys.creator_escrow,
            ticker_record: keys.ticker_record,
            name_record: keys.name_record,
            token_program: anchor_spl::token::ID,
            associated_token_program: anchor_spl::associated_token::ID,
            system_program: anchor_lang::system_program::ID,
        }
        .to_account_metas(None);
        let ix = self.ix(metas, herdcall::instruction::CreateLaunch {
                bonded,
                lobby_secs,
                ticker: ticker.to_string(),
                name: name.to_string(),
                name_hash: name_hash(name),
            }
            .data());
        let creator = keys.creator.insecure_clone();
        self.send(ix, &[&creator, &mint_kp])?;
        Ok(keys)
    }

    pub fn deposit(&mut self, k: &LaunchKeys, user: &Keypair, amount: u64) -> Result<(), String> {
        let u = pk(&user.pubkey());
        let metas = herdcall::accounts::DepositCtx {
            user: u,
            launch: k.launch,
            sol_vault: k.sol_vault,
            deposit: pda(&[DEPOSIT_SEED, k.launch.as_ref(), u.as_ref()]),
            system_program: anchor_lang::system_program::ID,
        }
        .to_account_metas(None);
        let ix = self.ix(metas, herdcall::instruction::Deposit { amount }.data());
        self.send(ix, &[user])
    }

    pub fn settle(&mut self, k: &LaunchKeys, caller: &Keypair) -> Result<(), String> {
        let metas = herdcall::accounts::Settle {
            launch: k.launch,
            sol_vault: k.sol_vault,
            creator_fees: k.creator_fees,
            creator_escrow: k.creator_escrow,
            rekt_back: pda(&[REKT_BACK_SEED]),
            protocol: pda(&[PROTOCOL_SEED]),
            ticker_record: k.ticker_record,
            name_record: k.name_record,
            creator: pk(&k.creator.pubkey()),
        }
        .to_account_metas(None);
        let ix = self.ix(metas, herdcall::instruction::Settle {}.data());
        self.send(ix, &[caller])
    }

    pub fn claim(&mut self, k: &LaunchKeys, user: &Keypair) -> Result<(), String> {
        let u = pk(&user.pubkey());
        let metas = herdcall::accounts::Claim {
            user: u,
            launch: k.launch,
            mint: k.mint,
            deposit: pda(&[DEPOSIT_SEED, k.launch.as_ref(), u.as_ref()]),
            owner: u,
            sol_vault: k.sol_vault,
            token_vault: k.token_vault,
            user_ata: ata(&u, &k.mint),
            token_program: anchor_spl::token::ID,
            associated_token_program: anchor_spl::associated_token::ID,
            system_program: anchor_lang::system_program::ID,
        }
        .to_account_metas(None);
        let ix = self.ix(metas, herdcall::instruction::Claim {}.data());
        self.send(ix, &[user])
    }

    pub fn refund(&mut self, k: &LaunchKeys, user: &Keypair) -> Result<(), String> {
        let u = pk(&user.pubkey());
        let metas = herdcall::accounts::Refund {
            user: u,
            launch: k.launch,
            deposit: pda(&[DEPOSIT_SEED, k.launch.as_ref(), u.as_ref()]),
            sol_vault: k.sol_vault,
        }
        .to_account_metas(None);
        let ix = self.ix(metas, herdcall::instruction::Refund {}.data());
        self.send(ix, &[user])
    }

    /// Trade accounts in order: trader 0, launch 1, mint 2, sol_vault 3, token_vault 4, trader_ata 5, creator_fees 6,
    /// creator_escrow 7, rekt_back 8, protocol 9, interface 10, token 11, associated token 12, system 13.
    pub fn trade_metas(&self, k: &LaunchKeys, trader: &Pubkey, interface: Option<Pubkey>) -> Vec<anchor_lang::prelude::AccountMeta> {
        herdcall::accounts::Trade {
            trader: *trader,
            launch: k.launch,
            mint: k.mint,
            sol_vault: k.sol_vault,
            token_vault: k.token_vault,
            trader_ata: ata(trader, &k.mint),
            creator_fees: k.creator_fees,
            creator_escrow: k.creator_escrow,
            rekt_back: pda(&[REKT_BACK_SEED]),
            protocol: pda(&[PROTOCOL_SEED]),
            interface,
            token_program: anchor_spl::token::ID,
            associated_token_program: anchor_spl::associated_token::ID,
            system_program: anchor_lang::system_program::ID,
        }
        .to_account_metas(None)
    }

    pub fn buy_exact_in_ix(&self, k: &LaunchKeys, trader: &Keypair, gross: u64, min_tokens: u64, interface: Option<Pubkey>) -> Instruction {
        let metas = self.trade_metas(k, &pk(&trader.pubkey()), interface);
        self.ix(metas, herdcall::instruction::BuyExactIn { gross, min_tokens }.data())
    }

    pub fn buy_exact_in(&mut self, k: &LaunchKeys, trader: &Keypair, gross: u64, min_tokens: u64, interface: Option<Pubkey>) -> Result<(), String> {
        let ix = self.buy_exact_in_ix(k, trader, gross, min_tokens, interface);
        self.send(ix, &[trader])
    }

    pub fn buy_exact_out(&mut self, k: &LaunchKeys, trader: &Keypair, tokens: u64, max_gross: u64, interface: Option<Pubkey>) -> Result<(), String> {
        let metas = self.trade_metas(k, &pk(&trader.pubkey()), interface);
        let ix = self.ix(metas, herdcall::instruction::BuyExactOut { tokens, max_gross }.data());
        self.send(ix, &[trader])
    }

    pub fn sell(&mut self, k: &LaunchKeys, trader: &Keypair, tokens: u64, min_received: u64, interface: Option<Pubkey>) -> Result<(), String> {
        let metas = self.trade_metas(k, &pk(&trader.pubkey()), interface);
        let ix = self.ix(metas, herdcall::instruction::Sell { tokens, min_received }.data());
        self.send(ix, &[trader])
    }

    pub fn breakout(&mut self, k: &LaunchKeys, keeper: &Keypair) -> Result<(), String> {
        let metas = herdcall::accounts::Breakout {
            keeper: pk(&keeper.pubkey()),
            launch: k.launch,
            mint: k.mint,
            token_vault: k.token_vault,
            sol_vault: k.sol_vault,
            token_program: anchor_spl::token::ID,
        }
        .to_account_metas(None);
        let ix = self.ix(metas, herdcall::instruction::Breakout {}.data());
        self.send(ix, &[keeper])
    }

    pub fn register_interface(&mut self, owner: &Keypair) -> Result<Pubkey, String> {
        let o = pk(&owner.pubkey());
        let iface = pda(&[INTERFACE_SEED, o.as_ref()]);
        let metas = herdcall::accounts::RegisterInterface { owner: o, interface: iface, system_program: anchor_lang::system_program::ID }.to_account_metas(None);
        let ix = self.ix(metas, herdcall::instruction::RegisterInterface {}.data());
        self.send(ix, &[owner])?;
        Ok(iface)
    }

    pub fn withdraw_interface_fees(&mut self, owner: &Keypair, interface: Pubkey) -> Result<(), String> {
        let metas = herdcall::accounts::WithdrawInterface { owner: pk(&owner.pubkey()), interface }.to_account_metas(None);
        let ix = self.ix(metas, herdcall::instruction::WithdrawInterfaceFees {}.data());
        self.send(ix, &[owner])
    }

    pub fn withdraw_creator_fees(&mut self, k: &LaunchKeys, creator: &Keypair) -> Result<(), String> {
        let metas = herdcall::accounts::WithdrawCreatorFees { creator: pk(&creator.pubkey()), launch: k.launch, creator_fees: k.creator_fees }
            .to_account_metas(None);
        let ix = self.ix(metas, herdcall::instruction::WithdrawCreatorFees {}.data());
        self.send(ix, &[creator])
    }

    pub fn rent(&self, k: &Pubkey) -> u64 {
        self.svm.minimum_balance_for_rent_exemption(self.data(k).len())
    }

    pub fn release_name(&mut self, k: &LaunchKeys, caller: &Keypair) -> Result<(), String> {
        let metas = herdcall::accounts::ReleaseName {
            ticker_record: k.ticker_record,
            name_record: k.name_record,
            launch: k.launch,
            creator: pk(&k.creator.pubkey()),
        }.to_account_metas(None);
        let ix = self.ix(metas, herdcall::instruction::ReleaseName {}.data());
        self.send(ix, &[caller])
    }

    pub fn record(&self, key: &Pubkey) -> Option<NameRecord> {
        let d = self.data(key);
        if d.is_empty() {
            return None;
        }
        Some(NameRecord::try_deserialize(&mut d.as_slice()).expect("name record"))
    }
}
