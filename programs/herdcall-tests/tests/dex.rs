//! Breakout into Meteora DAMM v2, against the real program binary dumped from mainnet.
mod common;

use anchor_lang::prelude::Pubkey;
use common::*;
use herdcall::state::*;
use solana_instruction::{AccountMeta, Instruction};
use solana_keypair::Keypair;
use solana_signer::Signer;

fn expect_err<T>(r: Result<T, String>, code: &str) {
    match r {
        Ok(_) => panic!("expected {code}, transaction succeeded"),
        Err(e) => assert!(e.contains(code), "expected {code}, got:\n{e}"),
    }
}

/// A launch whose curve is sold out (BreakingOut), and the wallet that bought it.
fn breaking_out(env: &mut Env) -> (LaunchKeys, Keypair) {
    let k = env.create_launch(true, 60).unwrap();
    let u = env.user(400);
    env.deposit(&k, &u, SOL).unwrap();
    env.set_time(env.launch(&k).close_at);
    env.settle(&k, &u).unwrap();
    env.buy_exact_out(&k, &u, u64::MAX, u64::MAX, None).unwrap();
    assert_eq!(env.launch(&k).phase, Phase::BreakingOut);
    (k, u)
}

fn env() -> Env {
    let mut env = Env::new();
    env.initialize(example_params()).unwrap();
    env
}

fn meta(k: Pubkey, writable: bool, signer: bool) -> AccountMeta {
    AccountMeta { pubkey: addr(&k), is_signer: signer, is_writable: writable }
}

fn disc(name: &str) -> Vec<u8> {
    use sha2::{Digest, Sha256};
    Sha256::digest(format!("global:{name}").as_bytes())[..8].to_vec()
}

/// Associated token account (idempotent create).
fn create_ata(payer: &Pubkey, owner: &Pubkey, mint: &Pubkey) -> Instruction {
    Instruction {
        program_id: addr(&anchor_spl::associated_token::ID),
        accounts: vec![
            meta(*payer, true, true),
            meta(ata(owner, mint), true, false),
            meta(*owner, false, false),
            meta(*mint, false, false),
            meta(anchor_lang::system_program::ID, false, false),
            meta(anchor_spl::token::ID, false, false),
        ],
        data: vec![1],
    }
}

/// A wallet with a funded wrapped SOL account.
fn wsol_wallet(env: &mut Env, sol: u64) -> Keypair {
    let w = env.user(sol + 3);
    let wp = pk(&w.pubkey());
    let ix = create_ata(&wp, &wp, &WSOL);
    env.send(ix, &[&w]).unwrap();
    let wsol = ata(&wp, &WSOL);
    let mut tx = vec![2u8, 0, 0, 0];
    tx.extend_from_slice(&(sol * SOL).to_le_bytes());
    let fund = Instruction {
        program_id: addr(&anchor_lang::system_program::ID),
        accounts: vec![meta(wp, true, true), meta(wsol, true, false)],
        data: tx,
    };
    env.send(fund, &[&w]).unwrap();
    let sync = Instruction { program_id: addr(&anchor_spl::token::ID), accounts: vec![meta(wsol, true, false)], data: vec![17] };
    env.send(sync, &[&w]).unwrap();
    w
}

