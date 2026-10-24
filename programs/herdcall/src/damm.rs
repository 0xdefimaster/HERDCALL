//! Meteora DAMM v2 (program `cpamdp…`): the pool a token migrates to at breakout (document/CURVE_SPEC.md §5.4).
//!
//! The instructions are built by hand from the DAMM v2 source (MeteoraAg/damm-v2, cp_amm 0.2.5) because its crate
//! targets another Anchor version. Discriminators, account order and the config layout are checked by the LiteSVM
//! tests against the real program binary dumped from mainnet.
use anchor_lang::prelude::*;
use anchor_lang::solana_program::instruction::{AccountMeta, Instruction};
use anchor_lang::solana_program::program::invoke_signed;
use herdcall_core::dex::{MAX_SQRT_PRICE, MIN_SQRT_PRICE};

use crate::errors::HerdError;

pub const PROGRAM_ID: Pubkey = Pubkey::from_str_const("cpamdpZCGKUy5JxQXB4dcpGPiikHawvSWAd6mEn1sGG");
pub const POOL_AUTHORITY: Pubkey = Pubkey::from_str_const("HLnpSz9h2S4hiLQ43rnSD9XkcUThA7B8hQMKmDaiTLcC");

const IX_INITIALIZE_POOL: [u8; 8] = [95, 180, 10, 172, 84, 174, 232, 40];
const IX_PERMANENT_LOCK_POSITION: [u8; 8] = [165, 176, 125, 6, 231, 171, 186, 213];
const CONFIG_DISCRIMINATOR: [u8; 8] = [155, 12, 170, 224, 30, 250, 204, 130];
const CONFIG_LEN: usize = 8 + 320;

/// Fees are collected in token B (SOL) only.
const COLLECT_FEE_ONLY_B: u8 = 1;
const CONFIG_STATIC: u8 = 0;
/// DAMM fee numerators are over 1e9; HerdCall accepts at most a 2% pool fee.
const MAX_FEE_NUMERATOR: u64 = 20_000_000;

/// What the breakout needs from a DAMM v2 static config.
pub struct PoolConfig {
    pub sqrt_min_price: u128,
    pub sqrt_max_price: u128,
}

fn u128_at(d: &[u8], at: usize) -> u128 {
    u128::from_le_bytes(d[at..at + 16].try_into().unwrap())
}

/// Reads and checks the config the pool is created with. Only a config whose pool creator is the HerdCall migrator can
/// be used, so nobody else can create the canonical pool first and block the breakout; the `devnet` build also takes
/// public configs, for testing only. The config must be static, collect fees in SOL, span the full price range and
/// charge a constant fee of at most 2%.
pub fn read_config(acc: &AccountInfo, migrator: &Pubkey) -> Result<PoolConfig> {
    require_keys_eq!(*acc.owner, PROGRAM_ID, HerdError::InvalidDexConfig);
    let d = acc.try_borrow_data()?;
    require!(d.len() >= CONFIG_LEN && d[..8] == CONFIG_DISCRIMINATOR, HerdError::InvalidDexConfig);
    let d = &d[8..];
    let creator = Pubkey::new_from_array(d[32..64].try_into().unwrap());
    let creator_ok = creator == *migrator || (cfg!(feature = "devnet") && creator == Pubkey::default());
    require!(creator_ok, HerdError::InvalidDexConfig);
    // pool_fees.base_fee (32 bytes at 64): cliff_fee_numerator u64, base_fee_mode u8, padding, number_of_period u16,
    // period_frequency u64, reduction_factor u64. A constant fee has no periods.
    let base = &d[64..96];
    let cliff = u64::from_le_bytes(base[0..8].try_into().unwrap());
    let periods = u16::from_le_bytes(base[14..16].try_into().unwrap());
    require!(cliff <= MAX_FEE_NUMERATOR && periods == 0 && base[8] == 0, HerdError::InvalidDexConfig);
    let (collect_fee_mode, config_type) = (d[193], d[194]);
    require!(collect_fee_mode == COLLECT_FEE_ONLY_B && config_type == CONFIG_STATIC, HerdError::InvalidDexConfig);
    let cfg = PoolConfig { sqrt_min_price: u128_at(d, 208), sqrt_max_price: u128_at(d, 224) };
    require!(cfg.sqrt_min_price == MIN_SQRT_PRICE && cfg.sqrt_max_price == MAX_SQRT_PRICE, HerdError::InvalidDexConfig);
    Ok(cfg)
}

