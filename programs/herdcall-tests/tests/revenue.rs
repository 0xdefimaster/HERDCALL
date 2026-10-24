//! The protocol revenue ledger (HERDCALL.md §7): what the protocol vault receives, by source, counted once.
mod common;

use common::*;
use herdcall::state::*;
use herdcall_core::fees::{curve_protocol_revenue, lp_protocol_revenue, split_fee, split_lp_fees, Kind};
use herdcall_core::stampede::settle as core_settle;
use herdcall_core::BPS;
use solana_signer::Signer;

fn ledger(env: &Env) -> RevenueLedger {
    env.account(&pda(&[REVENUE_SEED]))
}

/// Everything in the protocol vault beyond its rent is in exactly one counter.
fn assert_balanced(env: &Env, ctx: &str) {
    let r = ledger(env);
    let protocol = pda(&[PROTOCOL_SEED]);
    let held = env.lamports(&protocol) - env.rent(&protocol);
    let counters = (r.curve, r.stampede, r.lp, r.rounding, r.other);
    assert_eq!(held, r.curve + r.stampede + r.lp + r.rounding + r.other, "{ctx}: vault = sum of counters {counters:?}");
}

/// What the ledger should say, accumulated from the core's splits.
#[derive(Default, Debug, PartialEq)]
struct Expected {
    curve: u64,
    stampede: u64,
    lp: u64,
    rounding: u64,
}

impl Expected {
    fn curve(&mut self, fee: u128, kind: Kind, iface: bool) {
        let received = split_fee(fee, kind, iface, BPS).unwrap().protocol;
        let revenue = curve_protocol_revenue(fee, kind, iface).unwrap();
        self.curve += revenue as u64;
        self.rounding += (received - revenue) as u64;
    }

    fn lp(&mut self, fee: u128, kind: Kind) {
        let received = split_lp_fees(fee, kind, BPS).unwrap().protocol;
        let revenue = lp_protocol_revenue(fee, kind).unwrap();
        self.lp += revenue as u64;
        self.rounding += (received - revenue) as u64;
    }

    fn check(&self, env: &Env, ctx: &str) {
        let r = ledger(env);
        assert_eq!(
            (r.curve, r.stampede, r.lp, r.rounding),
            (self.curve, self.stampede, self.lp, self.rounding),
            "{ctx}"
        );
        assert_balanced(env, ctx);
    }
}

fn run_lifecycle(kind: Kind) {
    let bonded = kind == Kind::Bonded;
    let mut env = Env::new();
    env.initialize(example_params()).unwrap();
    let mut want = Expected::default();
    want.check(&env, "start");

    let k = env.create_launch(bonded, 60).unwrap();
    let u = env.user(500);
    env.deposit(&k, &u, 3 * SOL + 777).unwrap();
    env.set_time(env.launch(&k).close_at);
    env.settle(&k, &u).unwrap();
    let (s, curve) = core_settle(&example_params().lobby(), (3 * SOL + 777) as u128).unwrap();
    let received = split_fee(s.f, kind, false, BPS).unwrap().protocol;
    let revenue = curve_protocol_revenue(s.f, kind, false).unwrap();
    want.stampede += revenue as u64;
    want.rounding += (received - revenue) as u64;
    want.check(&env, "stampede");

    // Curve trades with and without an interface, replayed on the core market for the fees.
    let iface_owner = env.user(1);
    let iface = env.register_interface(&iface_owner).unwrap();
    let mut m = herdcall_core::curve::Market::new(curve, example_params().market(kind)).unwrap();
    for (i, gross) in [SOL + 13, 2 * SOL / 3, 5 * SOL + 1].into_iter().enumerate() {
        let with = i % 2 == 0;
        let q = m.buy_exact_in(gross as u128, 1, with).unwrap();
        env.buy_exact_in(&k, &u, gross, 1, with.then_some(iface)).unwrap();
        want.curve(q.fee, kind, with);
        want.check(&env, &format!("buy {i}"));
    }
    let tokens = env.token_balance(&ata(&pk(&u.pubkey()), &k.mint)) / 3;
    let q = m.sell(tokens as u128, 0, false).unwrap();
    env.sell(&k, &u, tokens, 0, None).unwrap();
    want.curve(q.fee, kind, false);
    want.check(&env, "sell");
    // A failed transaction changes nothing.
    assert!(env.sell(&k, &u, tokens, u64::MAX, None).is_err());
    want.check(&env, "failed sell");

    let q = m.buy_exact_out(u64::MAX as u128, u128::MAX, false).unwrap();
    env.buy_exact_out(&k, &u, u64::MAX, u64::MAX, None).unwrap();
    want.curve(q.fee, kind, false);
    want.check(&env, "last buy");

    // Breakout: the few lamports the pool could not take are not revenue.
    let keeper = env.user(1);
    env.breakout(&k, &keeper).unwrap();
    let r = ledger(&env);
    assert!(r.other > 0, "breakout leftover recorded as other");
    assert_eq!((r.curve, r.stampede, r.lp), (want.curve, want.stampede, want.lp), "breakout adds no revenue");
    assert_balanced(&env, "breakout");

    // LP fees, claimed twice; a claim with nothing new and a reinvest change nothing.
    for round in 0..2 {
        let t = env.dex_trader(21, &k.mint);
        env.dex_swap(&k, &t, 20 * SOL, true).unwrap();
        let bought = env.token_balance(&ata(&pk(&t.pubkey()), &k.mint));
        env.dex_swap(&k, &t, bought, false).unwrap();
        let before = env.launch(&k).lp_fees_claimed;
        let caller = env.user(1);
        env.claim_lp_fees(&k, &caller).unwrap();
        want.lp(u128::from(env.launch(&k).lp_fees_claimed - before), kind);
        let r = ledger(&env);
        assert_eq!((r.curve, r.stampede, r.lp), (want.curve, want.stampede, want.lp), "claim {round}");
        assert_balanced(&env, &format!("claim {round}"));
        assert!(env.claim_lp_fees(&k, &caller).is_err(), "no fees left");
    }
    let before = ledger(&env);
    env.reinvest_lp(&k).unwrap();
    let after = ledger(&env);
    assert_eq!(
        (before.curve, before.stampede, before.lp, before.rounding, before.other),
        (after.curve, after.stampede, after.lp, after.rounding, after.other),
        "a reinvest is not revenue"
    );
    // The creator withdrawing its fees does not touch the protocol vault or the ledger.
    let creator = k.creator.insecure_clone();
    env.withdraw_creator_fees(&k, &creator).unwrap();
    assert_balanced(&env, "end");
    let r = ledger(&env);
    println!(
        "REPORT {kind:?}: curve {} stampede {} lp {} revenue {} | rounding {} other {} lamports",
        r.curve, r.stampede, r.lp, r.revenue().unwrap(), r.rounding, r.other
    );
}