#[test]
fn breakout_cost_and_pool_after_it() {
    let mut env = env();
    let (k, _) = breaking_out(&mut env);
    let keeper = env.user(1);
    env.breakout(&k, &keeper).unwrap();
    let d = dex_keys(&k);
    let rent = env.lamports(&d.pool) + env.lamports(&d.position) + env.lamports(&d.position_nft_mint)
        + env.lamports(&d.position_nft_account) + env.lamports(&d.token_vault) + env.rent(&d.wsol_vault);
    println!("REPORT pool rent {rent} lamports, migration fee {}", example_params().migration_fee);
    assert!(rent < example_params().migration_fee);

    // The pool trades: 1 SOL in buys launch tokens at about the curve's final price.
    let t = wsol_wallet(&mut env, 2);
    let tp = pk(&t.pubkey());
    let ix = create_ata(&tp, &tp, &k.mint);
    env.send(ix, &[&t]).unwrap();
    let l = env.launch(&k);
    let mut data = disc("swap");
    data.extend_from_slice(&SOL.to_le_bytes());
    data.extend_from_slice(&0u64.to_le_bytes());
    let swap = Instruction {
        program_id: addr(&DAMM),
        accounts: vec![
            meta(herdcall::damm::POOL_AUTHORITY, false, false),
            meta(d.pool, true, false),
            meta(ata(&tp, &WSOL), true, false),
            meta(ata(&tp, &k.mint), true, false),
            meta(d.token_vault, true, false),
            meta(d.wsol_vault, true, false),
            meta(k.mint, false, false),
            meta(WSOL, false, false),
            meta(tp, false, true),
            meta(anchor_spl::token::ID, false, false),
            meta(anchor_spl::token::ID, false, false),
            meta(DAMM, false, false), // no referral account
            meta(damm_pda(&[b"__event_authority"]), false, false),
            meta(DAMM, false, false),
        ],
        data,
    };
    env.send(swap, &[&t]).unwrap();
    let got = env.token_balance(&ata(&tp, &k.mint));
    // Curve's final price: v / t lamports per unit; 1% pool fee, small price impact.
    let at_curve = (SOL as u128 * l.t as u128 / l.v as u128) as u64;
    assert!(got < at_curve && got > at_curve / 100 * 97, "got {got}, curve price would give {at_curve}");
}

#[test]
fn nobody_else_can_create_the_canonical_pool() {
    let mut env = env();
    let k = env.create_launch(true, 60).unwrap();
    let u = env.user(400);
    env.deposit(&k, &u, SOL).unwrap();
    env.set_time(env.launch(&k).close_at);
    env.settle(&k, &u).unwrap();
    // The attacker buys a few tokens on the curve and holds wrapped SOL.
    let a = wsol_wallet(&mut env, 5);
    let ap = pk(&a.pubkey());
    env.buy_exact_in(&k, &a, SOL, 1, None).unwrap();
    let d = dex_keys(&k);
    let nft = Keypair::new();
    let nft_key = pk(&nft.pubkey());
    let mut data = disc("initialize_pool");
    data.extend_from_slice(&(1u128 << 64).to_le_bytes());
    data.extend_from_slice(&herdcall_core::dex::sqrt_price_ceil(1, 1000).unwrap().to_le_bytes());
    data.push(0);
    let attack = Instruction {
        program_id: addr(&DAMM),
        accounts: vec![
            meta(ap, false, false),
            meta(nft_key, true, true),
            meta(damm_pda(&[b"position_nft_account", nft_key.as_ref()]), true, false),
            meta(ap, true, true),
            meta(DEX_CONFIG, false, false),
            meta(herdcall::damm::POOL_AUTHORITY, false, false),
            meta(d.pool, true, false),
            meta(damm_pda(&[b"position", nft_key.as_ref()]), true, false),
            meta(k.mint, false, false),
            meta(WSOL, false, false),
            meta(d.token_vault, true, false),
            meta(d.wsol_vault, true, false),
            meta(ata(&ap, &k.mint), true, false),
            meta(ata(&ap, &WSOL), true, false),
            meta(anchor_spl::token::ID, false, false),
            meta(anchor_spl::token::ID, false, false),
            meta(anchor_spl::token_2022::ID, false, false),
            meta(anchor_lang::system_program::ID, false, false),
            meta(damm_pda(&[b"__event_authority"]), false, false),
            meta(DAMM, false, false),
        ],
        data,
    };
    // The config is private to the HerdCall migrator: DAMM refuses anyone else.
    expect_err(env.send(attack.clone(), &[&a, &nft]), "InvalidAuthorityToCreateThePool");

    // With a public config the same attack would work and block the breakout for good: that is why mainnet
    // breakouts only accept the private config.
    env.install_dex_config(Pubkey::default());
    env.send(attack, &[&a, &nft]).unwrap();
    env.install_dex_config(migrator());
    env.buy_exact_out(&k, &u, u64::MAX, u64::MAX, None).unwrap();
    let keeper = env.user(1);
    assert!(env.breakout(&k, &keeper).is_err(), "pool already exists");
}