/// Accounts of `initialize_pool`, in DAMM's order.
pub struct InitializePool<'a, 'info> {
    pub creator: &'a AccountInfo<'info>,
    pub position_nft_mint: &'a AccountInfo<'info>,
    pub position_nft_account: &'a AccountInfo<'info>,
    pub payer: &'a AccountInfo<'info>,
    pub config: &'a AccountInfo<'info>,
    pub pool_authority: &'a AccountInfo<'info>,
    pub pool: &'a AccountInfo<'info>,
    pub position: &'a AccountInfo<'info>,
    pub token_a_mint: &'a AccountInfo<'info>,
    pub token_b_mint: &'a AccountInfo<'info>,
    pub token_a_vault: &'a AccountInfo<'info>,
    pub token_b_vault: &'a AccountInfo<'info>,
    pub payer_token_a: &'a AccountInfo<'info>,
    pub payer_token_b: &'a AccountInfo<'info>,
    pub token_program: &'a AccountInfo<'info>,
    pub token_2022_program: &'a AccountInfo<'info>,
    pub system_program: &'a AccountInfo<'info>,
    pub event_authority: &'a AccountInfo<'info>,
    pub program: &'a AccountInfo<'info>,
}

pub fn initialize_pool(a: &InitializePool, liquidity: u128, sqrt_price: u128, signers: &[&[&[u8]]]) -> Result<()> {
    let w = |k: &AccountInfo| AccountMeta::new(k.key(), false);
    let r = |k: &AccountInfo| AccountMeta::new_readonly(k.key(), false);
    let accounts = vec![
        r(a.creator),
        AccountMeta::new(a.position_nft_mint.key(), true),
        w(a.position_nft_account),
        AccountMeta::new(a.payer.key(), true),
        r(a.config),
        r(a.pool_authority),
        w(a.pool),
        w(a.position),
        r(a.token_a_mint),
        r(a.token_b_mint),
        w(a.token_a_vault),
        w(a.token_b_vault),
        w(a.payer_token_a),
        w(a.payer_token_b),
        r(a.token_program),
        r(a.token_program),
        r(a.token_2022_program),
        r(a.system_program),
        r(a.event_authority),
        r(a.program),
    ];
    let mut data = IX_INITIALIZE_POOL.to_vec();
    data.extend_from_slice(&liquidity.to_le_bytes());
    data.extend_from_slice(&sqrt_price.to_le_bytes());
    data.push(0); // activation_point: None, trading starts now
    let ix = Instruction { program_id: PROGRAM_ID, accounts, data };
    let infos = [
        a.creator, a.position_nft_mint, a.position_nft_account, a.payer, a.config, a.pool_authority, a.pool, a.position,
        a.token_a_mint, a.token_b_mint, a.token_a_vault, a.token_b_vault, a.payer_token_a, a.payer_token_b,
        a.token_program, a.token_2022_program, a.system_program, a.event_authority, a.program,
    ]
    .map(|i| i.clone());
    invoke_signed(&ix, &infos, signers).map_err(Into::into)
}

/// Locks `liquidity` of the position for good: it can never be withdrawn, but keeps earning fees.
#[allow(clippy::too_many_arguments)]
pub fn permanent_lock_position<'info>(
    pool: &AccountInfo<'info>,
    position: &AccountInfo<'info>,
    position_nft_account: &AccountInfo<'info>,
    owner: &AccountInfo<'info>,
    event_authority: &AccountInfo<'info>,
    program: &AccountInfo<'info>,
    liquidity: u128,
    signers: &[&[&[u8]]],
) -> Result<()> {
    let accounts = vec![
        AccountMeta::new(pool.key(), false),
        AccountMeta::new(position.key(), false),
        AccountMeta::new_readonly(position_nft_account.key(), false),
        AccountMeta::new_readonly(owner.key(), true),
        AccountMeta::new_readonly(event_authority.key(), false),
        AccountMeta::new_readonly(program.key(), false),
    ];
    let mut data = IX_PERMANENT_LOCK_POSITION.to_vec();
    data.extend_from_slice(&liquidity.to_le_bytes());
    let ix = Instruction { program_id: PROGRAM_ID, accounts, data };
    let infos = [pool, position, position_nft_account, owner, event_authority, program].map(|i| i.clone());
    invoke_signed(&ix, &infos, signers).map_err(Into::into)
}
