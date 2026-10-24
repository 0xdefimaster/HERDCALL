//! End-to-end tests of the compiled program (target/deploy/herdcall.so) in LiteSVM. Every number the program produces
//! is checked against `herdcall-core`, which is itself checked unit-for-unit against the JS reference model.
mod common;

use common::*;
use herdcall::state::*;
use herdcall_core::curve::{Market, Phase as CorePhase};
use herdcall_core::dex;
use herdcall_core::fees::{split_fee, FeeParts, Kind};
use herdcall_core::stampede::{claim_tokens, settle as core_settle};
use herdcall_core::BPS;
use solana_keypair::Keypair;
use solana_signer::Signer;

const TX_FEE: u64 = 5_000;

fn expect_err(r: Result<(), String>, code: &str) {
    match r {
        Ok(()) => panic!("expected {code}, transaction succeeded"),
        Err(e) => assert!(e.contains(code), "expected {code}, got:\n{e}"),
    }
}

/// The five fee vaults of a launch, read together so deltas can be compared with `split_fee`.
struct FeeVaults {
    creator_now: u64,
    creator_escrow: u64,
    interface: u64,
    rekt_back: u64,
    protocol: u64,
}

fn fee_vaults(env: &Env, k: &LaunchKeys, iface: Option<anchor_lang::prelude::Pubkey>) -> FeeVaults {
    FeeVaults {
        creator_now: env.lamports(&k.creator_fees),
        creator_escrow: env.lamports(&k.creator_escrow),
        interface: iface.map(|i| env.lamports(&i)).unwrap_or(0),
        rekt_back: env.lamports(&pda(&[REKT_BACK_SEED])),
        protocol: env.lamports(&pda(&[PROTOCOL_SEED])),
    }
}

fn assert_fee_delta(before: &FeeVaults, after: &FeeVaults, want: &FeeParts, ctx: &str) {
    assert_eq!((after.creator_now - before.creator_now) as u128, want.creator_now, "{ctx}: creator now");
    assert_eq!((after.creator_escrow - before.creator_escrow) as u128, want.creator_escrow, "{ctx}: creator escrow");
    assert_eq!((after.interface - before.interface) as u128, want.interface, "{ctx}: interface");
    assert_eq!((after.rekt_back - before.rekt_back) as u128, want.rekt_back, "{ctx}: rekt-back");
    assert_eq!((after.protocol - before.protocol) as u128, want.protocol, "{ctx}: protocol");
}

fn assert_curve(l: &Launch, m: &Market, ctx: &str) {
    assert_eq!(l.v as u128, m.curve.v, "{ctx}: v");
    assert_eq!(l.t as u128, m.curve.t, "{ctx}: t");
    assert_eq!(l.k, m.curve.k, "{ctx}: k");
    assert_eq!(l.reserve as u128, m.curve.reserve, "{ctx}: reserve");
    assert_eq!(l.sold as u128, m.curve.sold, "{ctx}: sold");
    let want = match m.phase {
        CorePhase::Curve => Phase::Curve,
        CorePhase::BreakingOut => Phase::BreakingOut,
        CorePhase::Migrated => Phase::Migrated,
    };
    assert_eq!(l.phase, want, "{ctx}: phase");
}

/// The SOL vault always holds its rent, the curve reserve and every Lobby deposit still owed back.
fn assert_solvent(env: &Env, k: &LaunchKeys, ctx: &str) {
    let l = env.launch(k);
    let floor = env.rent(&k.sol_vault) + l.reserve;
    assert!(env.lamports(&k.sol_vault) >= floor, "{ctx}: SOL vault below rent + reserve");
}

fn setup(bonded: bool) -> (Env, LaunchKeys) {
    let mut env = Env::new();
    env.initialize(example_params()).unwrap();
    let k = env.create_launch(bonded, 60).unwrap();
    (env, k)
}