#[test]
fn only_a_private_constant_fee_config_is_accepted() {
    let mut env = env();
    let (k, _) = breaking_out(&mut env);
    let keeper = env.user(1);
    let fixture = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../fixtures/damm_config_1pct.bin")).unwrap();
    let private = |mut d: Vec<u8>| {
        d[40..72].copy_from_slice(migrator().as_ref());
        d
    };
    let mut cases: Vec<(&str, Vec<u8>)> = vec![("public config", fixture.clone())];
    let mut d = private(fixture.clone());
    d[72..80].copy_from_slice(&30_000_000u64.to_le_bytes()); // 3% fee
    cases.push(("fee above 2%", d));
    let mut d = private(fixture.clone());
    d[8 + 193] = 0; // fees in both tokens
    cases.push(("fees not in SOL", d));
    let mut d = private(fixture.clone());
    d[8 + 194] = 1; // dynamic config
    cases.push(("dynamic config", d));
    let mut d = private(fixture.clone());
    d[8 + 208..8 + 224].copy_from_slice(&(herdcall_core::dex::MIN_SQRT_PRICE * 2).to_le_bytes());
    cases.push(("narrow range", d));
    let mut d = private(fixture.clone());
    d[0] ^= 1;
    cases.push(("not a config", d));
    for (what, bytes) in cases {
        env.set_dex_config_bytes(bytes);
        let r = env.breakout(&k, &keeper);
        assert!(r.as_ref().unwrap_err().contains("InvalidDexConfig"), "{what}: {r:?}");
    }
    env.install_dex_config(migrator());
    env.breakout(&k, &keeper).unwrap();
}

#[test]
fn breakout_accounts_cannot_be_swapped() {
    let mut env = env();
    let (k, _) = breaking_out(&mut env);
    let keeper = env.user(1);
    let kp = pk(&keeper.pubkey());
    let fake = Pubkey::new_unique();
    let cases: Vec<(&str, herdcall::accounts::Breakout, &str)> = vec![
        ("fake DAMM program", herdcall::accounts::Breakout { dex_program: anchor_spl::token::ID, ..env.breakout_accounts(&k, &kp) }, "ConstraintAddress"),
        ("other config", herdcall::accounts::Breakout { dex_config: fake, ..env.breakout_accounts(&k, &kp) }, "InvalidDexConfig"),
        ("other pool authority", herdcall::accounts::Breakout { pool_authority: fake, ..env.breakout_accounts(&k, &kp) }, "ConstraintAddress"),
        ("other NFT mint", herdcall::accounts::Breakout { position_nft_mint: fake, ..env.breakout_accounts(&k, &kp) }, "ConstraintSeeds"),
        ("other migrator", herdcall::accounts::Breakout { migrator: fake, ..env.breakout_accounts(&k, &kp) }, "ConstraintSeeds"),
        ("other protocol vault", herdcall::accounts::Breakout { protocol: k.creator_fees, ..env.breakout_accounts(&k, &kp) }, "ConstraintSeeds"),
    ];
    for (what, accounts, code) in cases {
        let r = env.breakout_with(&keeper, accounts);
        assert!(r.as_ref().unwrap_err().contains(code), "{what}: expected {code}, got {r:?}");
    }
    env.breakout(&k, &keeper).unwrap();
}

#[test]
fn admin_can_point_future_breakouts_to_another_config() {
    let mut env = env();
    let (k, _) = breaking_out(&mut env);
    let other = Pubkey::new_unique();
    let stranger = env.user(1);
    let set = |env: &mut Env, signer: &Keypair, cfg: Pubkey| {
        let metas = anchor_lang::ToAccountMetas::to_account_metas(
            &herdcall::accounts::AdminOnly { admin: pk(&signer.pubkey()), config: pda(&[CONFIG_SEED]) },
            None,
        );
        let ix = env.ix(metas, anchor_lang::InstructionData::data(&herdcall::instruction::SetDexConfig { dex_config: cfg }));
        env.send(ix, &[signer])
    };
    expect_err(set(&mut env, &stranger, other), "ConstraintHasOne");
    let admin = env.admin.insecure_clone();
    set(&mut env, &admin, other).unwrap();
    let keeper = env.user(1);
    // The breakout now expects the new config; passing the old one fails, the new one must still pass the checks.
    expect_err(env.breakout(&k, &keeper), "InvalidDexConfig");
    set(&mut env, &admin, DEX_CONFIG).unwrap();
    env.breakout(&k, &keeper).unwrap();
}
