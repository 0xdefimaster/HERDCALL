//! Only the program's upgrade authority can initialize, so nobody can take the admin role between deploy and setup.
mod common;

use common::*;
use anchor_lang::prelude::Pubkey;
use herdcall::state::*;
use solana_signer::Signer;

#[test]
fn the_upgrade_authority_initializes() {
    let mut env = Env::new();
    env.initialize(example_params()).unwrap();
    let c: Config = env.account(&pda(&[CONFIG_SEED]));
    assert_eq!(c.admin, pk(&env.admin.pubkey()));
    assert_eq!(c.reinvest_keeper, c.admin);
    // Once only.
    assert!(env.initialize(example_params()).is_err());
}

#[test]
fn anyone_else_is_refused() {
    let mut env = Env::new();
    let attacker = env.user(10);
    let r = env.initialize_as(&attacker, program_data(), example_params());
    assert!(r.unwrap_err().contains("NotUpgradeAuthority"));
    // The real authority can still initialize afterwards.
    env.initialize(example_params()).unwrap();
}

#[test]
fn another_programs_program_data_is_refused() {
    let mut env = Env::new();
    let attacker = env.user(10);
    // A program the attacker deployed, with the attacker as its upgrade authority (here: the DAMM program's
    // ProgramData account, rewritten).
    let other = Pubkey::find_program_address(&[DAMM.as_ref()], &UPGRADEABLE_LOADER).0;
    let mut acc = env.svm.get_account(&addr(&other)).unwrap();
    acc.data[12] = 1;
    acc.data[13..45].copy_from_slice(attacker.pubkey().as_ref());
    env.svm.set_account(addr(&other), acc).unwrap();
    let r = env.initialize_as(&attacker, other, example_params());
    assert!(r.unwrap_err().contains("NotUpgradeAuthority"));
}

#[test]
fn an_immutable_program_cannot_be_initialized() {
    let mut env = Env::new();
    env.set_upgrade_authority(None);
    let admin = env.admin.insecure_clone();
    let r = env.initialize_as(&admin, program_data(), example_params());
    assert!(r.unwrap_err().contains("NotUpgradeAuthority"));
}
