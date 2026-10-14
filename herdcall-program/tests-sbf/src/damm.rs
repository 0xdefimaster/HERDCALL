//! Meteora DAMM v2 helpers. Layouts, PDA seeds and liquidity math come from the cp-amm SOURCE
//! (pinned commit); the program that EXECUTES is the binary dumped from mainnet in CI.

use anchor_lang::{InstructionData, ToAccountMetas};
use cp_amm::state::{CollectFeeMode, Config, Pool, Position};
use litesvm::LiteSVM;
use solana_account::Account;
use solana_address::Address;
use solana_instruction::Instruction;

use crate::hc::{self, DammBreakoutAccounts};

pub const DAMM_ID: Address = Address::from_str_const("cpamdpZCGKUy5JxQXB4dcpGPiikHawvSWAd6mEn1sGG");
pub const POOL_AUTHORITY: Address = Address::from_str_const("HLnpSz9h2S4hiLQ43rnSD9XkcUThA7B8hQMKmDaiTLcC");

pub fn event_authority() -> Address {
    Address::find_program_address(&[b"__event_authority"], &DAMM_ID).0
}
fn sorted(a: &Address, b: &Address) -> (Address, Address) {
    if a > b { (*a, *b) } else { (*b, *a) }
}
pub fn pool_pda(config: &Address, a: &Address, b: &Address) -> Address {
    let (hi, lo) = sorted(a, b);
    Address::find_program_address(&[b"pool", config.as_ref(), hi.as_ref(), lo.as_ref()], &DAMM_ID).0
}
pub fn customizable_pool_pda(a: &Address, b: &Address) -> Address {
    let (hi, lo) = sorted(a, b);
    Address::find_program_address(&[b"cpool", hi.as_ref(), lo.as_ref()], &DAMM_ID).0
}
pub fn vault_pda(mint: &Address, pool: &Address) -> Address {
    Address::find_program_address(&[b"token_vault", mint.as_ref(), pool.as_ref()], &DAMM_ID).0
}
pub fn position_pda(nft_mint: &Address) -> Address {
    Address::find_program_address(&[b"position", nft_mint.as_ref()], &DAMM_ID).0
}
pub fn position_nft_account_pda(nft_mint: &Address) -> Address {
    Address::find_program_address(&[b"position_nft_account", nft_mint.as_ref()], &DAMM_ID).0
}

pub fn accounts_for(config: &Address, mint: &Address, nft_mint: &Address) -> DammBreakoutAccounts {
    let pool = pool_pda(config, mint, &hc::WSOL);
    DammBreakoutAccounts {
        config: *config,
        position_nft_mint: *nft_mint,
        position_nft_account: position_nft_account_pda(nft_mint),
        pool,
        position: position_pda(nft_mint),
        token_a_vault: vault_pda(mint, &pool),
        token_b_vault: vault_pda(&hc::WSOL, &pool),
    }
}

/// Real PUBLIC static config account data fetched from mainnet by CI (read-only RPC).
pub fn public_config_bytes() -> Vec<u8> {
    let path = crate::env::so_path("DAMM_PUBLIC_CONFIG_BIN");
    let d = std::fs::read(&path).unwrap_or_else(|e| panic!("read {path}: {e}"));
    assert_eq!(&d[..8], &<Config as anchor_lang::Discriminator>::DISCRIMINATOR[..], "not a DAMM Config account");
    d
}

pub fn decode_config(d: &[u8]) -> Config {
    bytemuck::pod_read_unaligned(&d[8..8 + std::mem::size_of::<Config>()])
}

/// Installs a DAMM config account. `creator_authority = None` keeps the real public config as is
/// (`pool_creator_authority == default`); `Some(k)` turns the same bytes into a PRIVATE config.
pub fn install_config(svm: &mut LiteSVM, at: Address, creator_authority: Option<Address>) -> Config {
    let mut d = public_config_bytes();
    let off = 8 + std::mem::offset_of!(Config, pool_creator_authority);
    if let Some(k) = creator_authority {
        d[off..off + 32].copy_from_slice(k.as_ref());
    }
    let lamports = svm.minimum_balance_for_rent_exemption(d.len());
    svm.set_account(at, Account { lamports, data: d.clone(), owner: DAMM_ID, executable: false, rent_epoch: 0 })
        .unwrap();
    decode_config(&d)
}