/// Lobby → Stampede → claims → curve trades → breakout, with every state and lamport checked against the core.
#[test]
fn full_lifecycle_matches_core() {
    let (mut env, k) = setup(true);
    let p = example_params();

    // The token: fixed supply in the vault, no mint authority.
    assert_eq!(env.mint_info(&k.mint), (false, p.total_supply));
    assert_eq!(env.token_balance(&k.token_vault), p.total_supply);

    // Lobby.
    let amounts = [3 * SOL, SOL / 2, 7 * SOL + 123_457, SOL / 100, 2 * SOL];
    let users: Vec<Keypair> = amounts.iter().map(|_| env.user(50)).collect();
    for (u, a) in users.iter().zip(amounts) {
        env.deposit(&k, u, a).unwrap();
    }
    env.deposit(&k, &users[0], SOL).unwrap(); // deposits of one wallet add up
    let deposits: Vec<u64> = amounts.iter().enumerate().map(|(i, a)| if i == 0 { a + SOL } else { *a }).collect();
    let total: u64 = deposits.iter().sum();
    let l = env.launch(&k);
    assert_eq!((l.total_deposits, l.depositors), (total, 5));

    // Stampede.
    env.set_time(l.close_at);
    let fees_before = fee_vaults(&env, &k, None);
    let keeper = env.user(1);
    env.settle(&k, &keeper).unwrap();
    let (s, curve) = core_settle(&p.lobby(), total as u128).unwrap();
    assert!(!s.empty);
    let l = env.launch(&k);
    assert_eq!(l.lobby_tokens as u128, s.x);
    assert_eq!(l.phase, Phase::Curve);
    let mut m = Market::new(curve, p.market(Kind::Bonded)).unwrap();
    assert_curve(&l, &m, "after settle");
    assert_fee_delta(&fees_before, &fee_vaults(&env, &k, None), &split_fee(s.f, Kind::Bonded, false, BPS).unwrap(), "stampede fee");
    assert_solvent(&env, &k, "after settle");
    expect_err(env.settle(&k, &keeper), "LobbyClosed");

    // Claims (the last depositor waits until after the breakout).
    for (u, d) in users.iter().zip(&deposits).take(4) {
        env.claim(&k, u).unwrap();
        let want = claim_tokens(*d as u128, total as u128, s.x).unwrap();
        assert_eq!(env.token_balance(&ata(&pk(&u.pubkey()), &k.mint)) as u128, want);
        expect_err(env.claim(&k, u), "AlreadyWithdrawn");
    }

    // Curve trades against the core replica.
    let iface_owner = env.user(1);
    let iface = env.register_interface(&iface_owner).unwrap();
    let trader = env.user(200);
    let t = pk(&trader.pubkey());
    enum Op {
        In(u64),
        Out(u64),
        Sell(u64),
    }
    let ops = [
        (Op::In(2 * SOL), None),
        (Op::Out(10_000_000 * TOKEN), Some(iface)),
        (Op::Sell(3_000_000 * TOKEN), None),
        (Op::In(SOL / 3 + 17), Some(iface)),
        (Op::Sell(1_234_567), Some(iface)),
        (Op::Out(1), None),
    ];
    for (i, (op, iface_key)) in ops.iter().enumerate() {
        let ctx = format!("op {i}");
        let has = iface_key.is_some();
        let before = fee_vaults(&env, &k, Some(iface));
        let vault_before = env.lamports(&k.sol_vault);
        let trader_before = env.lamports(&t);
        let tokens_before = env.token_balance(&ata(&t, &k.mint));
        match *op {
            Op::In(gross) => {
                let q = m.buy_exact_in(gross as u128, 1, has).unwrap();
                env.buy_exact_in(&k, &trader, gross, 1, *iface_key).unwrap();
                assert_eq!((env.lamports(&k.sol_vault) - vault_before) as u128, q.cost, "{ctx}: reserve in");
                assert_eq!((env.token_balance(&ata(&t, &k.mint)) - tokens_before) as u128, q.tokens, "{ctx}: tokens");
                assert_fee_delta(&before, &fee_vaults(&env, &k, Some(iface)), &split_fee(q.fee, Kind::Bonded, has, BPS).unwrap(), &ctx);
                assert!(trader_before - env.lamports(&t) <= gross + TX_FEE + 3_000_000, "{ctx}: trader paid more than gross");
            }
            Op::Out(tokens) => {
                let q = m.buy_exact_out(tokens as u128, u128::MAX, has).unwrap();
                env.buy_exact_out(&k, &trader, tokens, u64::MAX, *iface_key).unwrap();
                assert_eq!((env.lamports(&k.sol_vault) - vault_before) as u128, q.cost, "{ctx}: reserve in");
                assert_eq!((env.token_balance(&ata(&t, &k.mint)) - tokens_before) as u128, q.tokens, "{ctx}: tokens");
                assert_fee_delta(&before, &fee_vaults(&env, &k, Some(iface)), &split_fee(q.fee, Kind::Bonded, has, BPS).unwrap(), &ctx);
            }
            Op::Sell(tokens) => {
                let q = m.sell(tokens as u128, 0, has).unwrap();
                env.sell(&k, &trader, tokens, 0, *iface_key).unwrap();
                assert_eq!((vault_before - env.lamports(&k.sol_vault)) as u128, q.out, "{ctx}: reserve out");
                assert_eq!((tokens_before - env.token_balance(&ata(&t, &k.mint))) as u128, tokens as u128, "{ctx}: tokens");
                assert_eq!((env.lamports(&t) + TX_FEE - trader_before) as u128, q.received, "{ctx}: received");
                assert_fee_delta(&before, &fee_vaults(&env, &k, Some(iface)), &split_fee(q.fee, Kind::Bonded, has, BPS).unwrap(), &ctx);
            }
        }
        assert_curve(&env.launch(&k), &m, &ctx);
        assert_solvent(&env, &k, &ctx);
    }

    // Buy the rest: the order is clamped to what is left and the token starts breaking out.
    let left = (p.curve_supply - env.launch(&k).sold) as u128;
    let q = m.buy_exact_out(u64::MAX as u128, u128::MAX, false).unwrap();
    assert!(q.clamped && q.tokens == left);
    env.buy_exact_out(&k, &trader, u64::MAX, u64::MAX, None).unwrap();
    assert_curve(&env.launch(&k), &m, "last buy");
    assert_eq!(env.launch(&k).phase, Phase::BreakingOut);
    expect_err(env.sell(&k, &trader, TOKEN, 0, None), "TradingClosed");
    expect_err(env.buy_exact_in(&k, &trader, SOL, 1, None), "TradingClosed");

    // Breakout: the Meteora DAMM v2 pool opens at the curve's final price and the position is locked for good.
    let keeper_before = env.lamports(&pk(&keeper.pubkey()));
    let vault_before = env.lamports(&k.sol_vault);
    let protocol_before = env.lamports(&pda(&[PROTOCOL_SEED]));
    let mig = m.breakout().unwrap();
    let sqrt_price = dex::sqrt_price_ceil(mig.final_spot.0, mig.final_spot.1).unwrap();
    let dep = dex::fit(mig.lp_tokens as u64, mig.lp_native as u64, dex::MIN_SQRT_PRICE, sqrt_price, dex::MAX_SQRT_PRICE).unwrap();
    env.breakout(&k, &keeper).unwrap();
    let l = env.launch(&k);
    let d = dex_keys(&k);
    assert_eq!(l.phase, Phase::Migrated);
    assert_eq!((l.lp_native, l.lp_tokens, l.lp_liquidity), (dep.b, dep.a, dep.liquidity));
    assert_eq!(l.burned as u128, mig.burned + (mig.lp_tokens - dep.a as u128));
    assert_eq!((l.dex_pool, l.dex_position), (d.pool, d.position));
    // Everything set aside left the vault: the LP SOL and the migration fee.
    assert_eq!((vault_before - env.lamports(&k.sol_vault)) as u128, mig.lp_native + mig.keeper_fee);
    assert!(env.lamports(&k.sol_vault) >= env.rent(&k.sol_vault));
    // The pool holds exactly what the core says, at the curve's final price.
    assert_eq!(env.token_balance(&d.token_vault), dep.a);
    assert_eq!(env.token_balance(&d.wsol_vault), dep.b);
    let pool = env.data(&d.pool);
    assert_eq!(&pool[168..200], k.mint.as_ref(), "token A is the launch token");
    assert_eq!(u128_at(&pool, 360), dep.liquidity, "pool liquidity");
    assert_eq!(u128_at(&pool, 456), sqrt_price, "pool price = curve's final price");
    // The whole position is locked for good and its NFT stays with the launch PDA.
    let position = env.data(&d.position);
    assert_eq!((u128_at(&position, 152), u128_at(&position, 184)), (0, dep.liquidity), "unlocked, permanently locked");
    let nft = env.data(&d.position_nft_account);
    assert_eq!((&nft[32..64], env.token_balance(&d.position_nft_account)), (k.launch.as_ref(), 1));
    // The SOL the pool could not take goes to the protocol; the migration fee pays the pool's rent, the keeper keeps
    // the rest; the migrator and its temporary accounts are empty again.
    assert_eq!((env.lamports(&pda(&[PROTOCOL_SEED])) - protocol_before) as u128, mig.lp_native - dep.b as u128);
    let pool_rent = env.lamports(&d.pool) + env.lamports(&d.position) + env.lamports(&d.position_nft_mint)
        + env.lamports(&d.position_nft_account) + env.lamports(&d.token_vault) + env.lamports(&d.wsol_vault) - dep.b;
    assert!((pool_rent as u128) < mig.keeper_fee, "the migration fee covers the pool's rent ({pool_rent})");
    assert_eq!(env.lamports(&pk(&keeper.pubkey())) + TX_FEE + pool_rent - keeper_before, mig.keeper_fee as u64, "keeper");
    assert_eq!(env.lamports(&migrator()), 0);
    for temp in [MIGRATOR_TOKEN_SEED, MIGRATOR_WSOL_SEED] {
        assert!(env.data(&pda(&[temp, k.launch.as_ref()])).is_empty(), "temporary account closed");
    }
    let unclaimed = claim_tokens(deposits[4] as u128, total as u128, s.x).unwrap();
    let dust = s.x - deposits.iter().map(|d| claim_tokens(*d as u128, total as u128, s.x).unwrap()).sum::<u128>();
    assert_eq!(env.token_balance(&k.token_vault) as u128, unclaimed + dust, "only unclaimed Lobby tokens stay");
    assert_eq!(env.mint_info(&k.mint), (false, p.total_supply - l.burned));
    expect_err(env.breakout(&k, &keeper), "NotBreakingOut");
    expect_err(env.buy_exact_in(&k, &trader, SOL, 1, None), "TradingClosed");

    // A Lobby claim still works after the breakout and never touches the LP tokens.
    env.claim(&k, &users[4]).unwrap();
    assert_eq!(env.token_balance(&ata(&pk(&users[4].pubkey()), &k.mint)) as u128, unclaimed);
    assert_eq!(env.token_balance(&k.token_vault) as u128, dust);

    // Fee withdrawals: only the owners, never below rent.
    let cf = env.lamports(&k.creator_fees) - env.rent(&k.creator_fees);
    assert!(cf > 0);
    let stranger = env.user(1);
    expect_err(env.withdraw_creator_fees(&k, &stranger), "ConstraintHasOne");
    let creator = k.creator.insecure_clone();
    let before = env.lamports(&pk(&creator.pubkey()));
    env.withdraw_creator_fees(&k, &creator).unwrap();
    assert_eq!(env.lamports(&pk(&creator.pubkey())) + TX_FEE - before, cf);
    assert_eq!(env.lamports(&k.creator_fees), env.rent(&k.creator_fees));

    let ifees = env.lamports(&iface) - env.rent(&iface);
    assert!(ifees > 0);
    expect_err(env.withdraw_interface_fees(&stranger, iface), "ConstraintSeeds");
    let before = env.lamports(&pk(&iface_owner.pubkey()));
    env.withdraw_interface_fees(&iface_owner, iface).unwrap();
    assert_eq!(env.lamports(&pk(&iface_owner.pubkey())) + TX_FEE - before, ifees);
}

