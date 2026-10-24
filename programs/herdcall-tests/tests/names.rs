//! Name and ticker reservation: both unique, held from creation, permanent at breakout, released (bond back) only
//! when the launch is cancelled. A token on the curve keeps them with no time limit.
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
fn on_curve(env: &mut Env, ticker: &str, name: &str) -> (LaunchKeys, Keypair) {
    let k = env.create_launch_full(true, 60, ticker, name).unwrap();
    let u = env.user(300);
    env.deposit(&k, &u, SOL).unwrap();
    env.set_time(env.launch(&k).close_at);
    env.settle(&k, &u).unwrap();
    (k, u)
}

fn held_by(env: &Env, k: &LaunchKeys, permanent: bool) {
    for key in [k.ticker_record, k.name_record] {
        let r = env.record(&key).expect("record exists");
        assert_eq!((r.launch, r.permanent), (k.launch, permanent));
    }
}

fn released(env: &Env, k: &LaunchKeys) {
    assert!(env.record(&k.ticker_record).is_none() && env.record(&k.name_record).is_none());
}

#[test]
fn only_valid_tickers_and_names() {
    let mut env = env();
    for bad in ["", "pepe", "Pepe", "PE PE", "PEPE!", "ABCDEFGHIJK", "$PEPE"] {
        expect_err(env.create_launch_full(true, 60, bad, "Some Coin"), "InvalidTicker");
    }
    for bad in ["", "   ", "Pepe\nCoin", &"x".repeat(33)] {
        expect_err(env.create_launch_full(true, 60, "PEPE", bad), "InvalidName");
    }
    // A hash that is not the hash of the name: rejected even though the record address is valid.
    let creator = env.user(10);
    let mint = Keypair::new();
    let m = pk(&mint.pubkey());
    let launch = pda(&[LAUNCH_SEED, m.as_ref()]);
    let metas = anchor_lang::ToAccountMetas::to_account_metas(
        &herdcall::accounts::CreateLaunch {
            creator: pk(&creator.pubkey()),
            config: pda(&[CONFIG_SEED]),
            mint: m,
            launch,
            token_vault: ata(&launch, &m),
            sol_vault: pda(&[SOL_VAULT_SEED, launch.as_ref()]),
            creator_fees: pda(&[CREATOR_FEES_SEED, launch.as_ref()]),
            creator_escrow: pda(&[CREATOR_ESCROW_SEED, launch.as_ref()]),
            ticker_record: ticker_pda("PEPE"),
            name_record: name_pda("Other Coin"),
            token_program: anchor_spl::token::ID,
            associated_token_program: anchor_spl::associated_token::ID,
            system_program: anchor_lang::system_program::ID,
        },
        None,
    );
    let data = anchor_lang::InstructionData::data(&herdcall::instruction::CreateLaunch {
        bonded: true,
        lobby_secs: 60,
        ticker: "PEPE".into(),
        name: "Pepe Coin".into(),
        name_hash: name_hash("Other Coin"),
    });
    let ix = env.ix(metas, data);
    expect_err(env.send(ix, &[&creator, &mint]), "NameHashMismatch");

    let k = env.create_launch_full(true, 60, "PEPE2", "  Pepe Two ").unwrap();
    let l = env.launch(&k);
    assert_eq!((l.ticker.as_str(), l.name.as_str()), ("PEPE2", "  Pepe Two "));
    held_by(&env, &k, false);
}

#[test]
fn name_and_ticker_are_both_unique() {
    let mut env = env();
    let k = env.create_launch_full(true, 60, "PEPE", "Pepe Coin").unwrap();
    // Same ticker, other name.
    expect_err(env.create_launch_full(true, 60, "PEPE", "Frog Coin"), "already in use");
    // Same name up to case and surrounding whitespace, other ticker.
    for same in ["Pepe Coin", "PEPE COIN", "pepe coin", "  pEpE cOiN\t"] {
        expect_err(env.create_launch_full(true, 60, "FROG", same), "already in use");
    }
    // Inner spaces count; a different name and ticker is fine.
    env.create_launch_full(true, 60, "FROG", "Pepe  Coin").unwrap();
    env.create_launch_full(true, 60, "PEPE3", "PepeCoin").unwrap();
    held_by(&env, &k, false);
}

#[test]
fn creating_a_launch_locks_the_bond() {
    let mut env = env();
    let k = env.create_launch_full(true, 60, "PEPE", "Pepe Coin").unwrap();
    let bond = example_params().name_bond;
    assert_eq!(env.lamports(&k.ticker_record), env.rent(&k.ticker_record) + bond);
    assert_eq!(env.lamports(&k.name_record), env.rent(&k.name_record));
    assert_eq!(env.launch(&k).name_bond, bond);
}

#[test]
fn a_launch_that_can_break_out_keeps_its_name() {
    let mut env = env();
    let anyone = env.user(1);
    let k = env.create_launch_full(true, 60, "PEPE", "Pepe Coin").unwrap();
    expect_err(env.release_name(&k, &anyone), "NameNotReleasable");
    env.set_time(env.launch(&k).settle_deadline - 1);
    expect_err(env.release_name(&k, &anyone), "NameNotReleasable");

    // On the curve there is no time limit: a year later buys are open and the name is still held.
    let (c, u) = on_curve(&mut env, "WIF", "Dog Hat");
    expect_err(env.release_name(&c, &anyone), "NameNotReleasable");
    env.set_time(env.now + 365 * 24 * 3600);
    expect_err(env.release_name(&c, &anyone), "NameNotReleasable");
    env.buy_exact_in(&c, &u, SOL, 1, None).unwrap();
    held_by(&env, &c, false);
    expect_err(env.create_launch_full(true, 60, "WIF", "Other"), "already in use");
}

