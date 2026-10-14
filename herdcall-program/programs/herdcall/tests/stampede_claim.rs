//! Stampede claim lifecycle, including claims made AFTER breakout.
//! Breakout itself is simulated (see `common::simulate_breakout`): DAMM v2 is not loaded here.
mod common;

use common::*;
use herdcall::errors::HerdError;
use herdcall::state::*;
use solana_sdk::program_pack::Pack;
use solana_sdk::signature::{Keypair, Signer};

struct Launched {
    mint: Keypair,
    creator: Keypair,
    users: Vec<(Keypair, u64)>,
}

/// Config + launch + oversubscribed lobby (cap 2 SOL) with deposits chosen so the
/// pro-rata `accepted` amounts are NOT integers (exercises rounding dust).
async fn oversubscribed_lobby(env: &mut Env) -> Launched {
    init_config(env).await.unwrap();
    let creator = env.funded(10 * SOL).await;
    let mint = Keypair::new();
    env.send(&[ix_create_launch(&creator.pubkey(), &mint.pubkey())], &[&creator, &mint]).await.unwrap();

    let mut users = vec![];
    for amt in [1_000_000_001u64, 1_333_333_333, 700_000_007, 10_000_019] {
        let u = env.funded(5 * SOL).await;
        env.send(&[ix_deposit(&u.pubkey(), &mint.pubkey(), amt)], &[&u]).await.unwrap();
        users.push((u, amt));
    }
    env.warp_secs(20).await;
    let payer = env.payer();
    env.send(&[ix_settle(&payer.pubkey(), &mint.pubkey())], &[]).await.unwrap();
    Launched { mint, creator, users }
}

async fn fill_curve(env: &mut Env, l: &Launched) {
    let buyer = env.funded(100 * SOL).await;
    let ui = env.funded(SOL).await;
    env.send(
        &[ix_buy(&buyer.pubkey(), &l.mint.pubkey(), &l.creator.pubkey(), &ui.pubkey(), 60 * SOL)],
        &[&buyer],
    )
    .await
    .unwrap();
    let launch: Launch = env.anchor(&launch_pda(&l.mint.pubkey())).await;
    assert_eq!(launch.state, LaunchState::CurveFull);
}

async fn assert_claim_ok(env: &mut Env, mint: &solana_sdk::pubkey::Pubkey, owner: &Keypair, deposit: u64) {
    let launch: Launch = env.anchor(&launch_pda(mint)).await;
    let (_, tokens) = herdcall::stampede::claim_amounts(
        deposit,
        launch.total_deposited,
        launch.cap_lamports,
        launch.accepted_total,
        launch.stampede_tokens,
    )
    .unwrap();
    env.send(&[ix_claim(&owner.pubkey(), mint)], &[owner]).await.unwrap_or_else(|e| {
        panic!("claim failed in state {:?}: {e:?}", launch.state)
    });
    assert_eq!(env.token_balance(&ata(&owner.pubkey(), mint)).await, tokens);
    assert!(env.account(&order_pda(&launch_pda(mint), &owner.pubkey())).await.is_none(), "order not closed");
}

/// Regression for the main bug: claims after breakout must work (tokens + cap refund),
/// with the mint authority revoked and the curve SOL swept out of the launch PDA.
#[tokio::test]
async fn claim_works_after_breakout_for_every_depositor() {
    let mut env = start().await;
    let l = oversubscribed_lobby(&mut env).await;

    // one user claims during Curve (normal path), the rest never claim before breakout
    let (u0, d0) = (&l.users[0].0, l.users[0].1);
    assert_claim_ok(&mut env, &l.mint.pubkey(), &u0.insecure_clone(), d0).await;

    fill_curve(&mut env, &l).await;
    simulate_breakout(&mut env, &l.mint.pubkey()).await;

    for (u, d) in l.users.iter().skip(1) {
        assert_claim_ok(&mut env, &l.mint.pubkey(), &u.insecure_clone(), *d).await;
    }

    // launch PDA still rent exempt, escrow holds at most rounding dust
    let launch_key = launch_pda(&l.mint.pubkey());
    let rent = env.rent_min(8 + <Launch as anchor_lang::Space>::INIT_SPACE).await;
    assert!(env.lamports(&launch_key).await >= rent);
    // escrow holds exactly the pro-rata rounding dust (stampede_tokens - sum of claimed tokens)
    let launch: Launch = env.anchor(&launch_key).await;
    let claimed: u64 = l.users.iter().map(|(_, d)| {
        herdcall::stampede::claim_amounts(*d, launch.total_deposited, launch.cap_lamports,
            launch.accepted_total, launch.stampede_tokens).unwrap().1
    }).sum();
    let dust = env.token_balance(&ata(&launch_key, &l.mint.pubkey())).await;
    assert_eq!(dust, launch.stampede_tokens - claimed);
    // and the rounding dust is tiny: < 1 token per depositor (6 decimals)
    assert!(dust < l.users.len() as u64 * 1_000_000, "escrow dust {dust}");
}

