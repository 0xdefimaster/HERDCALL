//! C5 regression: an attacker must not be able to block `breakout` by creating the canonical DAMM
//! pool (same config + mint pair) first. With a PUBLIC config the attacker needs only wSOL:
//! sqrt_price = config.sqrt_max_price makes the token-A side zero.
use herdcall_sbf_tests::{damm, hc};
use herdcall_sbf_tests::scenario::*;
use solana_keypair::Keypair;
use solana_signer::Signer;

#[test]
fn attacker_precreated_pool_must_not_block_breakout() {
    for p in PROFILES {
        let mut w = World::new(p, ConfigKind::Public);
        let attacker = funded(&mut w.svm, 20 * hc::SOL);
        let wsol = w.ensure_wsol(&attacker, 10 * hc::SOL);
        let m = w.mint();
        ok(send(&mut w.svm, &attacker, &[
            spl_associated_token_account_interface::instruction::create_associated_token_account_idempotent(
                &attacker.pubkey(), &attacker.pubkey(), &m, &hc::TOKEN),
        ], &[]), "attacker token ATA");
        let cfg = damm::decode_config(&w.svm.get_account(&w.damm_config).unwrap().data);
        let liq = damm::max_liquidity(&cfg, cfg.sqrt_max_price, 0, 10 * hc::SOL);
        let nft = Keypair::new();
        let ix = damm::ix_initialize_pool(&attacker.pubkey(), &attacker.pubkey(), &nft.pubkey(), &w.damm_config, &m, &hc::WSOL,
            &hc::ata(&attacker.pubkey(), &m), &wsol, liq, cfg.sqrt_max_price);
        let r = send(&mut w.svm, &attacker, &[ix], &[&nft]);
        println!("REPORT c5.attacker_initialize_pool profile={p:?} {} (tokens needed: 0, wSOL spent: {})",
            describe(&r), 10 * hc::SOL - token_amount(&w.svm, &wsol));

        w.fill_curve();
        let (liq, sqrt, _, _) = w.breakout_plan();
        let nft = Keypair::new();
        let (ix, _) = w.breakout_ix(&nft, liq, sqrt);
        let payer = w.payer.insecure_clone();
        let r = send(&mut w.svm, &payer, &[ix], &[&nft]);
        println!("REPORT c5.breakout_after_attack profile={p:?} {}", describe(&r));
        ok(r, "breakout after attacker pre-created the pool");
        assert_eq!(w.launch().state, hc::state::BREAKOUT);
    }
}