#[test]
fn unbonded_launch_uses_unbonded_split() {
    let (mut env, k) = setup(false);
    let u = env.user(20);
    env.deposit(&k, &u, 5 * SOL).unwrap();
    env.set_time(env.launch(&k).close_at);
    let before = fee_vaults(&env, &k, None);
    env.settle(&k, &u).unwrap();
    let (s, curve) = core_settle(&example_params().lobby(), (5 * SOL) as u128).unwrap();
    assert_fee_delta(&before, &fee_vaults(&env, &k, None), &split_fee(s.f, Kind::Unbonded, false, BPS).unwrap(), "stampede");

    let mut m = Market::new(curve, example_params().market(Kind::Unbonded)).unwrap();
    let q = m.buy_exact_in(SOL as u128, 1, false).unwrap();
    let before = fee_vaults(&env, &k, None);
    env.buy_exact_in(&k, &u, SOL, 1, None).unwrap();
    assert_fee_delta(&before, &fee_vaults(&env, &k, None), &split_fee(q.fee, Kind::Unbonded, false, BPS).unwrap(), "buy");
    assert_eq!(split_fee(q.fee, Kind::Unbonded, false, BPS).unwrap().creator_escrow, 0, "unbonded escrows nothing");
    assert_curve(&env.launch(&k), &m, "buy");
}

