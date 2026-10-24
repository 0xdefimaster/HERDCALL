//! Adding the "back to liquidity" share of the LP fees to the locked position (CURVE_SPEC §5.7), against the real
//! DAMM v2 binary. DAMM v2 has no price oracle, so a keeper supplies a price band from its own observations.
mod common;

use common::*;
use herdcall::state::*;
use herdcall_core::dex;
use solana_signer::Signer;

fn expect_err<T>(r: Result<T, String>, code: &str) {
    match r {
        Ok(_) => panic!("expected {code}, transaction succeeded"),
        Err(e) => assert!(e.contains(code), "expected {code}, got:\n{e}"),
    }
}

fn migrated(env: &mut Env) -> LaunchKeys {
    let k = env.create_launch(true, 60).unwrap();
    let u = env.user(400);
    env.deposit(&k, &u, SOL).unwrap();
    env.set_time(env.launch(&k).close_at);
    env.settle(&k, &u).unwrap();
    env.buy_exact_out(&k, &u, u64::MAX, u64::MAX, None).unwrap();
    let keeper = env.user(1);
    env.breakout(&k, &keeper).unwrap();
    k
}

fn env() -> Env {
    let mut env = Env::new();
    env.initialize(example_params()).unwrap();
    env
}

/// Volume on the pool, then a fee claim: about `sol` of buys and the same back in sells.
fn earn_fees(env: &mut Env, k: &LaunchKeys, sol: u64) {
    let t = env.dex_trader(sol + 1, &k.mint);
    env.dex_swap(k, &t, sol * SOL, true).unwrap();
    let tokens = env.token_balance(&ata(&pk(&t.pubkey()), &k.mint));
    env.dex_swap(k, &t, tokens, false).unwrap();
    let caller = env.user(1);
    env.claim_lp_fees(k, &caller).unwrap();
}

/// `sol` SOL waiting to be reinvested (as if many claims had added up).
fn pending(env: &mut Env, k: &LaunchKeys, sol: u64) {
    env.patch_launch(k, |l| l.lp_reinvest_pending = sol * SOL);
    env.add_lamports(&k.sol_vault, sol * SOL);
}

fn pool_sol(env: &Env, k: &LaunchKeys) -> u128 {
    let d = env.data(&dex_keys(k).pool);
    dex::reserves(u128_at(&d, 360), dex::MIN_SQRT_PRICE, u128_at(&d, 456), dex::MAX_SQRT_PRICE).unwrap().0
}

/// Lamports a token amount is worth at √price `sqrt` (Q64.64).
fn value(tokens: u64, sqrt: u128) -> f64 {
    let p = (sqrt as f64 / 2f64.powi(64)).powi(2);
    tokens as f64 * p
}

#[test]
fn pending_share_is_added_and_locked() {
    let mut env = env();
    let k = migrated(&mut env);
    expect_err(env.reinvest_lp(&k), "NothingToReinvest");
    earn_fees(&mut env, &k, 40);
    let before = env.launch(&k);
    assert!(before.lp_reinvest_pending >= dex::REINVEST_MIN_LAMPORTS, "pending {}", before.lp_reinvest_pending);
    let d = dex_keys(&k);
    let pool_l = u128_at(&env.data(&d.pool), 360);
    let sqrt_before = env.pool_sqrt_price(&k);
    let locked_before = u128_at(&env.data(&d.position), 184);
    let supply_before = env.mint_info(&k.mint).1;
    let keeper = pk(&env.admin.pubkey());
    let keeper_before = env.lamports(&keeper);

    env.reinvest_lp(&k).unwrap();
    let l = env.launch(&k);
    let added = l.lp_liquidity - before.lp_liquidity;
    assert!(added > 0);
    let position = env.data(&d.position);
    assert_eq!((u128_at(&position, 152), u128_at(&position, 184)), (0, locked_before + added), "all of it locked");
    assert_eq!(u128_at(&env.data(&d.pool), 360), pool_l + added, "pool liquidity");
    let used = before.lp_reinvest_pending - l.lp_reinvest_pending;
    assert_eq!(l.lp_reinvested, used);
    assert!(used > before.lp_reinvest_pending * 98 / 100, "almost all of it is used: {used}");
    assert!(env.lamports(&k.sol_vault) >= env.rent(&k.sol_vault) + l.lp_reinvest_pending);
    assert_eq!(supply_before - env.mint_info(&k.mint).1, l.burned - before.burned, "leftover tokens burned");
    assert!(dex::price_move_ok(sqrt_before, env.pool_sqrt_price(&k)));
    assert_eq!(keeper_before - env.lamports(&keeper), 5_000, "keeper pays only the tx fee");
    for seed in [REINVEST_WSOL_SEED, REINVEST_TOKEN_SEED] {
        assert!(env.data(&pda(&[seed, k.launch.as_ref()])).is_empty());
    }
    assert_eq!(env.lamports(&migrator()), 0);
    expect_err(env.reinvest_lp(&k), "NothingToReinvest");
}