pub fn read_pool(svm: &LiteSVM, pool: &Address) -> Pool {
    let a = svm.get_account(pool).expect("pool account");
    bytemuck::pod_read_unaligned(&a.data[8..8 + std::mem::size_of::<Pool>()])
}
pub fn read_position(svm: &LiteSVM, position: &Address) -> Position {
    let a = svm.get_account(position).expect("position account");
    bytemuck::pod_read_unaligned(&a.data[8..8 + std::mem::size_of::<Position>()])
}

/// sqrt(b / a) in Q64.64.
pub fn sqrt_price_from_amounts(a: u64, b: u64) -> u128 {
    use ruint::aliases::U256;
    let v: U256 = (U256::from(b) << 128usize) / U256::from(a);
    let r: U256 = v.root(2);
    r.to::<u128>()
}

/// Token amounts DAMM would pull for `liquidity` at `sqrt_price` (exact program math).
pub fn initial_amounts(cfg: &Config, sqrt_price: u128, liquidity: u128) -> Option<(u64, u64)> {
    let mode = CollectFeeMode::try_from(cfg.collect_fee_mode).ok()?;
    cp_amm::get_initial_pool_information(mode, cfg.sqrt_min_price, cfg.sqrt_max_price, sqrt_price, liquidity)
        .ok()
        .map(|i| (i.token_a_amount, i.token_b_amount))
}

/// Largest liquidity whose required amounts fit in (max_a, max_b): what an honest breakout caller
/// computes off-chain.
pub fn max_liquidity(cfg: &Config, sqrt_price: u128, max_a: u64, max_b: u64) -> u128 {
    let (mut lo, mut hi) = (0u128, u128::MAX >> 1);
    while lo < hi {
        let mid = lo + (hi - lo + 1) / 2;
        match initial_amounts(cfg, sqrt_price, mid) {
            Some((a, b)) if a <= max_a && b <= max_b => lo = mid,
            _ => hi = mid - 1,
        }
    }
    lo
}

#[allow(clippy::too_many_arguments)]
pub fn ix_initialize_pool(
    payer: &Address,
    creator: &Address,
    nft_mint: &Address,
    config: &Address,
    a_mint: &Address,
    b_mint: &Address,
    payer_token_a: &Address,
    payer_token_b: &Address,
    liquidity: u128,
    sqrt_price: u128,
) -> Instruction {
    let pool = pool_pda(config, a_mint, b_mint);
    let accounts = cp_amm::accounts::InitializePoolCtx {
        creator: *creator,
        position_nft_mint: *nft_mint,
        position_nft_account: position_nft_account_pda(nft_mint),
        payer: *payer,
        config: *config,
        pool_authority: POOL_AUTHORITY,
        pool,
        position: position_pda(nft_mint),
        token_a_mint: *a_mint,
        token_b_mint: *b_mint,
        token_a_vault: vault_pda(a_mint, &pool),
        token_b_vault: vault_pda(b_mint, &pool),
        payer_token_a: *payer_token_a,
        payer_token_b: *payer_token_b,
        token_a_program: hc::TOKEN,
        token_b_program: hc::TOKEN,
        token_2022_program: hc::TOKEN_2022,
        system_program: hc::SYSTEM,
        event_authority: event_authority(),
        program: DAMM_ID,
    };
    Instruction {
        program_id: DAMM_ID,
        accounts: accounts.to_account_metas(None),
        data: cp_amm::instruction::InitializePool {
            params: cp_amm::InitializePoolParameters { liquidity, sqrt_price, activation_point: None },
        }
        .data(),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn ix_swap(
    payer: &Address,
    pool: &Address,
    input: &Address,
    output: &Address,
    a_vault: &Address,
    b_vault: &Address,
    a_mint: &Address,
    b_mint: &Address,
    amount_in: u64,
) -> Instruction {
    let accounts = cp_amm::accounts::SwapCtx {
        pool_authority: POOL_AUTHORITY,
        pool: *pool,
        input_token_account: *input,
        output_token_account: *output,
        token_a_vault: *a_vault,
        token_b_vault: *b_vault,
        token_a_mint: *a_mint,
        token_b_mint: *b_mint,
        payer: *payer,
        token_a_program: hc::TOKEN,
        token_b_program: hc::TOKEN,
        referral_token_account: None,
        event_authority: event_authority(),
        program: DAMM_ID,
    };
    Instruction {
        program_id: DAMM_ID,
        accounts: accounts.to_account_metas(None),
        data: cp_amm::instruction::Swap {
            _params: cp_amm::SwapParameters { amount_in, minimum_amount_out: 0 },
        }
        .data(),
    }
}