#[test]
fn an_unsettled_lobby_frees_name_and_ticker_and_returns_the_bond() {
    let mut env = env();
    let k = env.create_launch_full(true, 60, "PEPE", "Pepe Coin").unwrap();
    let u = env.user(5);
    env.deposit(&k, &u, SOL).unwrap();
    env.set_time(env.launch(&k).settle_deadline);
    // It can never be settled now; nobody has to refund first.
    let anyone = env.user(1);
    let creator = pk(&k.creator.pubkey());
    let held = env.lamports(&k.ticker_record) + env.lamports(&k.name_record);
    let before = env.lamports(&creator);
    env.release_name(&k, &anyone).unwrap();
    released(&env, &k);
    assert_eq!(env.lamports(&creator) - before, held, "rent of both records and the bond go back to the creator");
    assert_eq!(env.launch(&k).name_bond, 0);
    expect_err(env.settle(&k, &anyone), "SettleDeadlinePassed");
    env.refund(&k, &u).unwrap();
    let k2 = env.create_launch_full(true, 60, "PEPE", "pepe coin").unwrap();
    held_by(&env, &k2, false);
}

#[test]
fn a_lobby_below_the_minimum_frees_name_and_ticker_at_once() {
    let mut env = env();
    let k = env.create_launch_full(true, 60, "PEPE", "Pepe Coin").unwrap();
    let u = env.user(5);
    env.deposit(&k, &u, example_params().min_total - 1).unwrap();
    env.set_time(env.launch(&k).close_at);
    let anyone = env.user(1);
    // Still in the Lobby: held until settlement decides.
    expect_err(env.release_name(&k, &anyone), "NameNotReleasable");
    env.settle(&k, &anyone).unwrap();
    assert_eq!(env.launch(&k).phase, Phase::Cancelled);

    // Released right after the settlement, long before the settlement deadline.
    assert!(env.now < env.launch(&k).settle_deadline);
    let creator = pk(&k.creator.pubkey());
    let held = env.lamports(&k.ticker_record) + env.lamports(&k.name_record);
    let before = env.lamports(&creator);
    env.release_name(&k, &anyone).unwrap();
    released(&env, &k);
    assert_eq!(env.lamports(&creator) - before, held);
    expect_err(env.release_name(&k, &anyone), "AccountNotInitialized");
    env.refund(&k, &u).unwrap();
    let k2 = env.create_launch_full(true, 60, "PEPE", "PEPE COIN").unwrap();
    held_by(&env, &k2, false);
}

#[test]
fn breakout_makes_name_and_ticker_permanent_and_returns_the_bond() {
    let mut env = env();
    let (k, u) = on_curve(&mut env, "HERD", "Herd Coin");
    env.buy_exact_out(&k, &u, u64::MAX, u64::MAX, None).unwrap();
    assert_eq!(env.launch(&k).phase, Phase::BreakingOut);
    // Reached breakout: the breakout call may come later, and the name stays held meanwhile.
    let anyone = env.user(1);
    expect_err(env.release_name(&k, &anyone), "NameNotReleasable");

    let creator = pk(&k.creator.pubkey());
    let before = env.lamports(&creator);
    env.breakout(&k, &u).unwrap();
    held_by(&env, &k, true);
    assert_eq!(env.lamports(&creator) - before, example_params().name_bond, "bond back to the creator");
    assert_eq!(env.lamports(&k.ticker_record), env.rent(&k.ticker_record));
    assert_eq!(env.launch(&k).name_bond, 0);

    env.set_time(env.now + 365 * 24 * 3600);
    expect_err(env.release_name(&k, &anyone), "NameNotReleasable");
    expect_err(env.create_launch_full(true, 60, "HERD", "Other"), "already in use");
    expect_err(env.create_launch_full(true, 60, "OTHER", "herd coin"), "already in use");
}

#[test]
fn the_creator_can_call_breakout_too() {
    let mut env = env();
    let (k, u) = on_curve(&mut env, "SELF", "Self Made");
    env.buy_exact_out(&k, &u, u64::MAX, u64::MAX, None).unwrap();
    let creator = k.creator.insecure_clone();
    env.breakout(&k, &creator).unwrap();
    held_by(&env, &k, true);
}

#[test]
fn release_needs_the_matching_records_and_creator() {
    let mut env = env();
    let a = env.create_launch_full(true, 60, "AAA", "Aaa").unwrap();
    let b = env.create_launch_full(true, 60, "BBB", "Bbb").unwrap();
    env.set_time(env.launch(&a).settle_deadline);
    let anyone = env.user(1);
    let send = |env: &mut Env, accounts: herdcall::accounts::ReleaseName| {
        let metas = anchor_lang::ToAccountMetas::to_account_metas(&accounts, None);
        let ix = env.ix(metas, anchor_lang::InstructionData::data(&herdcall::instruction::ReleaseName {}));
        env.send(ix, &[&anyone])
    };
    let base = |a: &LaunchKeys| herdcall::accounts::ReleaseName {
        ticker_record: a.ticker_record,
        name_record: a.name_record,
        launch: a.launch,
        creator: pk(&a.creator.pubkey()),
    };
    expect_err(send(&mut env, herdcall::accounts::ReleaseName { ticker_record: b.ticker_record, ..base(&a) }), "ConstraintSeeds");
    expect_err(send(&mut env, herdcall::accounts::ReleaseName { name_record: b.name_record, ..base(&a) }), "ConstraintSeeds");
    expect_err(send(&mut env, herdcall::accounts::ReleaseName { creator: pk(&anyone.pubkey()), ..base(&a) }), "WrongAccount");
    send(&mut env, base(&a)).unwrap();
    released(&env, &a);
}
