//! End-to-end scenario builder on LiteSVM: config -> launch -> oversubscribed lobby -> settle ->
//! curve filled to CurveFull. Every step goes through the real `herdcall.so`.

use agave_feature_set::FeatureSet;
use litesvm::types::{FailedTransactionMetadata, TransactionMetadata};
use litesvm::LiteSVM;
use solana_account::Account;
use solana_address::Address;
use solana_clock::Clock;
use solana_compute_budget_interface::ComputeBudgetInstruction;
use solana_instruction::Instruction;
use solana_keypair::Keypair;
use solana_loader_v3_interface::state::UpgradeableLoaderState;
use solana_program_pack::Pack;
use solana_signer::Signer;
use solana_transaction::Transaction;

use crate::env::load_programs;
use crate::hc::{self, Launch, SOL};

/// Runtime feature profiles. Mainnet activation status of the two ABI features could not be
/// queried from the CI sandbox, so lamport/CPI-ordering tests run under both.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Profile {
    /// `FeatureSet::all_enabled()` (LiteSVM default)
    AllFeatures,
    /// all_enabled minus `stricter_abi_and_runtime_constraints` and `account_data_direct_mapping`
    LegacyAbi,
}
pub const PROFILES: [Profile; 2] = [Profile::AllFeatures, Profile::LegacyAbi];

pub fn new_svm(p: Profile) -> LiteSVM {
    let mut fs = FeatureSet::all_enabled();
    if p == Profile::LegacyAbi {
        fs.deactivate(&agave_feature_set::stricter_abi_and_runtime_constraints::id());
        fs.deactivate(&agave_feature_set::account_data_direct_mapping::id());
    }
    let mut svm = LiteSVM::default()
        .with_feature_set(fs)
        .with_builtins()
        .with_lamports(1_000_000u64 * SOL)
        .with_sysvars()
        .with_feature_accounts()
        .with_default_programs()
        .with_sigverify(true)
        .with_blockhash_check(true)
        .with_log_bytes_limit(Some(100_000));
    load_programs(&mut svm);
    svm
}

pub type TxResult = Result<TransactionMetadata, FailedTransactionMetadata>;

pub fn send(svm: &mut LiteSVM, payer: &Keypair, ixs: &[Instruction], extra: &[&Keypair]) -> TxResult {
    let mut all = vec![ComputeBudgetInstruction::set_compute_unit_limit(1_400_000)];
    all.extend_from_slice(ixs);
    let mut signers: Vec<&Keypair> = vec![payer];
    signers.extend_from_slice(extra);
    let tx = Transaction::new_signed_with_payer(&all, Some(&payer.pubkey()), &signers, svm.latest_blockhash());
    let r = svm.send_transaction(tx);
    svm.expire_blockhash();
    r
}

pub fn ok(r: TxResult, what: &str) -> TransactionMetadata {
    match r {
        Ok(m) => m,
        Err(e) => panic!("{what} failed: {:?}\n{}", e.err, e.meta.logs.join("\n")),
    }
}

/// One-line summary of a failure for the CI report.
pub fn describe(r: &TxResult) -> String {
    match r {
        Ok(m) => format!("OK cu={}", m.compute_units_consumed),
        Err(e) => {
            let last: Vec<&String> = e.meta.logs.iter().rev().take(4).collect();
            format!("ERR {:?} | {}", e.err, last.into_iter().rev().cloned().collect::<Vec<_>>().join(" || "))
        }
    }
}

pub fn funded(svm: &mut LiteSVM, lamports: u64) -> Keypair {
    let k = Keypair::new();
    svm.airdrop(&k.pubkey(), lamports).unwrap();
    k
}

pub fn lamports(svm: &LiteSVM, k: &Address) -> u64 {
    svm.get_account(k).map(|a| a.lamports).unwrap_or(0)
}