#[test]
fn one_call_takes_at_most_half_a_percent_of_the_pool() {
    let mut env = env();
    let k = migrated(&mut env);
    pending(&mut env, &k, 10);
    let cap = (pool_sol(&env, &k) * dex::REINVEST_MAX_POOL_BPS / 10_000) as u64;
    env.reinvest_lp(&k).unwrap();
    let first = 10 * SOL - env.launch(&k).lp_reinvest_pending;
    assert!(first <= cap && first > cap * 98 / 100, "used {first}, cap {cap}");
    env.reinvest_lp(&k).unwrap();
    let l = env.launch(&k);
    assert!(10 * SOL - l.lp_reinvest_pending - first <= (pool_sol(&env, &k) * 50 / 10_000) as u64);
    assert!(l.lp_reinvest_pending > 8 * SOL);
}

/// The attack the price limits alone cannot see: the price is pushed up in an earlier transaction. The keeper's band
/// comes from its observations before the push, so the call is rejected and nothing is spent; once the price is back,
/// the same band passes.
#[test]
fn a_price_moved_before_the_call_is_rejected_by_the_keeper_band() {
    let mut env = env();
    let k = migrated(&mut env);
    pending(&mut env, &k, 1);
    let observed = env.pool_sqrt_price(&k);
    let keeper = env.admin.insecure_clone();
    let attacker = env.dex_trader(21, &k.mint);
    env.dex_swap(&k, &attacker, 20 * SOL, true).unwrap();
    let moved = env.pool_sqrt_price(&k);
    println!("REPORT 20 SOL buy moved the price by {:.1}%", ((moved as f64 / observed as f64).powi(2) - 1.0) * 100.0);
    expect_err(env.reinvest_lp_at(&k, &keeper, observed), "PriceOutOfBand");
    assert_eq!(env.launch(&k).lp_reinvest_pending, SOL, "nothing spent");
    // A small push stays inside the band: the call goes through and the loss is bounded by the band.
    let tokens = env.token_balance(&ata(&pk(&attacker.pubkey()), &k.mint));
    env.dex_swap(&k, &attacker, tokens, false).unwrap();
    let observed = env.pool_sqrt_price(&k);
    env.dex_swap(&k, &attacker, SOL / 5, true).unwrap();
    let inside = env.pool_sqrt_price(&k);
    assert!(inside <= observed + observed / 400, "a 0.2 SOL push stays in the band");
    let before = env.launch(&k);
    env.reinvest_lp_at(&k, &keeper, observed).unwrap();
    let l = env.launch(&k);
    let used = (before.lp_reinvest_pending - l.lp_reinvest_pending) as f64;
    let (sol, toks) = dex::reserves(l.lp_liquidity - before.lp_liquidity, dex::MIN_SQRT_PRICE, observed, dex::MAX_SQRT_PRICE).unwrap();
    let loss = used - (sol as f64 + value(toks as u64, observed));
    println!("REPORT in-band push (+{:.2}% price): reinvested {:.4} SOL, loss at the observed price {:.5} SOL ({:.2}%)",
        ((inside as f64 / observed as f64).powi(2) - 1.0) * 100.0, used / 1e9, loss / 1e9, loss / used * 100.0);
    assert!(loss / used < 0.02, "in-band loss stays small");
}