/// Claiming twice is impossible (order closed), also after breakout.
#[tokio::test]
async fn double_claim_after_breakout_rejected() {
    let mut env = start().await;
    let l = oversubscribed_lobby(&mut env).await;
    fill_curve(&mut env, &l).await;
    simulate_breakout(&mut env, &l.mint.pubkey()).await;
    let (u, d) = (&l.users[1].0.insecure_clone(), l.users[1].1);
    assert_claim_ok(&mut env, &l.mint.pubkey(), u, d).await;
    assert!(env.send(&[ix_claim(&u.pubkey(), &l.mint.pubkey())], &[u]).await.is_err());
}

/// Claims stay closed while the lobby is still open.
#[tokio::test]
async fn claim_rejected_during_lobby() {
    let mut env = start().await;
    init_config(&mut env).await.unwrap();
    let creator = env.funded(10 * SOL).await;
    let mint = Keypair::new();
    env.send(&[ix_create_launch(&creator.pubkey(), &mint.pubkey())], &[&creator, &mint]).await.unwrap();
    let u = env.funded(5 * SOL).await;
    env.send(&[ix_deposit(&u.pubkey(), &mint.pubkey(), SOL)], &[&u]).await.unwrap();
    let e = env.send(&[ix_claim(&u.pubkey(), &mint.pubkey())], &[&u]).await.unwrap_err();
    assert_eq!(custom_code(&e), Some(herr(HerdError::InvalidState)));
}

/// Exact SOL accounting of an oversubscribed claim after breakout: the owner receives
/// refund - fee (+ order rent back, - token ATA rent) and the protocol pool receives the fee,
/// even though the fee is taken out of the refund (no extra wallet SOL needed).
#[tokio::test]
async fn claim_after_breakout_pays_exact_refund_and_fee() {
    let mut env = start().await;
    let l = oversubscribed_lobby(&mut env).await;
    fill_curve(&mut env, &l).await;
    simulate_breakout(&mut env, &l.mint.pubkey()).await;

    let (u, d) = (l.users[1].0.insecure_clone(), l.users[1].1);
    let launch_key = launch_pda(&l.mint.pubkey());
    let launch: Launch = env.anchor(&launch_key).await;
    let (accepted, _) = herdcall::stampede::claim_amounts(
        d, launch.total_deposited, launch.cap_lamports, launch.accepted_total, launch.stampede_tokens,
    ).unwrap();
    let refund = d - accepted;
    let fee = herdcall::fees::claim_fee(accepted, launch.claim_fee_bps).unwrap();
    assert!(refund > fee && fee > 0, "scenario must have fee covered by refund");

    let order_rent = env.lamports(&order_pda(&launch_key, &u.pubkey())).await;
    let ata_rent = env.rent_min(spl_token::state::Account::LEN).await;
    let (u0, pp0, l0) = (
        env.lamports(&u.pubkey()).await,
        env.lamports(&protocol_pda()).await,
        env.lamports(&launch_key).await,
    );
    env.send(&[ix_claim(&u.pubkey(), &l.mint.pubkey())], &[&u]).await.unwrap();
    assert_eq!(env.lamports(&u.pubkey()).await, u0 + refund - fee + order_rent - ata_rent);
    assert_eq!(env.lamports(&protocol_pda()).await, pp0 + fee);
    assert_eq!(env.lamports(&launch_key).await, l0 - refund);
    let pp: herdcall::state::ProtocolPool = env.anchor(&protocol_pda()).await;
    assert!(pp.total_in >= fee);
}

/// Failed launch (min not reached): full refund, no tokens, no fee.
#[tokio::test]
async fn failed_launch_full_refund() {
    let mut env = start().await;
    init_config(&mut env).await.unwrap();
    let creator = env.funded(10 * SOL).await;
    let mint = Keypair::new();
    env.send(&[ix_create_launch(&creator.pubkey(), &mint.pubkey())], &[&creator, &mint]).await.unwrap();
    let u = env.funded(5 * SOL).await;
    let dep = SOL / 2; // below min_stampede_lamports (1 SOL)
    env.send(&[ix_deposit(&u.pubkey(), &mint.pubkey(), dep)], &[&u]).await.unwrap();
    env.warp_secs(20).await;
    let payer = env.payer();
    env.send(&[ix_settle(&payer.pubkey(), &mint.pubkey())], &[]).await.unwrap();
    let launch: Launch = env.anchor(&launch_pda(&mint.pubkey())).await;
    assert_eq!(launch.state, LaunchState::Failed);

    let order_rent = env.lamports(&order_pda(&launch_pda(&mint.pubkey()), &u.pubkey())).await;
    let ata_rent = env.rent_min(spl_token::state::Account::LEN).await;
    let before = env.lamports(&u.pubkey()).await;
    env.send(&[ix_claim(&u.pubkey(), &mint.pubkey())], &[&u]).await.unwrap();
    assert_eq!(env.lamports(&u.pubkey()).await, before + dep + order_rent - ata_rent);
    assert_eq!(env.token_balance(&ata(&u.pubkey(), &mint.pubkey())).await, 0);
}
