//! Replays vectors exported from the JS reference model (sim/vectors.js) and checks that the Rust core gives
//! exactly the same results, unit for unit. Regenerate with `npm run vectors`.
use herdcall_core::curve::{Market, MarketParams, Phase};
use herdcall_core::fees::{split_fee, split_lp_fees, CreatorLock, FeeParts, Kind};
use herdcall_core::stampede::{claim_tokens, settle, settle_math, LobbyParams};
use herdcall_core::{Error, BPS};
use serde_json::Value;

fn vectors() -> Value {
    let raw = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/vectors.json"))
        .expect("vectors.json");
    serde_json::from_str(&raw).expect("valid json")
}

fn u(v: &Value) -> u128 {
    v.as_str()
        .unwrap_or_else(|| panic!("expected a number string, got {v}"))
        .parse()
        .unwrap()
}

fn kind(v: &Value) -> Kind {
    match v.as_str().unwrap() {
        "bonded" => Kind::Bonded,
        "unbonded" => Kind::Unbonded,
        k => panic!("unknown kind {k}"),
    }
}

fn lobby_params(p: &Value) -> LobbyParams {
    LobbyParams {
        v0: u(&p["v0"]),
        t0: u(&p["t0"]),
        curve_supply: u(&p["curveSupply"]),
        fee_bps: u(&p["feeBps"]),
        min_deposit: u(&p["minDeposit"]),
        min_total: u(&p["minTotal"]),
    }
}

fn market_params(p: &Value) -> MarketParams {
    MarketParams {
        v0: u(&p["v0"]),
        t0: u(&p["t0"]),
        total_supply: u(&p["totalSupply"]),
        curve_supply: u(&p["curveSupply"]),
        lp_supply: u(&p["lpSupply"]),
        fee_bps: u(&p["feeBps"]),
        migration_fee: u(&p["migrationFee"]),
        kind: Kind::Bonded,
    }
}

fn assert_fee_parts(got: &FeeParts, want: &Value, ctx: &str) {
    assert_eq!(got.creator_now, u(&want["creatorNow"]), "{ctx}: creatorNow");
    assert_eq!(
        got.creator_escrow,
        u(&want["creatorEscrow"]),
        "{ctx}: creatorEscrow"
    );
    assert_eq!(got.interface, u(&want["interface"]), "{ctx}: interface");
    assert_eq!(got.rekt_back, u(&want["rektBack"]), "{ctx}: rektBack");
    assert_eq!(got.protocol, u(&want["protocol"]), "{ctx}: protocol");
}

fn assert_state(m: &Market, want: &Value, ctx: &str) {
    assert_eq!(m.curve.v, u(&want["v"]), "{ctx}: v");
    assert_eq!(m.curve.t, u(&want["t"]), "{ctx}: t");
    assert_eq!(m.curve.k, u(&want["k"]), "{ctx}: k");
    assert_eq!(m.curve.reserve, u(&want["reserve"]), "{ctx}: reserve");
    assert_eq!(m.curve.sold, u(&want["sold"]), "{ctx}: sold");
    let phase = match m.phase {
        Phase::Curve => "Curve",
        Phase::BreakingOut => "BreakingOut",
        Phase::Migrated => "Migrated",
    };
    assert_eq!(phase, want["phase"].as_str().unwrap(), "{ctx}: phase");
    assert_fee_parts(&m.fees, &want["fees"], ctx);
    assert_eq!(m.fee_total, u(&want["feeTotal"]), "{ctx}: feeTotal");
}

// JS error messages ↔ Rust errors.
fn js_error(e: Error) -> &'static str {
    match e {
        Error::Slippage => "slippage",
        Error::TradingClosed => "trading closed",
        Error::OutOfRange => "dt out of range",
        Error::Insolvent => "insolvent",
        Error::ZeroAmount => "amount must be positive",
        other => panic!("unexpected error {other:?}"),
    }
}

