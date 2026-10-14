//! C4 regression: `buyback_burn` moves SOL out of the launch PDA and then CPIs; it must execute on
//! the real runtime, conserve lamports and roll back on a failed swap.
//! Uses a SIMULATED breakout (see `World::simulate_breakout`) so it does not depend on `breakout`.
use herdcall_sbf_tests::hc;
use herdcall_sbf_tests::scenario::*;
use solana_address::Address;
use solana_signer::Signer;

const POOL_TOKENS: u64 = 100_000_000 * 1_000_000;
const POOL_SOL: u64 = 10 * hc::SOL;

fn setup(p: Profile) -> (World, SimPool) {
    let mut w = World::new(p, ConfigKind::Public);
    w.fill_curve();
    let sim = w.simulate_breakout(POOL_TOKENS, POOL_SOL);
    (w, sim)
}

fn keys(w: &World, s: &SimPool) -> Vec<Address> {
    let auth = hc::breakout_auth_pda(&w.launch_key());
    let mut k = vec![
        w.payer.pubkey(), w.launch_key(), w.mint(), auth, hc::ata(&auth, &w.mint()), hc::ata(&auth, &hc::WSOL),
        s.pool, s.a_vault, s.b_vault,
    ];
    k.sort();
    k.dedup();
    k
}

#[test]
fn buyback_burn_succeeds_and_conserves_lamports() {
    for p in PROFILES {
        let (mut w, s) = setup(p);
        // DAMM's real config starts with a decaying anti-sniper fee (fee scheduler); buyback's
        // min_out (spot - 10%) only clears once the fee has decayed.
        warp_secs(&mut w.svm, 3 * 3600);
        let l0 = w.launch();
        assert!(l0.burn_reserve >= 20_000_000, "burn_reserve {}", l0.burn_reserve);
        let amount = 20_000_000u64; // 0.02 SOL (>= 0.01 min, <= 1% of 10 SOL pool)
        let k = keys(&w, &s);
        let total0 = total_lamports(&w.svm, &k);
        let launch0 = lamports(&w.svm, &w.launch_key());
        let vault_b0 = token_amount(&w.svm, &s.b_vault);
        let supply0 = mint_info(&w.svm, &w.mint()).supply;

        let payer = w.payer.insecure_clone();
        let ix = hc::ix_buyback_burn(&payer.pubkey(), &w.mint(), &s.pool, &s.a_vault, &s.b_vault, amount);
        let r = send(&mut w.svm, &payer, &[ix], &[]);
        println!("REPORT c4.buyback profile={p:?} {}", describe(&r));
        let m = ok(r, "buyback_burn");

        let l1 = w.launch();
        assert_eq!(l1.burn_reserve, l0.burn_reserve - amount);
        assert!(l1.total_bought_back > 0);
        assert_eq!(lamports(&w.svm, &w.launch_key()), launch0 - amount);
        assert_eq!(token_amount(&w.svm, &s.b_vault), vault_b0 + amount, "all SOL reached the pool vault");
        assert_eq!(mint_info(&w.svm, &w.mint()).supply, supply0 - l1.total_bought_back, "bought tokens burnt");
        let auth = hc::breakout_auth_pda(&w.launch_key());
        assert!(w.svm.get_account(&hc::ata(&auth, &hc::WSOL)).map(|a| a.lamports == 0).unwrap_or(true), "temp wSOL closed");
        assert_eq!(total_lamports(&w.svm, &k) + m.fee as u128, total0, "lamports not conserved");
        println!("REPORT c4.buyback profile={p:?} bought={} cu={}", l1.total_bought_back, m.compute_units_consumed);
    }
}

/// Right after pool creation the real config's fee scheduler charges ~50%: DAMM rejects the swap
/// against buyback's on-chain min_out. The SOL already moved out of the launch PDA must come back.
#[test]
fn buyback_burn_failed_swap_rolls_back() {
    for p in PROFILES {
        let (mut w, s) = setup(p);
        let mut k = keys(&w, &s);
        k.retain(|x| *x != w.payer.pubkey());
        let before = snapshot(&w.svm, &k);
        let payer = w.payer.insecure_clone();
        let ix = hc::ix_buyback_burn(&payer.pubkey(), &w.mint(), &s.pool, &s.a_vault, &s.b_vault, 20_000_000);
        let r = send(&mut w.svm, &payer, &[ix], &[]);
        println!("REPORT c4.buyback_rollback profile={p:?} {}", describe(&r));
        let e = r.expect_err("swap must fail while the anti-sniper fee is high");
        let logs = e.meta.logs.join("\n");
        assert!(
            logs.contains("Program cpamdpZCGKUy5JxQXB4dcpGPiikHawvSWAd6mEn1sGG failed"),
            "failure must come from the DAMM swap (slippage), not from herdcall's lamport handling:\n{logs}"
        );
        assert_eq!(snapshot(&w.svm, &k), before, "state not rolled back");
    }
}