pub fn token_amount(svm: &LiteSVM, k: &Address) -> u64 {
    match svm.get_account(k) {
        Some(a) if a.data.len() >= spl_token_interface::state::Account::LEN => {
            spl_token_interface::state::Account::unpack_from_slice(&a.data[..spl_token_interface::state::Account::LEN])
                .map(|t| t.amount)
                .unwrap_or(0)
        }
        _ => 0,
    }
}

pub fn mint_info(svm: &LiteSVM, mint: &Address) -> spl_token_interface::state::Mint {
    let a = svm.get_account(mint).unwrap();
    spl_token_interface::state::Mint::unpack_from_slice(&a.data).unwrap()
}

pub fn read_launch(svm: &LiteSVM, mint: &Address) -> Launch {
    Launch::decode(&svm.get_account(&hc::launch_pda(mint)).expect("launch").data)
}

pub fn warp_secs(svm: &mut LiteSVM, secs: i64) {
    let mut c: Clock = svm.get_sysvar();
    c.unix_timestamp += secs;
    c.slot += (secs as u64) * 2 + 1;
    svm.set_sysvar(&c);
}

/// Snapshot of (lamports, data) for a set of accounts; used for rollback / conservation checks.
pub fn snapshot(svm: &LiteSVM, keys: &[Address]) -> Vec<(Address, u64, Vec<u8>)> {
    keys.iter()
        .map(|k| {
            let a = svm.get_account(k);
            (*k, a.as_ref().map(|a| a.lamports).unwrap_or(0), a.map(|a| a.data).unwrap_or_default())
        })
        .collect()
}

pub fn total_lamports(svm: &LiteSVM, keys: &[Address]) -> u128 {
    keys.iter().map(|k| lamports(svm, k) as u128).sum()
}

pub struct World {
    pub svm: LiteSVM,
    pub profile: Profile,
    pub admin: Keypair,
    pub payer: Keypair,
    pub creator: Keypair,
    pub mint: Keypair,
    pub users: Vec<(Keypair, u64)>,
    pub buyer: Keypair,
    pub ui: Keypair,
    pub damm_config: Address,
    pub wallets: [Keypair; 3],
}

pub enum ConfigKind {
    /// The real mainnet public config, unchanged (anyone may create pools with it).
    Public,
    /// Same bytes, pool_creator_authority = given key (private config).
    PrivateFor(Address),
}

/// Program upgrade authority -> `admin` (LiteSVM installs ProgramData with authority None).
fn set_upgrade_authority(svm: &mut LiteSVM, admin: &Address) {
    let pd = hc::program_data_pda();
    let mut a = svm.get_account(&pd).expect("herdcall ProgramData");
    let meta = bincode::serialize(&UpgradeableLoaderState::ProgramData { slot: 0, upgrade_authority_address: Some(*admin) }).unwrap();
    a.data[..meta.len()].copy_from_slice(&meta);
    svm.set_account(pd, a).unwrap();
}

impl World {
    /// Config + launch + oversubscribed lobby (4 depositors, cap 2 SOL) + settle. State: Curve.
    pub fn new(profile: Profile, cfg: ConfigKind) -> World {
        let mut svm = new_svm(profile);
        let admin = funded(&mut svm, 100 * SOL);
        let payer = funded(&mut svm, 1_000 * SOL);
        let wallets = [funded(&mut svm, SOL), funded(&mut svm, SOL), funded(&mut svm, SOL)];
        set_upgrade_authority(&mut svm, &admin.pubkey());

        let damm_config = Address::new_unique();
        crate::damm::install_config(&mut svm, damm_config, match cfg {
            ConfigKind::Public => None,
            ConfigKind::PrivateFor(k) => Some(k),
        });

        let ws = [wallets[0].pubkey(), wallets[1].pubkey(), wallets[2].pubkey()];
        ok(send(&mut svm, &admin, &[hc::ix_initialize_config(&admin.pubkey(), ws, hc::default_params(damm_config))], &[]), "initialize_config");

        let creator = funded(&mut svm, 10 * SOL);
        let mint = Keypair::new();
        ok(send(&mut svm, &creator, &[hc::ix_create_launch(&creator.pubkey(), &mint.pubkey())], &[&mint]), "create_launch");

        let mut users = vec![];
        for amt in [1_000_000_001u64, 1_333_333_333, 700_000_007, 10_000_019] {
            let u = funded(&mut svm, 5 * SOL);
            ok(send(&mut svm, &u, &[hc::ix_lobby_deposit(&u.pubkey(), &mint.pubkey(), amt)], &[]), "lobby_deposit");
            users.push((u, amt));
        }
        warp_secs(&mut svm, 20);
        ok(send(&mut svm, &payer, &[hc::ix_settle(&payer.pubkey(), &mint.pubkey())], &[]), "settle");
        let buyer = funded(&mut svm, 200 * SOL);
        let ui = funded(&mut svm, SOL);
        World { svm, profile, admin, payer, creator, mint, users, buyer, ui, damm_config, wallets }
    }

