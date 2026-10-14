//! ProtocolPool SOL distribution (50/30/20) and wallet management.
mod common;

use anchor_lang::error::ErrorCode as AnchorError;
use common::*;
use herdcall::errors::HerdError;
use herdcall::state::*;
use solana_sdk::{pubkey::Pubkey, signature::{Keypair, Signer}};

/// Real fee flow: lobby -> settle -> buys -> claims put SOL in ProtocolPool.
async fn pool_with_fees(env: &mut Env) {
    init_config(env).await.unwrap();
    let creator = env.funded(10 * SOL).await;
    let mint = Keypair::new();
    env.send(&[ix_create_launch(&creator.pubkey(), &mint.pubkey())], &[&creator, &mint]).await.unwrap();
    let u = env.funded(5 * SOL).await;
    env.send(&[ix_deposit(&u.pubkey(), &mint.pubkey(), 3 * SOL / 2)], &[&u]).await.unwrap();
    env.warp_secs(20).await;
    let payer = env.payer();
    env.send(&[ix_settle(&payer.pubkey(), &mint.pubkey())], &[]).await.unwrap();
    env.send(&[ix_claim(&u.pubkey(), &mint.pubkey())], &[&u]).await.unwrap();
    let buyer = env.funded(50 * SOL).await;
    let ui = env.funded(SOL).await;
    for amt in [SOL, 2 * SOL, 7 * SOL / 3] {
        env.send(&[ix_buy(&buyer.pubkey(), &mint.pubkey(), &creator.pubkey(), &ui.pubkey(), amt)], &[&buyer])
            .await
            .unwrap();
    }
}

async fn pool_rent(env: &mut Env) -> u64 {
    env.rent_min(8 + <ProtocolPool as anchor_lang::Space>::INIT_SPACE).await
}

#[tokio::test]
async fn distributes_50_30_20_and_keeps_rent() {
    let mut env = start().await;
    pool_with_fees(&mut env).await;

    let rent = pool_rent(&mut env).await;
    let pool0 = env.lamports(&protocol_pda()).await;
    let amount = pool0 - rent;
    assert!(amount > 0, "fees must have accrued");
    let (v0, b0, o0) = (
        env.lamports(&vault_pda()).await,
        env.lamports(&env.builders.clone()).await,
        env.lamports(&env.ops.clone()).await,
    );

    let admin = env.admin.insecure_clone();
    let (b, o) = (env.builders, env.ops);
    env.send(&[ix_distribute(&admin.pubkey(), &b, &o, amount)], &[&admin]).await.unwrap();

    let (ev, eb, eo) = herdcall::fees::split_protocol_surplus(amount).unwrap();
    assert_eq!(ev + eb + eo, amount);
    assert_eq!(env.lamports(&vault_pda()).await, v0 + ev);
    assert_eq!(env.lamports(&b).await, b0 + eb);
    assert_eq!(env.lamports(&o).await, o0 + eo);
    assert_eq!(env.lamports(&protocol_pda()).await, rent);

    let pp: ProtocolPool = env.anchor(&protocol_pda()).await;
    assert_eq!(pp.total_distributed, amount);
    let hv: HerdVault = env.anchor(&vault_pda()).await;
    assert_eq!(hv.total_in, ev);

    // nothing left above rent
    let e = env.send(&[ix_distribute(&admin.pubkey(), &b, &o, 1)], &[&admin]).await.unwrap_err();
    assert_eq!(custom_code(&e), Some(herr(HerdError::InsufficientProtocolPool)));
}

#[tokio::test]
async fn cannot_dip_into_rent() {
    let mut env = start().await;
    pool_with_fees(&mut env).await;
    let rent = pool_rent(&mut env).await;
    let amount = env.lamports(&protocol_pda()).await - rent + 1;
    let admin = env.admin.insecure_clone();
    let (b, o) = (env.builders, env.ops);
    let e = env.send(&[ix_distribute(&admin.pubkey(), &b, &o, amount)], &[&admin]).await.unwrap_err();
    assert_eq!(custom_code(&e), Some(herr(HerdError::InsufficientProtocolPool)));
}

#[tokio::test]
async fn only_authority_can_distribute() {
    let mut env = start().await;
    pool_with_fees(&mut env).await;
    let mallory = env.funded(SOL).await;
    let (b, o) = (env.builders, env.ops);
    let e = env.send(&[ix_distribute(&mallory.pubkey(), &b, &o, 1_000)], &[&mallory]).await.unwrap_err();
    assert_eq!(custom_code(&e), Some(u32::from(AnchorError::ConstraintHasOne)));
}

#[tokio::test]
async fn destinations_must_match_config() {
    let mut env = start().await;
    pool_with_fees(&mut env).await;
    let admin = env.admin.insecure_clone();
    let thief = env.funded(SOL).await.pubkey();
    let o = env.ops;
    let e = env.send(&[ix_distribute(&admin.pubkey(), &thief, &o, 1_000)], &[&admin]).await.unwrap_err();
    assert_eq!(custom_code(&e), Some(herr(HerdError::InvalidWallet)));
}

#[tokio::test]
async fn update_wallets_authority_only_and_takes_effect() {
    let mut env = start().await;
    pool_with_fees(&mut env).await;
    let admin = env.admin.insecure_clone();
    let (nt, nb, no) = (Pubkey::new_unique(), env.funded(SOL).await.pubkey(), env.funded(SOL).await.pubkey());

    let mallory = env.funded(SOL).await;
    let e = env.send(&[ix_update_wallets(&mallory.pubkey(), nt, nb, no)], &[&mallory]).await.unwrap_err();
    assert_eq!(custom_code(&e), Some(u32::from(AnchorError::ConstraintHasOne)));

    let e = env
        .send(&[ix_update_wallets(&admin.pubkey(), nt, Pubkey::default(), no)], &[&admin])
        .await
        .unwrap_err();
    assert_eq!(custom_code(&e), Some(herr(HerdError::InvalidWallet)));

    env.send(&[ix_update_wallets(&admin.pubkey(), nt, nb, no)], &[&admin]).await.unwrap();
    let c: Config = env.anchor(&config_pda()).await;
    assert_eq!((c.treasury, c.builders_fund, c.ops_wallet), (nt, nb, no));

    let (old_b, old_o) = (env.builders, env.ops);
    assert!(env.send(&[ix_distribute(&admin.pubkey(), &old_b, &old_o, 1_000)], &[&admin]).await.is_err());
    let b0 = env.lamports(&nb).await;
    env.send(&[ix_distribute(&admin.pubkey(), &nb, &no, 1_000)], &[&admin]).await.unwrap();
    assert_eq!(env.lamports(&nb).await, b0 + 300);
}

/// Documents the remaining gap: RektPool accrues SOL from every trade and no instruction
/// can move it out. This test only asserts the inflow; see PR description (H-RektPool).
#[tokio::test]
async fn rekt_pool_accrues_with_no_outflow_path() {
    let mut env = start().await;
    let before = env.lamports(&rekt_pda()).await;
    pool_with_fees(&mut env).await;
    let rp: RektPool = env.anchor(&rekt_pda()).await;
    assert!(rp.total_in > 0);
    assert_eq!(rp.total_paid, 0);
    assert!(env.lamports(&rekt_pda()).await > before);
}
