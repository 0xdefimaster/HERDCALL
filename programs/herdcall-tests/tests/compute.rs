//! Compute units of every instruction on a full lifecycle, against ceilings with room below Solana's limits (200k per
//! instruction by default, 1.4M per transaction). The ceilings hold for the build that is deployed.
mod common;

use common::*;
use solana_signer::Signer;

fn record(env: &Env, name: &str, ceiling: u64, out: &mut Vec<(String, u64, u64)>) {
    out.push((name.to_string(), env.last_cu, ceiling));
}

#[test]
fn every_instruction_stays_under_its_compute_ceiling() {
    let mut env = Env::new();
    let mut cu = Vec::new();
    env.initialize(example_params()).unwrap();
    record(&env, "initialize", 60_000, &mut cu);
    let k = env.create_launch(true, 60).unwrap();
    record(&env, "create_launch", 200_000, &mut cu);
    let u = env.user(500);
    env.deposit(&k, &u, 3 * SOL).unwrap();
    record(&env, "deposit", 40_000, &mut cu);
    env.set_time(env.launch(&k).close_at);
    env.settle(&k, &u).unwrap();
    record(&env, "settle", 80_000, &mut cu);
    env.claim(&k, &u).unwrap();
    record(&env, "claim", 80_000, &mut cu);
    env.buy_exact_in(&k, &u, SOL, 1, None).unwrap();
    record(&env, "buy_exact_in", 150_000, &mut cu);
    env.sell(&k, &u, 1_000_000_000, 0, None).unwrap();
    record(&env, "sell", 150_000, &mut cu);
    env.buy_exact_out(&k, &u, u64::MAX, u64::MAX, None).unwrap();
    record(&env, "buy_exact_out (rest of the curve)", 150_000, &mut cu);
    let keeper = env.user(1);
    env.breakout(&k, &keeper).unwrap();
    record(&env, "breakout", 350_000, &mut cu);
    let t = env.dex_trader(21, &k.mint);
    env.dex_swap(&k, &t, 20 * SOL, true).unwrap();
    let caller = env.user(1);
    env.claim_lp_fees(&k, &caller).unwrap();
    record(&env, "claim_lp_fees", 300_000, &mut cu);
    env.reinvest_lp(&k).unwrap();
    record(&env, "reinvest_lp", 400_000, &mut cu);
    let creator = k.creator.insecure_clone();
    env.withdraw_creator_fees(&k, &creator).unwrap();
    record(&env, "withdraw_creator_fees", 20_000, &mut cu);

    let mut over = Vec::new();
    for (name, used, ceiling) in &cu {
        println!("CU {name:<36} {used:>9} (ceiling {ceiling})");
        if used > ceiling {
            over.push(name.clone());
        }
    }
    assert!(over.is_empty(), "over the ceiling: {over:?}");
    let _ = u.pubkey();
}
