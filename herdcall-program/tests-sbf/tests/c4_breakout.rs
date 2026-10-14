//! C4 regression: `breakout` must execute on the real SBF runtime with the real DAMM program,
//! conserve lamports, lock the LP permanently and roll back completely on failure.
use herdcall_sbf_tests::damm;
use herdcall_sbf_tests::hc::{self, state};
use herdcall_sbf_tests::scenario::*;
use solana_keypair::Keypair;
use solana_signer::Signer;

#[test]
fn breakout_succeeds_conserves_lamports_and_locks_lp() {
    for p in PROFILES {
        let mut w = World::new(p, ConfigKind::Public);
        w.fill_curve();
        let (liq, sqrt, lp, wsol) = w.breakout_plan();
        let nft = Keypair::new();
        let (ix, d) = w.breakout_ix(&nft, liq, sqrt);
        let keys = tx_keys(&ix);
        let l0 = w.launch();
        let launch0 = lamports(&w.svm, &w.launch_key());
        let total0 = total_lamports(&w.svm, &keys);
        let proto0 = lamports(&w.svm, &hc::protocol_pda());
        let supply0 = mint_info(&w.svm, &w.mint()).supply;

        let payer = w.payer.insecure_clone();
        let r = send(&mut w.svm, &payer, &[ix], &[&nft]);
        println!("REPORT c4.breakout profile={p:?} {}", describe(&r));
        let m = ok(r, "breakout");

        // state
        let l1 = w.launch();
        assert_eq!(l1.state, state::BREAKOUT);
        assert_eq!(l1.dex_pool, d.pool.to_bytes());
        assert_eq!(l1.dex_position, d.position.to_bytes());
        // launch PDA lost exactly the planned sweep (refund buffer + unclaimed refunds + burn_reserve stay)
        assert_eq!(lamports(&w.svm, &w.launch_key()), launch0 - l0.breakout_sweep_sol());
        // lamport conservation over every account of the transaction (only the tx fee leaves)
        assert_eq!(total_lamports(&w.svm, &keys) + m.fee as u128, total0, "lamports not conserved");
        assert!(lamports(&w.svm, &hc::protocol_pda()) > proto0, "leftover must reach protocol pool");
        // pool got >= 99% of plan, pool is ours
        let pool = damm::read_pool(&w.svm, &d.pool);
        assert_eq!(pool.token_a_mint.to_bytes(), w.mint().to_bytes());
        assert_eq!(pool.token_b_mint.to_bytes(), hc::WSOL.to_bytes());
        let (in_a, in_b) = (token_amount(&w.svm, &d.token_a_vault), token_amount(&w.svm, &d.token_b_vault));
        assert!(in_a as u128 * 10_000 >= lp as u128 * 9_900 && in_b as u128 * 10_000 >= wsol as u128 * 9_900, "fill {in_a}/{lp} {in_b}/{wsol}");
        // LP permanently locked, NFT held by the program PDA
        let pos = damm::read_position(&w.svm, &d.position);
        assert_eq!(pos.unlocked_liquidity, 0);
        assert_eq!(pos.permanent_locked_liquidity, liq);
        let nft_acc = w.svm.get_account(&d.position_nft_account).unwrap();
        assert_eq!(&nft_acc.data[32..64], hc::breakout_auth_pda(&w.launch_key()).as_ref(), "NFT owner");
        // mint authority revoked, supply = before + LP - burnt leftover
        let mi = mint_info(&w.svm, &w.mint());
        assert!(mi.mint_authority.is_none());
        assert_eq!(mi.supply, supply0 + in_a);
        println!("REPORT c4.breakout profile={p:?} cu={} sol_in_pool={in_b} tokens_in_pool={in_a} liquidity={liq}", m.compute_units_consumed);
    }
}

/// A breakout whose liquidity does not fill >= 99% of the plan must fail with BreakoutPoolMismatch
/// AFTER all SOL/token moves and DAMM CPIs, and everything must roll back.
#[test]
fn breakout_underfilled_rolls_back_everything() {
    for p in PROFILES {
        let mut w = World::new(p, ConfigKind::Public);
        w.fill_curve();
        let (liq, sqrt, _, _) = w.breakout_plan();
        let nft = Keypair::new();
        let (ix, _) = w.breakout_ix(&nft, liq / 2, sqrt);
        let mut keys = tx_keys(&ix);
        keys.retain(|k| *k != w.payer.pubkey());
        let before = snapshot(&w.svm, &keys);
        let l0 = w.launch();
        let payer = w.payer.insecure_clone();
        let r = send(&mut w.svm, &payer, &[ix], &[&nft]);
        println!("REPORT c4.breakout_rollback profile={p:?} {}", describe(&r));
        let e = r.expect_err("under-filled breakout must fail");
        assert_eq!(
            e.err,
            solana_transaction_error::TransactionError::InstructionError(1, solana_instruction::error::InstructionError::Custom(6016)),
            "expected BreakoutPoolMismatch (6016)"
        );
        assert_eq!(snapshot(&w.svm, &keys), before, "state not rolled back");
        assert_eq!(w.launch(), l0);
    }
}
