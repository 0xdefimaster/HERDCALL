//! Adding the "back to liquidity" share of the LP fees to the locked position (CURVE_SPEC §5.7), against the real
//! DAMM v2 binary.
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

fn pool(env: &Env, k: &LaunchKeys) -> (u128, u128) {
    let d = env.data(&dex_keys(k).pool);
    (u128_at(&d, 360), u128_at(&d, 456))
}

fn pool_sol(env: &Env, k: &LaunchKeys) -> u128 {
    let (l, p) = pool(env, k);
    dex::reserves(l, dex::MIN_SQRT_PRICE, p, dex::MAX_SQRT_PRICE).unwrap().0
}

#[test]
fn pending_share_is_added_and_locked() {
    let mut env = env();
    let k = migrated(&mut env);
    let caller = env.user(1);
    expect_err(env.reinvest_lp(&k, &caller), "NothingToReinvest");
    earn_fees(&mut env, &k, 40);
    let before = env.launch(&k);
    assert!(before.lp_reinvest_pending >= dex::REINVEST_MIN_LAMPORTS, "pending {}", before.lp_reinvest_pending);
    let d = dex_keys(&k);
    let (pool_l, sqrt_before) = pool(&env, &k);
    let locked_before = u128_at(&env.data(&d.position), 184);
    let supply_before = env.mint_info(&k.mint).1;
    let caller_before = env.lamports(&pk(&caller.pubkey()));

    env.reinvest_lp(&k, &caller).unwrap();
    let l = env.launch(&k);
    let added = l.lp_liquidity - before.lp_liquidity;
    assert!(added > 0);
    // Everything added is locked for good; nothing is unlocked.
    let position = env.data(&d.position);
    assert_eq!((u128_at(&position, 152), u128_at(&position, 184)), (0, locked_before + added));
    assert_eq!(pool(&env, &k).0, pool_l + added, "pool liquidity");
    // SOL taken from the pending share; what was not used still waits in the vault.
    let used = before.lp_reinvest_pending - l.lp_reinvest_pending;
    assert_eq!(l.lp_reinvested, used);
    assert!(used > before.lp_reinvest_pending * 98 / 100, "almost all of it is used: {used}");
    assert!(env.lamports(&k.sol_vault) >= env.rent(&k.sol_vault) + l.lp_reinvest_pending);
    // The few leftover tokens are burned.
    assert_eq!(supply_before - env.mint_info(&k.mint).1, l.burned - before.burned);
    // The swap stays within the on-chain limits.
    assert!(dex::price_move_ok(sqrt_before, pool(&env, &k).1));
    // Caller pays only the transaction fee; temporary accounts and the migrator are empty again.
    assert_eq!(caller_before - env.lamports(&pk(&caller.pubkey())), 5_000);
    for seed in [REINVEST_WSOL_SEED, REINVEST_TOKEN_SEED] {
        assert!(env.data(&pda(&[seed, k.launch.as_ref()])).is_empty());
    }
    assert_eq!(env.lamports(&migrator()), 0);
    // Under the minimum now: the rest keeps waiting.
    expect_err(env.reinvest_lp(&k, &caller), "NothingToReinvest");
}

#[test]
fn one_call_takes_at_most_half_a_percent_of_the_pool() {
    let mut env = env();
    let k = migrated(&mut env);
    // 10 SOL waiting (as if many claims had added up), far above the cap.
    env.patch_launch(&k, |l| l.lp_reinvest_pending = 10 * SOL);
    env.add_lamports(&k.sol_vault, 10 * SOL);
    let cap = (pool_sol(&env, &k) * dex::REINVEST_MAX_POOL_BPS / 10_000) as u64;
    let caller = env.user(1);
    env.reinvest_lp(&k, &caller).unwrap();
    let first = 10 * SOL - env.launch(&k).lp_reinvest_pending;
    assert!(first <= cap && first > cap * 98 / 100, "used {first}, cap {cap}");
    // The next call is capped again (by the slightly larger pool).
    env.reinvest_lp(&k, &caller).unwrap();
    let l = env.launch(&k);
    assert!(10 * SOL - l.lp_reinvest_pending - first <= (pool_sol(&env, &k) * 50 / 10_000) as u64);
    assert!(l.lp_reinvest_pending > 8 * SOL);
}