#[test]
fn stampede_settlement_matches_js() {
    let v = vectors();
    let p = lobby_params(&v["params"]);
    for (i, c) in v["settle"].as_array().unwrap().iter().enumerate() {
        let s = settle_math(&p, u(&c["total"])).unwrap();
        assert_eq!(s.empty, c["empty"].as_bool().unwrap(), "settle {i}: empty");
        assert_eq!(s.x, u(&c["X"]), "settle {i}: X");
        assert_eq!(s.n, u(&c["N"]), "settle {i}: N");
        assert_eq!(s.f, u(&c["F"]), "settle {i}: F");
        assert_eq!(s.refund, u(&c["refund"]), "settle {i}: refund");
    }
}

#[test]
fn claims_match_js_and_never_exceed_the_allocation() {
    let v = vectors();
    for (i, c) in v["claims"].as_array().unwrap().iter().enumerate() {
        let (total, x) = (u(&c["total"]), u(&c["x"]));
        let mut paid = 0;
        for (d, want) in c["deposits"]
            .as_array()
            .unwrap()
            .iter()
            .zip(c["tokens"].as_array().unwrap())
        {
            let q = claim_tokens(u(d), total, x).unwrap();
            assert_eq!(q, u(want), "claim {i}");
            paid += q;
        }
        assert!(paid <= x, "claim {i}: paid more than X");
    }
}

#[test]
fn fee_splits_match_js() {
    let v = vectors();
    for (i, c) in v["fees"].as_array().unwrap().iter().enumerate() {
        let got = split_fee(
            u(&c["fee"]),
            kind(&c["kind"]),
            c["hasInterface"].as_bool().unwrap(),
            u(&c["lockRatioBps"]),
        )
        .unwrap();
        assert_fee_parts(&got, &c["out"], &format!("fee {i}"));
        assert_eq!(got.total().unwrap(), u(&c["fee"]), "fee {i}: parts add up");
    }
    for (i, c) in v["lpFees"].as_array().unwrap().iter().enumerate() {
        let got = split_lp_fees(u(&c["fee"]), kind(&c["kind"]), u(&c["lockRatioBps"])).unwrap();
        let w = &c["out"];
        assert_eq!(got.liquidity, u(&w["liquidity"]), "lp {i}: liquidity");
        assert_eq!(got.creator, u(&w["creator"]), "lp {i}: creator");
        assert_eq!(got.rekt_back, u(&w["rektBack"]), "lp {i}: rektBack");
        assert_eq!(got.protocol, u(&w["protocol"]), "lp {i}: protocol");
        assert_eq!(got.forfeited, u(&w["forfeited"]), "lp {i}: forfeited");
    }
}

#[test]
fn creator_lock_matches_js() {
    let v = vectors();
    for (i, c) in v["locks"].as_array().unwrap().iter().enumerate() {
        let mut lock = CreatorLock::new(u(&c["allocation"]));
        for (upd, want) in c["updates"]
            .as_array()
            .unwrap()
            .iter()
            .zip(c["ratios"].as_array().unwrap())
        {
            let r = lock.update(u(upd)).unwrap();
            assert_eq!(r, u(want), "lock {i}");
            assert!(r <= BPS);
        }
    }
}