#[test]
fn bonded_revenue_is_counted_once_by_source() {
    run_lifecycle(Kind::Bonded);
}

#[test]
fn unbonded_revenue_is_counted_once_by_source() {
    run_lifecycle(Kind::Unbonded);
}

#[test]
fn the_ledger_cannot_be_swapped() {
    let mut env = Env::new();
    env.initialize(example_params()).unwrap();
    let k = env.create_launch(true, 60).unwrap();
    let u = env.user(20);
    env.deposit(&k, &u, SOL).unwrap();
    env.set_time(env.launch(&k).close_at);
    env.settle(&k, &u).unwrap();
    let mut metas = env.trade_metas(&k, &pk(&u.pubkey()), None);
    let i = metas.iter().position(|m| m.pubkey == pda(&[REVENUE_SEED])).unwrap();
    metas[i].pubkey = k.creator_fees;
    let ix = env.ix(metas, anchor_lang::InstructionData::data(&herdcall::instruction::BuyExactIn { gross: SOL, min_tokens: 1 }));
    let r = env.send(ix, &[&u]);
    assert!(r.unwrap_err().contains("AccountDiscriminatorMismatch"));
}

#[test]
fn sol_sent_straight_to_the_vault_is_not_revenue() {
    let mut env = Env::new();
    env.initialize(example_params()).unwrap();
    let k = env.create_launch(true, 60).unwrap();
    let u = env.user(20);
    env.deposit(&k, &u, 2 * SOL).unwrap();
    env.set_time(env.launch(&k).close_at);
    env.settle(&k, &u).unwrap();
    let before = ledger(&env);
    // Anyone can send lamports to the protocol PDA without the program: no counter moves, and the balance is then
    // above the sum of the counters. A distribution must use the counters, never the balance.
    let gift = 5 * SOL + 3;
    env.add_lamports(&pda(&[PROTOCOL_SEED]), gift);
    env.buy_exact_in(&k, &u, SOL, 1, None).unwrap();
    let after = ledger(&env);
    assert_eq!(after.stampede, before.stampede);
    assert_eq!(after.other, before.other);
    assert!(after.curve > before.curve, "the trade is still counted");
    let protocol = pda(&[PROTOCOL_SEED]);
    let held = env.lamports(&protocol) - env.rent(&protocol);
    let counted = after.curve + after.stampede + after.lp + after.rounding + after.other;
    assert_eq!(held - counted, gift, "the gift is exactly the uncounted part");
}