#[test]
fn lobby_time_and_phase_rules() {
    let (mut env, k) = setup(true);
    let a = env.user(20);
    let b = env.user(20);
    expect_err(env.deposit(&k, &a, SOL / 100 - 1), "BelowMinimum");
    env.deposit(&k, &a, SOL).unwrap();
    env.deposit(&k, &b, 2 * SOL).unwrap();

    // While the Lobby is open: no settlement, no trading, no refund, no claim.
    expect_err(env.settle(&k, &a), "LobbyOpen");
    expect_err(env.buy_exact_in(&k, &a, SOL, 1, None), "TradingClosed");
    expect_err(env.refund(&k, &a), "CannotCancelYet");
    expect_err(env.claim(&k, &a), "NotSettled");

    // Closed but inside the settlement window: no deposits, still no refund.
    let l = env.launch(&k);
    env.set_time(l.close_at);
    expect_err(env.deposit(&k, &a, SOL), "LobbyClosed");
    env.set_time(l.settle_deadline - 1);
    expect_err(env.refund(&k, &a), "CannotCancelYet");

    // Nobody settled in time: settlement is no longer possible and everyone takes the deposit back.
    env.set_time(l.settle_deadline);
    expect_err(env.settle(&k, &a), "SettleDeadlinePassed");
    let vault = env.lamports(&k.sol_vault);
    for (u, d) in [(&a, SOL), (&b, 2 * SOL)] {
        let before = env.lamports(&pk(&u.pubkey()));
        env.refund(&k, u).unwrap();
        assert_eq!(env.lamports(&pk(&u.pubkey())) + TX_FEE - before, d);
        expect_err(env.refund(&k, u), "AlreadyWithdrawn");
    }
    assert_eq!(env.launch(&k).phase, Phase::Cancelled);
    assert_eq!(vault - env.lamports(&k.sol_vault), 3 * SOL);
    assert_eq!(env.lamports(&k.sol_vault), env.rent(&k.sol_vault));
    expect_err(env.settle(&k, &a), "LobbyClosed");
    expect_err(env.claim(&k, &a), "NotSettled");
    expect_err(env.buy_exact_in(&k, &a, SOL, 1, None), "TradingClosed");

    // A wallet that never deposited has nothing to refund.
    let c = env.user(1);
    assert!(env.refund(&k, &c).is_err());
}