#[test]
fn markets_replay_identically() {
    let v = vectors();
    let lp = lobby_params(&v["params"]);
    let mp = market_params(&v["params"]);
    let mut breakouts = 0;
    for (i, mv) in v["markets"].as_array().unwrap().iter().enumerate() {
        let (s, curve) = settle(&lp, u(&mv["pool"])).unwrap();
        assert_eq!(s.x, u(&mv["settle"]["X"]), "market {i}: settle X");
        let mut m = Market::new(curve, mp).unwrap();
        if !s.empty {
            m.route(s.f, true).unwrap();
        }
        assert_state(&m, &mv["start"], &format!("market {i} start"));
        for (j, op) in mv["ops"].as_array().unwrap().iter().enumerate() {
            let ctx = format!("market {i} op {j} ({})", op["op"].as_str().unwrap());
            let iface = op["hasInterface"].as_bool().unwrap();
            let ok = op["ok"].as_bool().unwrap();
            match op["op"].as_str().unwrap() {
                "buyExactIn" => match m.buy_exact_in(u(&op["gross"]), u(&op["minTokens"]), iface) {
                    Ok(q) => {
                        assert!(ok, "{ctx}: JS failed, Rust succeeded");
                        let r = &op["result"];
                        assert_eq!(q.tokens, u(&r["tokens"]), "{ctx}: tokens");
                        assert_eq!(q.cost, u(&r["cost"]), "{ctx}: cost");
                        assert_eq!(q.fee, u(&r["fee"]), "{ctx}: fee");
                        assert_eq!(q.refund, u(&r["refund"]), "{ctx}: refund");
                        assert_eq!(q.clamped, r["clamped"].as_bool().unwrap(), "{ctx}: clamped");
                    }
                    Err(e) => {
                        assert!(!ok, "{ctx}: Rust failed with {e:?}, JS succeeded");
                        assert!(
                            op["error"].as_str().unwrap().starts_with(js_error(e)),
                            "{ctx}: error"
                        );
                    }
                },
                "buyExactOut" => match m.buy_exact_out(u(&op["tokens"]), u(&op["maxGross"]), iface)
                {
                    Ok(q) => {
                        assert!(ok, "{ctx}: JS failed, Rust succeeded");
                        let r = &op["result"];
                        assert_eq!(q.tokens, u(&r["tokens"]), "{ctx}: tokens");
                        assert_eq!(q.gross, u(&r["gross"]), "{ctx}: gross");
                        assert_eq!(q.clamped, r["clamped"].as_bool().unwrap(), "{ctx}: clamped");
                    }
                    Err(e) => {
                        assert!(!ok, "{ctx}: Rust failed with {e:?}, JS succeeded");
                        assert!(
                            op["error"].as_str().unwrap().starts_with(js_error(e)),
                            "{ctx}: error"
                        );
                    }
                },
                "sell" => match m.sell(u(&op["tokens"]), u(&op["minReceived"]), iface) {
                    Ok(q) => {
                        assert!(ok, "{ctx}: JS failed, Rust succeeded");
                        let r = &op["result"];
                        assert_eq!(q.out, u(&r["out"]), "{ctx}: out");
                        assert_eq!(q.received, u(&r["received"]), "{ctx}: received");
                    }
                    Err(e) => {
                        assert!(!ok, "{ctx}: Rust failed with {e:?}, JS succeeded");
                        assert!(
                            op["error"].as_str().unwrap().starts_with(js_error(e)),
                            "{ctx}: error"
                        );
                    }
                },
                other => panic!("unknown op {other}"),
            }
            assert_state(&m, &op["state"], &ctx);
            // Solvency after every step.
            assert!(
                m.curve.reserve >= m.curve.sell_all_payout().unwrap(),
                "{ctx}: solvency"
            );
        }
        if !mv["breakout"].is_null() {
            breakouts += 1;
            let b = m.breakout().unwrap();
            let w = &mv["breakout"];
            assert_eq!(b.keeper_fee, u(&w["keeperFee"]), "market {i}: keeper fee");
            assert_eq!(b.lp_native, u(&w["lpNative"]), "market {i}: lp native");
            assert_eq!(b.lp_tokens, u(&w["lpTokens"]), "market {i}: lp tokens");
            assert_eq!(b.burned, u(&w["burned"]), "market {i}: burned");
            assert_eq!(
                m.breakout(),
                Err(Error::NotBreakingOut),
                "market {i}: breakout only once"
            );
        }
        assert_state(&m, &mv["end"], &format!("market {i} end"));
    }
    assert!(breakouts > 0, "some markets reach breakout");
}
