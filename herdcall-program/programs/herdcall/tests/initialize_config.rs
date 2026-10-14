//! initialize_config must be callable only by the program's upgrade authority.
mod common;

use anchor_lang::error::ErrorCode as AnchorError;
use common::*;
use herdcall::errors::HerdError;
use herdcall::state::Config;
use solana_sdk::{
    account::Account,
    bpf_loader_upgradeable::{self, UpgradeableLoaderState},
    pubkey::Pubkey,
    signature::Signer,
};

#[tokio::test]
async fn front_runner_cannot_take_over_config() {
    let mut env = start().await;
    let attacker = env.funded(10 * SOL).await;
    let args = init_args(&env);
    let e = env
        .send(&[ix_init_config(&attacker.pubkey(), &program_data_pda(), args)], &[&attacker])
        .await
        .unwrap_err();
    assert_eq!(custom_code(&e), Some(herr(HerdError::NotUpgradeAuthority)));
    assert!(env.account(&config_pda()).await.is_none(), "config must not exist");

    // the legitimate upgrade authority can still initialize afterwards
    init_config(&mut env).await.unwrap();
    let c: Config = env.anchor(&config_pda()).await;
    assert_eq!(c.authority, env.admin.pubkey());
    assert_eq!(c.builders_fund, env.builders);
}

#[tokio::test]
async fn second_initialize_rejected() {
    let mut env = start().await;
    init_config(&mut env).await.unwrap();
    // different args (new blockhash) so it is not a duplicate tx
    let mut args = init_args(&env);
    args.params.claim_fee_bps = 0;
    let admin = env.admin.insecure_clone();
    assert!(env.send(&[ix_init_config(&admin.pubkey(), &program_data_pda(), args)], &[&admin]).await.is_err());
}

#[tokio::test]
async fn immutable_program_cannot_initialize() {
    let mut env = start_with(false).await; // ProgramData.upgrade_authority = None
    let e = init_config(&mut env).await.unwrap_err();
    assert_eq!(custom_code(&e), Some(herr(HerdError::NotUpgradeAuthority)));
}

/// An attacker cannot substitute the ProgramData of a program they control.
#[tokio::test]
async fn foreign_program_data_rejected() {
    let mut env = start().await;
    let attacker = env.funded(10 * SOL).await;
    let fake = Pubkey::new_unique();
    let data = bincode::serialize(&UpgradeableLoaderState::ProgramData {
        slot: 0,
        upgrade_authority_address: Some(attacker.pubkey()),
    })
    .unwrap();
    env.ctx.set_account(
        &fake,
        &Account { lamports: SOL, data, owner: bpf_loader_upgradeable::id(), executable: false, rent_epoch: 0 }
            .into(),
    );
    let args = init_args(&env);
    let e = env.send(&[ix_init_config(&attacker.pubkey(), &fake, args)], &[&attacker]).await.unwrap_err();
    assert_eq!(custom_code(&e), Some(u32::from(AnchorError::ConstraintSeeds)));
}

#[tokio::test]
async fn default_wallet_rejected() {
    let mut env = start().await;
    let mut args = init_args(&env);
    args.ops_wallet = Pubkey::default();
    let admin = env.admin.insecure_clone();
    let e = env.send(&[ix_init_config(&admin.pubkey(), &program_data_pda(), args)], &[&admin]).await.unwrap_err();
    assert_eq!(custom_code(&e), Some(herr(HerdError::InvalidWallet)));
}
