//! Fees of the locked DAMM v2 position: claimed by the launch PDA and split as CURVE_SPEC §5.8 says.
mod common;

use common::*;
use herdcall::state::*;
use herdcall_core::fees::{split_lp_fees, Kind};
use herdcall_core::BPS;
use solana_keypair::Keypair;
use solana_signer::Signer;

fn expect_err<T>(r: Result<T, String>, code: &str) {
    match r {
        Ok(_) => panic!("expected {code}, transaction succeeded"),
        Err(e) => assert!(e.contains(code), "expected {code}, got:\n{e}"),
    }
}

/// A launch that broke out into DAMM v2.
fn migrated(env: &mut Env, bonded: bool) -> LaunchKeys {
    let k = env.create_launch(bonded, 60).unwrap();
    let u = env.user(400);
    env.deposit(&k, &u, SOL).unwrap();
    env.set_time(env.launch(&k).close_at);
    env.settle(&k, &u).unwrap();
    env.buy_exact_out(&k, &u, u64::MAX, u64::MAX, None).unwrap();
    let keeper = env.user(1);
    env.breakout(&k, &keeper).unwrap();
    assert_eq!(env.launch(&k).phase, Phase::Migrated);
    k
}

fn env() -> Env {
    let mut env = Env::new();
    env.initialize(example_params()).unwrap();
    env
}

struct Balances {
    sol_vault: u64,
    creator: u64,
    rekt_back: u64,
    protocol: u64,
}

fn balances(env: &Env, k: &LaunchKeys) -> Balances {
    Balances {
        sol_vault: env.lamports(&k.sol_vault),
        creator: env.lamports(&k.creator_fees),
        rekt_back: env.lamports(&pda(&[REKT_BACK_SEED])),
        protocol: env.lamports(&pda(&[PROTOCOL_SEED])),
    }
}

/// Trades on the pool in both directions so the position earns fees in SOL.
fn trade(env: &mut Env, k: &LaunchKeys) -> Keypair {
    let t = env.dex_trader(20, &k.mint);
    env.dex_swap(k, &t, 5 * SOL, true).unwrap();
    let tokens = env.token_balance(&ata(&pk(&t.pubkey()), &k.mint));
    env.dex_swap(k, &t, tokens / 2, false).unwrap();
    t
}

fn claim_and_check(env: &mut Env, k: &LaunchKeys, kind: Kind) -> u64 {
    let before = balances(env, k);
    let pending_before = env.launch(k).lp_reinvest_pending;
    let claimed_before = env.launch(k).lp_fees_claimed;
    let caller = env.user(1);
    let caller_before = env.lamports(&pk(&caller.pubkey()));
    env.claim_lp_fees(k, &caller).unwrap();
    let after = balances(env, k);
    let l = env.launch(k);
    let fee = l.lp_fees_claimed - claimed_before;
    assert!(fee > 0);
    let parts = split_lp_fees(fee as u128, kind, BPS).unwrap();
    assert_eq!((after.creator - before.creator) as u128, parts.creator, "creator");
    assert_eq!((after.rekt_back - before.rekt_back) as u128, parts.rekt_back, "rekt-back");
    assert_eq!((after.protocol - before.protocol) as u128, parts.protocol, "protocol");
    // The "back to liquidity" share waits in the SOL vault.
    assert_eq!((l.lp_reinvest_pending - pending_before) as u128, parts.liquidity, "liquidity share recorded");
    assert_eq!((after.sol_vault - before.sol_vault) as u128, parts.liquidity, "liquidity share held in the vault");
    assert!(after.sol_vault >= env.rent(&k.sol_vault) + l.lp_reinvest_pending);
    // The caller only pays the transaction fee: the temporary account's rent comes back.
    assert_eq!(caller_before - env.lamports(&pk(&caller.pubkey())), 5_000, "caller pays only the tx fee");
    assert!(env.data(&pda(&[LP_FEE_WSOL_SEED, k.launch.as_ref()])).is_empty(), "temporary account closed");
    fee
}