#[test]
fn a_lobby_below_the_minimum_opens_no_curve() {
    let (mut env, k) = setup(true);
    let a = env.user(5);
    let b = env.user(5);
    env.deposit(&k, &a, SOL / 2).unwrap();
    env.deposit(&k, &b, SOL / 2 - 1).unwrap(); // one lamport below min_total (1 SOL)
    env.set_time(env.launch(&k).close_at);
    let fees_before = fee_vaults(&env, &k, None);
    env.settle(&k, &a).unwrap();
    let l = env.launch(&k);
    assert_eq!(l.phase, Phase::Cancelled);
    assert_eq!((l.lobby_tokens, l.v, l.t, l.reserve, l.sold), (0, example_params().v0, example_params().t0, 0, 0));
    assert_fee_delta(&fees_before, &fee_vaults(&env, &k, None), &FeeParts::default(), "no fee on a cancelled Lobby");

    // No curve: no trading, no claims; everyone is refunded in full right away.
    expect_err(env.buy_exact_in(&k, &a, SOL, 1, None), "TradingClosed");
    expect_err(env.claim(&k, &a), "NotSettled");
    expect_err(env.settle(&k, &a), "LobbyClosed");
    for (u, d) in [(&a, SOL / 2), (&b, SOL / 2 - 1)] {
        let before = env.lamports(&pk(&u.pubkey()));
        env.refund(&k, u).unwrap();
        assert_eq!(env.lamports(&pk(&u.pubkey())) + TX_FEE - before, d);
    }
    assert_eq!(env.lamports(&k.sol_vault), env.rent(&k.sol_vault));
}

