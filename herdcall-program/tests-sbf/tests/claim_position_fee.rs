//! `claim_position_fee` on the real runtime against real DAMM fees (simulated breakout pool whose
//! position NFT is owned by the launch's breakout_authority PDA): exact 60/20/20 split.
use herdcall_sbf_tests::{damm, hc};
use herdcall_sbf_tests::scenario::*;
use solana_signer::Signer;

#[test]
fn lp_fees_are_split_60_20_20() {
    for p in PROFILES {
        let mut w = World::new(p, ConfigKind::Public);
        w.fill_curve();
        let s = w.simulate_breakout(100_000_000 * 1_000_000, 10 * hc::SOL);
        warp_secs(&mut w.svm, 3 * 3600);
        // generate fees with real swaps (both directions)
        let buyer = w.buyer.insecure_clone();
        let wsol = w.ensure_wsol(&buyer, 5 * hc::SOL);
        let tok = hc::ata(&buyer.pubkey(), &w.mint());
        let m = w.mint();
        ok(send(&mut w.svm, &buyer, &[damm::ix_swap(&buyer.pubkey(), &s.pool, &wsol, &tok, &s.a_vault, &s.b_vault, &m, &hc::WSOL, 2 * hc::SOL)], &[]), "swap B->A");
        ok(send(&mut w.svm, &buyer, &[damm::ix_swap(&buyer.pubkey(), &s.pool, &tok, &wsol, &s.a_vault, &s.b_vault, &m, &hc::WSOL, 10_000_000 * 1_000_000)], &[]), "swap A->B");

        let (pp0, dp0, l0) = (lamports(&w.svm, &hc::protocol_pda()), lamports(&w.svm, &hc::dao_pda()), w.launch());
        let launch_lamports0 = lamports(&w.svm, &w.launch_key());
        let payer = w.payer.insecure_clone();
        let ix = hc::ix_claim_position_fee(&payer.pubkey(), &w.mint(), &w.creator.pubkey(), &s.pool, &s.position, &s.a_vault, &s.b_vault, &s.nft_mint);
        let r = send(&mut w.svm, &payer, &[ix], &[]);
        println!("REPORT claim_position_fee profile={p:?} {}", describe(&r));
        ok(r, "claim_position_fee");
        let (pp, dp) = (lamports(&w.svm, &hc::protocol_pda()) - pp0, lamports(&w.svm, &hc::dao_pda()) - dp0);
        let creator_burn = w.launch().burn_reserve - l0.burn_reserve; // creator not eligible -> all to burn
        assert_eq!(lamports(&w.svm, &w.launch_key()) - launch_lamports0, creator_burn);
        let total = pp + dp + creator_burn;
        assert!(total > 0, "no fees claimed");
        assert_eq!(creator_burn, total * 6_000 / 10_000);
        assert_eq!(dp, total * 2_000 / 10_000);
        assert_eq!(pp, total - creator_burn - dp);
        println!("REPORT claim_position_fee profile={p:?} sol_fee_total={total} creator->burn={creator_burn} protocol={pp} dao={dp}");
    }
}
