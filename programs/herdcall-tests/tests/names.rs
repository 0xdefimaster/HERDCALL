//! Ticker reservation: held while the launch is active, permanent at breakout, released after a cancellation or a
//! timeout without breakout.
mod common;

use common::*;
use herdcall::state::*;
use solana_keypair::Keypair;
use solana_signer::Signer;

fn expect_err<T>(r: Result<T, String>, code: &str) {
    match r {
        Ok(_) => panic!("expected {code}, transaction succeeded"),
        Err(e) => assert!(e.contains(code), "expected {code}, got:\n{e}"),
    }
}

fn env() -> Env {
    let mut env = Env::new();
    env.initialize(example_params()).unwrap();
    env
}

/// Lobby with one deposit, settled: the token is on the curve.
fn on_curve(env: &mut Env, ticker: &str) -> (LaunchKeys, Keypair) {
    let k = env.create_launch_named(true, 60, ticker).unwrap();
    let u = env.user(300);
    env.deposit(&k, &u, SOL).unwrap();
    env.set_time(env.launch(&k).close_at);
    env.settle(&k, &u).unwrap();
    (k, u)
}

fn break_out(env: &mut Env, k: &LaunchKeys, u: &Keypair) {
    env.buy_exact_out(k, u, u64::MAX, u64::MAX, None).unwrap();
    assert_eq!(env.launch(k).phase, Phase::BreakingOut);
    env.breakout(k, u).unwrap();
}

#[test]
fn only_canonical_tickers() {
    let mut env = env();
    for bad in ["", "pepe", "Pepe", "PE PE", "PEPE!", "ABCDEFGHIJK", "$PEPE"] {
        expect_err(env.create_launch_named(true, 60, bad), "InvalidTicker");
    }
    let k = env.create_launch_named(true, 60, "PEPE2").unwrap();
    let l = env.launch(&k);
    assert_eq!(l.ticker, "PEPE2");
    assert_eq!(l.name_expires_at, l.close_at + example_params().name_hold_secs);
    let rec = env.name_record("PEPE2").unwrap();
    assert_eq!((rec.launch, rec.permanent), (k.launch, false));
}

#[test]
fn an_active_launch_holds_its_ticker() {
    let mut env = env();
    let k = env.create_launch_named(true, 60, "PEPE").unwrap();
    expect_err(env.create_launch_named(true, 60, "PEPE"), "already in use");
    let anyone = env.user(1);
    // Lobby open, Lobby closed inside the settlement window, and on the curve before the hold time: not releasable.
    expect_err(env.release_name(&k, &anyone), "NameNotReleasable");
    let l = env.launch(&k);
    env.set_time(l.settle_deadline - 1);
    expect_err(env.release_name(&k, &anyone), "NameNotReleasable");
    let (k2, _) = on_curve(&mut env, "WIF");
    env.set_time(env.launch(&k2).name_expires_at - 1);
    expect_err(env.release_name(&k2, &anyone), "NameNotReleasable");
    expect_err(env.create_launch_named(false, 60, "WIF"), "already in use");
}

#[test]
fn a_cancelled_launch_frees_its_ticker() {
    let mut env = env();
    let k = env.create_launch_named(true, 60, "PEPE").unwrap();
    let u = env.user(5);
    env.deposit(&k, &u, SOL).unwrap();
    env.set_time(env.launch(&k).settle_deadline);
    // Nobody has refunded yet: the deadline alone is enough.
    let anyone = env.user(1);
    let creator = pk(&k.creator.pubkey());
    let rent = env.lamports(&k.name);
    let before = env.lamports(&creator);
    env.release_name(&k, &anyone).unwrap();
    assert!(env.name_record("PEPE").is_none());
    assert_eq!(env.lamports(&creator) - before, rent, "record rent goes back to the creator");
    // Refunds still work and the ticker can be launched again.
    env.refund(&k, &u).unwrap();
    let k2 = env.create_launch_named(true, 60, "PEPE").unwrap();
    assert_eq!(env.name_record("PEPE").unwrap().launch, k2.launch);
    // The old launch cannot release the new holder's record.
    expect_err(env.release_name(&k, &anyone), "WrongAccount");
}

#[test]
fn a_timed_out_curve_frees_its_ticker_and_can_still_break_out() {
    let mut env = env();
    let (old, u) = on_curve(&mut env, "PEPE");
    env.set_time(env.launch(&old).name_expires_at);
    let anyone = env.user(1);
    env.release_name(&old, &anyone).unwrap();
    expect_err(env.release_name(&old, &anyone), "AccountNotInitialized");
    let new = env.create_launch_named(true, 60, "PEPE").unwrap();

    // The old token keeps trading and breaks out; its breakout leaves the new holder's record alone.
    break_out(&mut env, &old, &u);
    assert_eq!(env.launch(&old).phase, Phase::Migrating);
    let rec = env.name_record("PEPE").unwrap();
    assert_eq!((rec.launch, rec.permanent), (new.launch, false));

    // A breakout also works when the record was released and nobody took the ticker again.
    let (lonely, u2) = on_curve(&mut env, "BONK");
    env.set_time(env.launch(&lonely).name_expires_at);
    env.release_name(&lonely, &anyone).unwrap();
    break_out(&mut env, &lonely, &u2);
    assert!(env.name_record("BONK").is_none());
}

#[test]
fn breakout_makes_the_ticker_permanent() {
    let mut env = env();
    let (k, u) = on_curve(&mut env, "HERD");
    env.buy_exact_out(&k, &u, u64::MAX, u64::MAX, None).unwrap();
    // Breaking out but not migrated yet: past the hold time the ticker is still not releasable.
    env.set_time(env.launch(&k).name_expires_at + 1);
    let anyone = env.user(1);
    expect_err(env.release_name(&k, &anyone), "NameNotReleasable");
    env.breakout(&k, &u).unwrap();
    let rec = env.name_record("HERD").unwrap();
    assert_eq!((rec.launch, rec.permanent), (k.launch, true));
    env.set_time(env.now + 365 * 24 * 3600);
    expect_err(env.release_name(&k, &anyone), "NameNotReleasable");
    expect_err(env.create_launch_named(true, 60, "HERD"), "already in use");
}

#[test]
fn release_needs_the_matching_launch_and_creator() {
    let mut env = env();
    let a = env.create_launch_named(true, 60, "AAA").unwrap();
    let b = env.create_launch_named(true, 60, "BBB").unwrap();
    env.set_time(env.launch(&a).settle_deadline);
    let anyone = env.user(1);
    // A's record with B's launch: the seeds come from B's ticker, so A's record does not match.
    let metas = anchor_lang::ToAccountMetas::to_account_metas(
        &herdcall::accounts::ReleaseName { name: a.name, launch: b.launch, creator: pk(&b.creator.pubkey()) },
        None,
    );
    let ix = env.ix(metas, anchor_lang::InstructionData::data(&herdcall::instruction::ReleaseName {}));
    expect_err(env.send(ix, &[&anyone]), "ConstraintSeeds");
    // Rent to someone other than the creator.
    let metas = anchor_lang::ToAccountMetas::to_account_metas(
        &herdcall::accounts::ReleaseName { name: a.name, launch: a.launch, creator: pk(&anyone.pubkey()) },
        None,
    );
    let ix = env.ix(metas, anchor_lang::InstructionData::data(&herdcall::instruction::ReleaseName {}));
    expect_err(env.send(ix, &[&anyone]), "WrongAccount");
    env.release_name(&a, &anyone).unwrap();
}