#[test]
fn a_lobby_at_the_minimum_opens_the_curve() {
    let (mut env, k) = setup(true);
    let a = env.user(5);
    env.deposit(&k, &a, SOL).unwrap();
    env.set_time(env.launch(&k).close_at);
    env.settle(&k, &a).unwrap();
    assert_eq!(env.launch(&k).phase, Phase::Curve);
}

#[test]
fn slippage_and_bad_amounts() {
    let (mut env, k) = setup(true);
    let u = env.user(100);
    env.deposit(&k, &u, SOL).unwrap();
    env.set_time(env.launch(&k).close_at);
    env.settle(&k, &u).unwrap();
    let l = env.launch(&k);
    let m = Market::new(l.curve(), example_params().market(Kind::Bonded)).unwrap();

    let q = m.quote_buy_exact_in(SOL as u128).unwrap();
    expect_err(env.buy_exact_in(&k, &u, SOL, (q.tokens + 1) as u64, None), "Slippage");
    expect_err(env.buy_exact_in(&k, &u, 0, 0, None), "Slippage");
    let q = m.quote_buy_exact_out((1_000_000 * TOKEN) as u128).unwrap();
    expect_err(env.buy_exact_out(&k, &u, 1_000_000 * TOKEN, (q.gross - 1) as u64, None), "Slippage");
    expect_err(env.buy_exact_out(&k, &u, 0, u64::MAX, None), "ZeroAmount");
    env.buy_exact_out(&k, &u, 1_000_000 * TOKEN, q.gross as u64, None).unwrap();

    let m = Market::new(env.launch(&k).curve(), example_params().market(Kind::Bonded)).unwrap();
    let s = m.quote_sell((500_000 * TOKEN) as u128).unwrap();
    expect_err(env.sell(&k, &u, 500_000 * TOKEN, (s.received + 1) as u64, None), "Slippage");
    expect_err(env.sell(&k, &u, 0, 0, None), "OutOfRange");
    // Selling more than the wallet holds fails in the token transfer.
    assert!(env.sell(&k, &u, 2_000_000 * TOKEN, 0, None).is_err());
    env.sell(&k, &u, 500_000 * TOKEN, s.received as u64, None).unwrap();
    assert_eq!(env.launch(&k).curve(), {
        let mut m = m;
        m.sell((500_000 * TOKEN) as u128, 0, false).unwrap();
        m.curve
    });
}

#[test]
fn wrong_accounts_are_rejected() {
    let (mut env, a) = setup(true);
    let b = env.create_launch(true, 60).unwrap();
    let u = env.user(100);
    let v = env.user(100);
    for k in [&a, &b] {
        env.deposit(k, &u, SOL).unwrap();
        env.deposit(k, &v, SOL).unwrap();
    }
    env.set_time(env.launch(&a).close_at);
    env.settle(&a, &u).unwrap();
    env.settle(&b, &u).unwrap();

    // The untouched transaction goes through (and opens the trader's token account).
    let ix = env.buy_exact_in_ix(&a, &u, SOL, 1, None);
    env.send(ix, &[&u]).unwrap();

    // Swap one account of a trade on A for B's (or a look-alike) and expect a constraint failure.
    let swaps: [(usize, anchor_lang::prelude::Pubkey, &str); 8] = [
        (2, b.mint, "ConstraintTokenMint"),
        (3, b.sol_vault, "ConstraintSeeds"),
        (4, b.token_vault, "ConstraintTokenOwner"),
        (6, b.creator_fees, "ConstraintSeeds"),
        (7, b.creator_escrow, "ConstraintSeeds"),
        (8, b.creator_escrow, "ConstraintSeeds"),
        (9, b.sol_vault, "ConstraintSeeds"),
        (9, pda(&[REKT_BACK_SEED]), "ConstraintDuplicateMutableAccount"),
    ];
    for (i, key, code) in swaps {
        let mut ix = env.buy_exact_in_ix(&a, &u, SOL, 1, None);
        ix.accounts[i].pubkey = addr(&key);
        expect_err(env.send(ix, &[&u]), code);
    }
    // A vault is not a trading interface.
    let mut ix = env.buy_exact_in_ix(&a, &u, SOL, 1, None);
    ix.accounts[10].pubkey = addr(&a.creator_fees);
    ix.accounts[10].is_writable = true;
    expect_err(env.send(ix, &[&u]), "AccountDiscriminatorMismatch");

    // Settling A with B's vaults fails.
    let mut ix = env.ix(
        anchor_lang::ToAccountMetas::to_account_metas(
            &herdcall::accounts::Settle {
                launch: a.launch,
                sol_vault: b.sol_vault,
                creator_fees: a.creator_fees,
                creator_escrow: a.creator_escrow,
                rekt_back: pda(&[REKT_BACK_SEED]),
                protocol: pda(&[PROTOCOL_SEED]),
                revenue: pda(&[REVENUE_SEED]),
                ticker_record: a.ticker_record,
                name_record: a.name_record,
                creator: pk(&a.creator.pubkey()),
            },
            None,
        ),
        anchor_lang::InstructionData::data(&herdcall::instruction::Settle {}),
    );
    ix.accounts[0].pubkey = addr(&a.launch);
    expect_err(env.send(ix, &[&u]), "ConstraintSeeds");

    // Claiming with another wallet's deposit record fails; so does claiming B's tokens with an A deposit.
    let mut ix = claim_ix(&env, &a, &u);
    ix.accounts[3].pubkey = addr(&pda(&[DEPOSIT_SEED, a.launch.as_ref(), pk(&v.pubkey()).as_ref()]));
    expect_err(env.send(ix, &[&u]), "ConstraintSeeds");
    let mut ix = claim_ix(&env, &a, &u);
    ix.accounts[6].pubkey = addr(&b.token_vault);
    expect_err(env.send(ix, &[&u]), "ConstraintTokenOwner");
    // The breakout of a token that is still on the curve fails.
    expect_err(env.breakout(&a, &u), "NotBreakingOut");
}