/// A sandwich against a capped reinvest on the real pool: buy before, sell after. With a 1% pool fee it loses money
/// at every size tried (the simulation's claim, checked on the real program; not a guarantee for other conditions).
#[test]
fn sandwiching_a_capped_reinvest_loses_money() {
    for attack in [1u64, 5, 20, 60] {
        let mut env = env();
        let k = migrated(&mut env);
        env.patch_launch(&k, |l| l.lp_reinvest_pending = 10 * SOL);
        env.add_lamports(&k.sol_vault, 10 * SOL);
        let a = env.dex_trader(attack + 1, &k.mint);
        let ap = pk(&a.pubkey());
        let wsol_before = env.token_balance(&ata(&ap, &WSOL));
        env.dex_swap(&k, &a, attack * SOL, true).unwrap();
        let caller = env.user(1);
        env.reinvest_lp(&k, &caller).unwrap();
        let tokens = env.token_balance(&ata(&ap, &k.mint));
        env.dex_swap(&k, &a, tokens, false).unwrap();
        let back = env.token_balance(&ata(&ap, &WSOL)) as i128 - (wsol_before as i128 - (attack * SOL) as i128);
        let profit = back - (attack * SOL) as i128;
        println!("REPORT sandwich of {attack} SOL: profit {profit} lamports");
        assert!(profit < 0, "attack {attack} SOL made {profit}");
    }
}

#[test]
fn reinvest_accounts_cannot_be_swapped() {
    let mut env = env();
    let a = migrated(&mut env);
    let b = migrated(&mut env);
    env.patch_launch(&a, |l| l.lp_reinvest_pending = SOL);
    env.add_lamports(&a.sol_vault, SOL);
    let caller = env.user(1);
    let cp = pk(&caller.pubkey());
    let db = dex_keys(&b);
    let cases: Vec<(&str, herdcall::accounts::ReinvestLp, &str)> = vec![
        ("other pool", herdcall::accounts::ReinvestLp { pool: db.pool, ..env.reinvest_lp_accounts(&a, &cp) }, "WrongAccount"),
        ("other position", herdcall::accounts::ReinvestLp { position: db.position, ..env.reinvest_lp_accounts(&a, &cp) }, "WrongAccount"),
        ("other vault", herdcall::accounts::ReinvestLp { sol_vault: b.sol_vault, ..env.reinvest_lp_accounts(&a, &cp) }, "ConstraintSeeds"),
        ("fake DAMM", herdcall::accounts::ReinvestLp { dex_program: anchor_spl::token::ID, ..env.reinvest_lp_accounts(&a, &cp) }, "ConstraintAddress"),
        ("other NFT account", herdcall::accounts::ReinvestLp { position_nft_account: db.position_nft_account, ..env.reinvest_lp_accounts(&a, &cp) }, "ConstraintRaw"),
    ];
    for (what, accounts, code) in cases {
        let ix = env.ix(anchor_lang::ToAccountMetas::to_account_metas(&accounts, None), anchor_lang::InstructionData::data(&herdcall::instruction::ReinvestLp {}));
        let r = env.send(ix, &[&caller]);
        assert!(r.as_ref().unwrap_err().contains(code), "{what}: expected {code}, got {r:?}");
    }
    env.reinvest_lp(&a, &caller).unwrap();
}

/// If the swap returns much less than the spot amount (here: the pool's fee raised to 40% behind its back), the
/// reinvest fails instead of adding at a bad rate.
#[test]
fn swap_slippage_limit_is_enforced() {
    let mut env = env();
    let k = migrated(&mut env);
    env.patch_launch(&k, |l| l.lp_reinvest_pending = SOL);
    env.add_lamports(&k.sol_vault, SOL);
    let pool_key = dex_keys(&k).pool;
    let mut acc = env.svm.get_account(&addr(&pool_key)).unwrap();
    acc.data[8..16].copy_from_slice(&400_000_000u64.to_le_bytes()); // base fee numerator: 40%
    env.svm.set_account(addr(&pool_key), acc).unwrap();
    let caller = env.user(1);
    expect_err(env.reinvest_lp(&k, &caller), "ExceededSlippage");
    assert_eq!(env.launch(&k).lp_reinvest_pending, SOL, "nothing taken");
}