/// Without the keeper for `KEEPER_GRACE_SECS`, anyone may reinvest; then the keeper-only rule is back.
#[test]
fn anyone_may_reinvest_after_the_keeper_has_been_idle() {
    let mut env = env();
    let k = migrated(&mut env);
    pending(&mut env, &k, 1);
    let stranger = env.user(1);
    let p = env.pool_sqrt_price(&k);
    expect_err(env.reinvest_lp_at(&k, &stranger, p), "KeeperOnly");
    env.set_time(env.launch(&k).last_reinvest_at + dex::KEEPER_GRACE_SECS - 1);
    expect_err(env.reinvest_lp_at(&k, &stranger, p), "KeeperOnly");
    env.set_time(env.launch(&k).last_reinvest_at + dex::KEEPER_GRACE_SECS);
    env.reinvest_lp_at(&k, &stranger, p).unwrap();
    assert_eq!(env.launch(&k).last_reinvest_at, env.now);
    let p = env.pool_sqrt_price(&k);
    expect_err(env.reinvest_lp_at(&k, &stranger, p), "KeeperOnly");
    // The admin can hand the job to a dedicated keeper.
    let keeper = env.user(1);
    let admin = env.admin.insecure_clone();
    let metas = anchor_lang::ToAccountMetas::to_account_metas(
        &herdcall::accounts::AdminOnly { admin: pk(&admin.pubkey()), config: pda(&[CONFIG_SEED]) },
        None,
    );
    let ix = env.ix(metas, anchor_lang::InstructionData::data(&herdcall::instruction::SetReinvestKeeper { keeper: pk(&keeper.pubkey()) }));
    env.send(ix, &[&admin]).unwrap();
    env.reinvest_lp_at(&k, &keeper, p).unwrap();
}

/// In the idle fallback the caller picks the band, so it can be a manipulator: it pushes the price, reinvests at the
/// pushed price and sells back. This measures what the protocol loses (what was added, valued at the price before the
/// push, against the SOL spent) and what the attacker makes. The cap bounds the loss; it does not prevent it.
#[test]
fn a_manipulated_fallback_call_costs_a_bounded_amount() {
    for push in [5u64, 20, 60] {
        let mut env = env();
        let k = migrated(&mut env);
        pending(&mut env, &k, 10);
        env.set_time(env.launch(&k).last_reinvest_at + dex::KEEPER_GRACE_SECS);
        let fair = env.pool_sqrt_price(&k);
        let attacker = env.dex_trader(push + 2, &k.mint);
        let ap = pk(&attacker.pubkey());
        let wsol_start = env.token_balance(&ata(&ap, &WSOL));
        env.dex_swap(&k, &attacker, push * SOL, true).unwrap();
        let pushed = env.pool_sqrt_price(&k);
        let before = env.launch(&k);
        env.reinvest_lp_at(&k, &attacker, pushed).unwrap();
        let l = env.launch(&k);
        let used = before.lp_reinvest_pending - l.lp_reinvest_pending;
        let tokens = env.token_balance(&ata(&ap, &k.mint));
        env.dex_swap(&k, &attacker, tokens, false).unwrap();
        let attacker_pnl = env.token_balance(&ata(&ap, &WSOL)) as i128 - wsol_start as i128;
        // What the protocol now holds for that SOL: the added liquidity, valued at the price before the push.
        let (sol, toks) = dex::reserves(l.lp_liquidity - before.lp_liquidity, dex::MIN_SQRT_PRICE, fair, dex::MAX_SQRT_PRICE).unwrap();
        let loss = used as f64 - (sol as f64 + value(toks as u64, fair));
        let pct = ((pushed as f64 / fair as f64).powi(2) - 1.0) * 100.0;
        println!(
            "REPORT push {push} SOL (+{pct:.1}% price): reinvested {:.4} SOL, protocol loss {:.4} SOL ({:.2}% of it, {:.3}% of pool SOL), attacker {:+.4} SOL",
            used as f64 / 1e9, loss / 1e9, loss / used as f64 * 100.0, loss / pool_sol(&env, &k) as f64 * 100.0, attacker_pnl as f64 / 1e9
        );
        assert!(loss < used as f64 * 0.5, "loss is a fraction of one capped call");
        assert!(attacker_pnl < 0, "with a 1% pool fee the attacker still loses at {push} SOL");
    }
}