#[test]
fn bonded_fees_are_claimed_and_split() {
    let mut env = env();
    let k = migrated(&mut env, true);
    let d = dex_keys(&k);
    let locked_before = u128_at(&env.data(&d.position), 184);
    let tokens_before = env.token_balance(&k.token_vault);
    trade(&mut env, &k);
    let fee = claim_and_check(&mut env, &k, Kind::Bonded);
    // 1% pool fee on ~5 SOL in and ~2.5 SOL out, of which Meteora keeps its protocol share (20%).
    assert!(fee > SOL / 20 && fee < SOL / 10, "fee {fee}");
    // The position stays locked, its NFT stays with the launch PDA, and no token fee reached the vault.
    assert_eq!(u128_at(&env.data(&d.position), 184), locked_before);
    assert_eq!(&env.data(&d.position_nft_account)[32..64], k.launch.as_ref());
    assert_eq!(env.token_balance(&k.token_vault), tokens_before);
    // The creator can withdraw the fee share.
    let creator = k.creator.insecure_clone();
    let before = env.lamports(&pk(&creator.pubkey()));
    env.withdraw_creator_fees(&k, &creator).unwrap();
    assert!(env.lamports(&pk(&creator.pubkey())) > before);
}

#[test]
fn unbonded_fees_use_the_unbonded_split() {
    let mut env = env();
    let k = migrated(&mut env, false);
    trade(&mut env, &k);
    claim_and_check(&mut env, &k, Kind::Unbonded);
}

#[test]
fn claims_add_up_and_nothing_is_claimed_twice() {
    let mut env = env();
    let k = migrated(&mut env, true);
    let caller = env.user(1);
    expect_err(env.claim_lp_fees(&k, &caller), "NoFees");
    trade(&mut env, &k);
    let first = claim_and_check(&mut env, &k, Kind::Bonded);
    expect_err(env.claim_lp_fees(&k, &caller), "NoFees");
    trade(&mut env, &k);
    let second = claim_and_check(&mut env, &k, Kind::Bonded);
    let l = env.launch(&k);
    assert_eq!(l.lp_fees_claimed, first + second);
    let parts = |f: u64| split_lp_fees(f as u128, Kind::Bonded, BPS).unwrap().liquidity as u64;
    assert_eq!(l.lp_reinvest_pending, parts(first) + parts(second));
}

#[test]
fn only_a_migrated_launch_and_its_own_position() {
    let mut env = env();
    // Before breakout there is no position.
    let early = env.create_launch(true, 60).unwrap();
    let caller = env.user(1);
    let accounts = env.claim_lp_fees_accounts(&early, &pk(&caller.pubkey()));
    // Its recorded pool is still empty, so any pool is the wrong one.
    expect_err(env.claim_lp_fees_with(&caller, accounts), "WrongAccount");
    let a = migrated(&mut env, true);
    let b = migrated(&mut env, true);
    trade(&mut env, &a);
    trade(&mut env, &b);
    let base = || env.claim_lp_fees_accounts(&a, &pk(&caller.pubkey()));
    let db = dex_keys(&b);
    let cases: Vec<(&str, herdcall::accounts::ClaimLpFees, &str)> = vec![
        ("other pool", herdcall::accounts::ClaimLpFees { pool: db.pool, ..base() }, "WrongAccount"),
        ("other position", herdcall::accounts::ClaimLpFees { position: db.position, ..base() }, "WrongAccount"),
        ("other creator vault", herdcall::accounts::ClaimLpFees { creator_fees: b.creator_fees, ..base() }, "ConstraintSeeds"),
        ("fake DAMM program", herdcall::accounts::ClaimLpFees { dex_program: anchor_spl::token::ID, ..base() }, "ConstraintAddress"),
        // DAMM checks the NFT account belongs to the position.
        ("other NFT account", herdcall::accounts::ClaimLpFees { position_nft_account: db.position_nft_account, ..base() }, "ConstraintRaw"),
    ];
    for (what, accounts, code) in cases {
        let r = env.claim_lp_fees_with(&caller, accounts);
        assert!(r.as_ref().unwrap_err().contains(code), "{what}: expected {code}, got {r:?}");
    }
    claim_and_check(&mut env, &a, Kind::Bonded);
}