fn claim_ix(env: &Env, k: &LaunchKeys, user: &Keypair) -> solana_instruction::Instruction {
    let u = pk(&user.pubkey());
    let metas = anchor_lang::ToAccountMetas::to_account_metas(
        &herdcall::accounts::Claim {
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
        },
        None,
    );
    env.ix(metas, anchor_lang::InstructionData::data(&herdcall::instruction::Claim {}))
}

#[test]
fn caps_and_setup_rules() {
    let mut env = Env::new();
    let mut bad = example_params();
    bad.lp_supply += 1;
    expect_err(env.initialize(bad), "InvalidParams");
    let mut p = example_params();
    p.max_total = 3 * SOL;
    p.max_per_wallet = 2 * SOL;
    env.initialize(p).unwrap();
    assert!(env.initialize(p).is_err(), "initialize only once");

    expect_err(env.create_launch(true, 0).map(|_| ()), "InvalidParams");
    expect_err(env.create_launch(true, MAX_LOBBY_SECS + 1).map(|_| ()), "InvalidParams");
    let k = env.create_launch(true, MAX_LOBBY_SECS).unwrap();
    let a = env.user(20);
    let b = env.user(20);
    env.deposit(&k, &a, 2 * SOL).unwrap();
    expect_err(env.deposit(&k, &a, SOL / 100), "WalletCap");
    expect_err(env.deposit(&k, &b, SOL + 1), "LobbyCap");
    env.deposit(&k, &b, SOL).unwrap();
    assert_eq!(env.launch(&k).total_deposits, 3 * SOL);
}

/// If the books were ever wrong, a payout fails with VaultUnderfunded instead of draining the vault below rent.
#[test]
fn vault_never_pays_below_rent() {
    let (mut env, k) = setup(true);
    let u = env.user(100);
    env.deposit(&k, &u, SOL).unwrap();
    env.set_time(env.launch(&k).close_at);
    env.settle(&k, &u).unwrap();
    env.buy_exact_in(&k, &u, 10 * SOL, 1, None).unwrap();
    // Take lamports out of the vault behind the program's back: the sell can still be paid, but only out of the rent.
    let tokens = env.token_balance(&ata(&pk(&u.pubkey()), &k.mint)) / 2;
    let out = Market::new(env.launch(&k).curve(), example_params().market(Kind::Bonded)).unwrap().quote_sell(tokens as u128).unwrap().out;
    let mut acc = env.svm.get_account(&addr(&k.sol_vault)).unwrap();
    acc.lamports = out as u64 + env.rent(&k.sol_vault) - 1;
    env.svm.set_account(addr(&k.sol_vault), acc).unwrap();
    expect_err(env.sell(&k, &u, tokens, 0, None), "VaultUnderfunded");
}