#[test]
fn band_must_be_narrow_and_contain_the_price() {
    let mut env = env();
    let k = migrated(&mut env);
    pending(&mut env, &k, 1);
    let keeper = env.admin.insecure_clone();
    let p = env.pool_sqrt_price(&k);
    expect_err(env.reinvest_lp_band(&k, &keeper, p - p / 50, p + p / 50), "BandTooWide");
    expect_err(env.reinvest_lp_band(&k, &keeper, p + 1, p + p / 400), "PriceOutOfBand");
    expect_err(env.reinvest_lp_band(&k, &keeper, p - p / 400, p - 1), "PriceOutOfBand");
    env.reinvest_lp_band(&k, &keeper, p, p).unwrap();
}

#[test]
fn reinvest_accounts_cannot_be_swapped() {
    let mut env = env();
    let a = migrated(&mut env);
    let b = migrated(&mut env);
    pending(&mut env, &a, 1);
    let keeper = env.admin.insecure_clone();
    let kp = pk(&keeper.pubkey());
    let db = dex_keys(&b);
    let p = env.pool_sqrt_price(&a);
    let cases: Vec<(&str, herdcall::accounts::ReinvestLp, &str)> = vec![
        ("other pool", herdcall::accounts::ReinvestLp { pool: db.pool, ..env.reinvest_lp_accounts(&a, &kp) }, "WrongAccount"),
        ("other position", herdcall::accounts::ReinvestLp { position: db.position, ..env.reinvest_lp_accounts(&a, &kp) }, "WrongAccount"),
        ("other vault", herdcall::accounts::ReinvestLp { sol_vault: b.sol_vault, ..env.reinvest_lp_accounts(&a, &kp) }, "ConstraintSeeds"),
        ("fake DAMM", herdcall::accounts::ReinvestLp { dex_program: anchor_spl::token::ID, ..env.reinvest_lp_accounts(&a, &kp) }, "ConstraintAddress"),
        ("other NFT account", herdcall::accounts::ReinvestLp { position_nft_account: db.position_nft_account, ..env.reinvest_lp_accounts(&a, &kp) }, "ConstraintRaw"),
        ("fake config", herdcall::accounts::ReinvestLp { config: a.creator_fees, ..env.reinvest_lp_accounts(&a, &kp) }, "AccountDiscriminatorMismatch"),
    ];
    for (what, accounts, code) in cases {
        let data = anchor_lang::InstructionData::data(&herdcall::instruction::ReinvestLp { min_sqrt_price: p, max_sqrt_price: p });
        let ix = env.ix(anchor_lang::ToAccountMetas::to_account_metas(&accounts, None), data);
        let r = env.send(ix, &[&keeper]);
        assert!(r.as_ref().unwrap_err().contains(code), "{what}: expected {code}, got {r:?}");
    }
    env.reinvest_lp(&a).unwrap();
}
