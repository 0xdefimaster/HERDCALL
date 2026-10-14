//! C1/C2 on the real SBF runtime: Stampede claims (tokens + cap refund + fee netted from refund)
//! work during Curve and after a (simulated) breakout, and the launch PDA stays rent exempt.
use herdcall_sbf_tests::hc;
use herdcall_sbf_tests::scenario::*;
use solana_signer::Signer;

#[test]
fn every_depositor_can_claim_before_and_after_breakout() {
    for p in PROFILES {
        let mut w = World::new(p, ConfigKind::Public);
        let r = w.claim(0);
        println!("REPORT claims.curve profile={p:?} {}", describe(&r));
        ok(r, "claim during Curve");
        w.fill_curve();
        w.simulate_breakout(100_000_000 * 1_000_000, 10 * hc::SOL);
        for i in 1..w.users.len() {
            let u = w.users[i].0.pubkey();
            let before = lamports(&w.svm, &u);
            let r = w.claim(i);
            println!("REPORT claims.after_breakout profile={p:?} user={i} {}", describe(&r));
            ok(r, "claim after breakout");
            assert!(token_amount(&w.svm, &hc::ata(&u, &w.mint())) > 0);
            assert!(lamports(&w.svm, &u) > before - 3_000_000, "refund paid");
            assert!(w.svm.get_account(&hc::order_pda(&w.launch_key(), &u)).map(|a| a.lamports == 0).unwrap_or(true));
        }
        let rent = w.svm.minimum_balance_for_rent_exemption(hc::LAUNCH_SPACE);
        assert!(lamports(&w.svm, &w.launch_key()) >= rent);
    }
}