    pub fn mint(&self) -> Address { self.mint.pubkey() }
    pub fn launch(&self) -> Launch { read_launch(&self.svm, &self.mint()) }
    pub fn launch_key(&self) -> Address { hc::launch_pda(&self.mint()) }

    pub fn buy(&mut self, gross: u64) -> TxResult {
        let (b, m, c, u) = (self.buyer.insecure_clone(), self.mint(), self.creator.pubkey(), self.ui.pubkey());
        send(&mut self.svm, &b, &[hc::ix_buy(&b.pubkey(), &m, &c, &u, gross)], &[])
    }

    /// Buys until the curve is full (state CurveFull).
    pub fn fill_curve(&mut self) {
        ok(self.buy(60 * SOL), "buy (fill curve)");
        assert_eq!(self.launch().state, hc::state::CURVE_FULL, "curve not full");
    }

    /// What an honest caller computes off-chain for `breakout(liquidity, sqrt_price)`.
    pub fn breakout_plan(&self) -> (u128, u128, u64, u64) {
        let l = self.launch();
        let cost = hc::default_params(self.damm_config).breakout_cost_lamports;
        let wsol = l.breakout_sweep_sol() - cost;
        let cfg = crate::damm::decode_config(&self.svm.get_account(&self.damm_config).unwrap().data);
        let sqrt = crate::damm::sqrt_price_from_amounts(hc::LP_TOKENS, wsol);
        let liq = crate::damm::max_liquidity(&cfg, sqrt, hc::LP_TOKENS, wsol);
        (liq, sqrt, hc::LP_TOKENS, wsol)
    }

    pub fn claim(&mut self, i: usize) -> TxResult {
        let u = self.users[i].0.insecure_clone();
        let m = self.mint();
        send(&mut self.svm, &u, &[hc::ix_claim_stampede(&u.pubkey(), &m)], &[])
    }

    /// Buyer's own token + wSOL accounts (for driving DAMM directly in tests).
    pub fn ensure_wsol(&mut self, owner: &Keypair, lamports_in: u64) -> Address {
        let wsol_ata = hc::ata(&owner.pubkey(), &hc::WSOL);
        let ixs = vec![
            spl_associated_token_account_interface::instruction::create_associated_token_account_idempotent(
                &owner.pubkey(), &owner.pubkey(), &hc::WSOL, &hc::TOKEN),
            solana_system_interface::instruction::transfer(&owner.pubkey(), &wsol_ata, lamports_in),
            spl_token_interface::instruction::sync_native(&hc::TOKEN, &wsol_ata).unwrap(),
        ];
        ok(send(&mut self.svm, owner, &ixs, &[]), "wrap SOL");
        wsol_ata
    }
}

/// Adds lamports to an account without a transaction (test setup only).
pub fn add_lamports(svm: &mut LiteSVM, k: &Address, extra: u64) {
    let mut a: Account = svm.get_account(k).unwrap();
    a.lamports += extra;
    svm.set_account(*k, a).unwrap();
}

/// DAMM pool created by `simulate_breakout`.
#[derive(Clone, Copy, Debug)]
pub struct SimPool {
    pub config: Address,
    pub pool: Address,
    pub position: Address,
    pub nft_mint: Address,
    pub a_vault: Address,
    pub b_vault: Address,
}

impl World {
    /// SIMULATED breakout, used to test post-breakout instructions independently of `breakout`:
    /// the buyer creates the DAMM pool through the real DAMM program with `creator` (= position
    /// NFT owner) set to the launch's breakout_authority PDA, exactly as `breakout` does; then the
    /// launch account is switched to Breakout (dex fields set, the curve SOL sweep removed from its
    /// lamports) and the mint authority is revoked. LP is NOT permanently locked here.
    pub fn simulate_breakout(&mut self, pool_tokens: u64, pool_sol: u64) -> SimPool {
        let mint = self.mint();
        let buyer = self.buyer.insecure_clone();
        let cfg_key = Address::new_unique();
        let cfg = crate::damm::install_config(&mut self.svm, cfg_key, None);
        let wsol = self.ensure_wsol(&buyer, pool_sol);
        let sqrt = crate::damm::sqrt_price_from_amounts(pool_tokens, pool_sol);
        let liq = crate::damm::max_liquidity(&cfg, sqrt, pool_tokens, pool_sol);
        let nft = Keypair::new();
        let auth = hc::breakout_auth_pda(&self.launch_key());
        let ix = crate::damm::ix_initialize_pool(
            &buyer.pubkey(), &auth, &nft.pubkey(), &cfg_key, &mint, &hc::WSOL,
            &hc::ata(&buyer.pubkey(), &mint), &wsol, liq, sqrt,
        );
        ok(send(&mut self.svm, &buyer, &[ix], &[&nft]), "simulated breakout: DAMM initialize_pool");
        let pool = crate::damm::pool_pda(&cfg_key, &mint, &hc::WSOL);
        let sim = SimPool {
            config: cfg_key,
            pool,
            position: crate::damm::position_pda(&nft.pubkey()),
            nft_mint: nft.pubkey(),
            a_vault: crate::damm::vault_pda(&mint, &pool),
            b_vault: crate::damm::vault_pda(&hc::WSOL, &pool),
        };

        let lk = self.launch_key();
        let mut acc = self.svm.get_account(&lk).unwrap();
        let mut l = Launch::decode(&acc.data);
        assert_eq!(l.state, hc::state::CURVE_FULL);
        acc.lamports -= l.breakout_sweep_sol();
        l.state = hc::state::BREAKOUT;
        l.dex_pool = sim.pool.to_bytes();
        l.dex_position = sim.position.to_bytes();
        l.position_nft_mint = sim.nft_mint.to_bytes();
        l.encode_into(&mut acc.data);
        self.svm.set_account(lk, acc).unwrap();

        let mut m = self.svm.get_account(&mint).unwrap();
        let mut mi = spl_token_interface::state::Mint::unpack_from_slice(&m.data).unwrap();
        mi.mint_authority = solana_program_option::COption::None;
        mi.pack_into_slice(&mut m.data);
        self.svm.set_account(mint, m).unwrap();
        sim
    }
}

impl World {
    pub fn breakout_ix(&self, nft: &Keypair, liq: u128, sqrt: u128) -> (Instruction, hc::DammBreakoutAccounts) {
        let d = crate::damm::accounts_for(&self.damm_config, &self.mint(), &nft.pubkey());
        (hc::ix_breakout(&self.payer.pubkey(), &self.mint(), &d, liq, sqrt), d)
    }
}

/// Unique account keys of an instruction.
pub fn tx_keys(ix: &Instruction) -> Vec<Address> {
    let mut k: Vec<Address> = ix.accounts.iter().map(|m| m.pubkey).collect();
    k.sort();
    k.dedup();
    k
}
