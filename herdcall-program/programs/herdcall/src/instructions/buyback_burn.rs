//! buyback_burn: `Launch.burn_reserve`'deki SOL ile DAMM havuzundan launch token'i alir ve yakar.
//! Rezerv; creator geliri hak edilmediginde (taahhut yok / hak kaybi) ya da creator'in gonullu
//! yakim payindan (`burn_bps`) dolar. Permissionless.
//!
//! On-run (sandwich) korumasi tam degil, SINIRLIDIR:
//!  * tek islem <= havuz SOL rezervinin %1'i (fiyat etkisi kucuk)
//!  * min_out, havuzun zincir uzerindeki spot fiyatindan hesaplanir (cagiranin verdigi deger degil)
//!    ve spotun %10 altina inemez. Saldirganin kazanci bu toleransla sinirlidir; asiri dusuk
//!    cikis olursa swap geri alinir (fon kaybi olmaz).
//! Akis: rezervden wSOL'e sar -> DAMM `swap` -> gelen token'i yak -> gecici ATA'lari kapat
//! (rent caller'a doner).
//!
//! UYARI: SBF derlemesi ve devnet testi YAPILMADI. `swap` hesap sirasi/diskriminator IDL'den
//! (cp_amm v0.2.5), Pool ofsetleri IDL tip tanimindan hesaplandi; entegrasyon testi zorunlu.

use anchor_lang::prelude::*;
use anchor_lang::solana_program::instruction::{AccountMeta, Instruction};
use anchor_lang::solana_program::program::invoke_signed;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{
    burn, close_account, sync_native, Burn, CloseAccount, Mint, SyncNative, Token, TokenAccount,
};

use crate::buyback::{buyback_min_out, max_buyback};
use crate::constants::*;
use crate::errors::HerdError;
use crate::events::*;
use crate::instructions::breakout_launch::token_amount;
use crate::state::*;
use crate::trade::assert_launch_rent;

const DISC_SWAP: [u8; 8] = [248, 198, 158, 145, 225, 117, 135, 200];

#[derive(Accounts)]
pub struct BuybackBurn<'info> {
    #[account(mut)]
    pub caller: Signer<'info>,

    #[account(
        mut,
        seeds = [LAUNCH_SEED, launch.mint.as_ref()], bump = launch.bump,
        constraint = launch.state == LaunchState::Breakout @ HerdError::InvalidState
    )]
    pub launch: Box<Account<'info, Launch>>,

    #[account(mut, address = launch.mint)]
    pub mint: Box<Account<'info, Mint>>,

    /// CHECK: system-owned PDA; DAMM swap'ta payer/signer (gecici ATA'larin sahibi).
    #[account(seeds = [BREAKOUT_AUTH_SEED, launch.key().as_ref()], bump)]
    pub breakout_authority: UncheckedAccount<'info>,

    #[account(address = anchor_spl::token::spl_token::native_mint::ID)]
    pub wsol_mint: Box<Account<'info, Mint>>,

    #[account(
        init_if_needed, payer = caller,
        associated_token::mint = mint, associated_token::authority = breakout_authority
    )]
    pub breakout_token: Box<Account<'info, TokenAccount>>,

    #[account(
        init_if_needed, payer = caller,
        associated_token::mint = wsol_mint, associated_token::authority = breakout_authority
    )]
    pub breakout_wsol: Box<Account<'info, TokenAccount>>,

    /// CHECK: breakout'ta kaydedilen havuz; icerigi asagida ofsetlerle dogrulanir.
    #[account(mut, address = launch.dex_pool)]
    pub pool: UncheckedAccount<'info>,
    /// CHECK: havuzdaki token_a_vault ile eslesmesi handler'da dogrulanir.
    #[account(mut)]
    pub token_a_vault: UncheckedAccount<'info>,
    /// CHECK: havuzdaki token_b_vault ile eslesmesi handler'da dogrulanir.
    #[account(mut)]
    pub token_b_vault: UncheckedAccount<'info>,
    /// CHECK: cp-amm program
    #[account(address = DAMM_PROGRAM_ID)]
    pub damm_program: UncheckedAccount<'info>,
    /// CHECK: cp-amm global pool authority
    #[account(address = DAMM_POOL_AUTHORITY)]
    pub damm_pool_authority: UncheckedAccount<'info>,
    /// CHECK: ["__event_authority"] @ cp-amm
    #[account(seeds = [b"__event_authority"], bump, seeds::program = DAMM_PROGRAM_ID)]
    pub damm_event_authority: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

fn read_key(d: &[u8], off: usize) -> Result<Pubkey> {
    require!(d.len() >= off + 32, HerdError::DammPoolMismatch);
    Ok(Pubkey::new_from_array(d[off..off + 32].try_into().unwrap()))
}

pub fn buyback_burn(ctx: Context<BuybackBurn>, amount: u64) -> Result<()> {
    require!(amount >= BUYBACK_MIN_LAMPORTS, HerdError::BuybackSizeOutOfBounds);
    require!(amount <= ctx.accounts.launch.burn_reserve, HerdError::InsufficientBurnReserve);

    // Havuzu dogrula ve spot fiyati oku
    let sqrt_price = {
        let pool = ctx.accounts.pool.to_account_info();
        require_keys_eq!(*pool.owner, DAMM_PROGRAM_ID, HerdError::DammPoolMismatch);
        let d = pool.try_borrow_data()?;
        require!(d.len() >= POOL_SQRT_PRICE_OFFSET + 16, HerdError::DammPoolMismatch);
        require_keys_eq!(read_key(&d, POOL_TOKEN_A_MINT_OFFSET)?, ctx.accounts.mint.key(), HerdError::DammPoolMismatch);
        require_keys_eq!(read_key(&d, POOL_TOKEN_B_MINT_OFFSET)?, ctx.accounts.wsol_mint.key(), HerdError::DammPoolMismatch);
        require_keys_eq!(read_key(&d, POOL_TOKEN_A_VAULT_OFFSET)?, ctx.accounts.token_a_vault.key(), HerdError::DammPoolMismatch);
        require_keys_eq!(read_key(&d, POOL_TOKEN_B_VAULT_OFFSET)?, ctx.accounts.token_b_vault.key(), HerdError::DammPoolMismatch);
        let mut b = [0u8; 16];
        b.copy_from_slice(&d[POOL_SQRT_PRICE_OFFSET..POOL_SQRT_PRICE_OFFSET + 16]);
        u128::from_le_bytes(b)
    };
    let reserve_b = token_amount(&ctx.accounts.token_b_vault.to_account_info())?;
    require!(amount <= max_buyback(reserve_b), HerdError::BuybackSizeOutOfBounds);
    let min_out = buyback_min_out(amount, sqrt_price)?;

    let launch_key = ctx.accounts.launch.key();
    let auth_bump = ctx.bumps.breakout_authority;
    let auth_seeds: &[&[u8]] = &[BREAKOUT_AUTH_SEED, launch_key.as_ref(), &[auth_bump]];
    let auth_info = ctx.accounts.breakout_authority.to_account_info();
    let token_prog = ctx.accounts.token_program.to_account_info();
    let caller_info = ctx.accounts.caller.to_account_info();

    let token_before = ctx.accounts.breakout_token.amount;

    // 1) Rezerv SOL'u: launch PDA -> wSOL hesabi (lamport), sync_native
    {
        let launch_info = ctx.accounts.launch.to_account_info();
        let wsol_info = ctx.accounts.breakout_wsol.to_account_info();
        **launch_info.try_borrow_mut_lamports()? = launch_info
            .lamports()
            .checked_sub(amount)
            .ok_or(HerdError::MathOverflow)?;
        **wsol_info.try_borrow_mut_lamports()? = wsol_info
            .lamports()
            .checked_add(amount)
            .ok_or(HerdError::MathOverflow)?;
    }
    {
        let l = &mut ctx.accounts.launch;
        l.burn_reserve = l.burn_reserve.checked_sub(amount).ok_or(HerdError::MathOverflow)?;
    }
    assert_launch_rent(&ctx.accounts.launch)?;
    sync_native(CpiContext::new(
        token_prog.clone(),
        SyncNative { account: ctx.accounts.breakout_wsol.to_account_info() },
    ))?;

    // 2) DAMM swap (hesap sirasi: IDL). referral_token_account opsiyonel: None => program id.
    {
        let a = &ctx.accounts;
        let mut data = Vec::with_capacity(8 + 16);
        data.extend_from_slice(&DISC_SWAP);
        data.extend_from_slice(&amount.to_le_bytes()); // amount_in
        data.extend_from_slice(&min_out.to_le_bytes()); // minimum_amount_out
        let metas = vec![
            AccountMeta::new_readonly(a.damm_pool_authority.key(), false),
            AccountMeta::new(a.pool.key(), false),
            AccountMeta::new(a.breakout_wsol.key(), false),  // input (token_b = wSOL)
            AccountMeta::new(a.breakout_token.key(), false), // output (token_a)
            AccountMeta::new(a.token_a_vault.key(), false),
            AccountMeta::new(a.token_b_vault.key(), false),
            AccountMeta::new_readonly(a.mint.key(), false),
            AccountMeta::new_readonly(a.wsol_mint.key(), false),
            AccountMeta::new_readonly(auth_info.key(), true), // payer / input sahibi
            AccountMeta::new_readonly(a.token_program.key(), false),
            AccountMeta::new_readonly(a.token_program.key(), false),
            AccountMeta::new_readonly(a.damm_program.key(), false), // referral = None
            AccountMeta::new_readonly(a.damm_event_authority.key(), false),
            AccountMeta::new_readonly(a.damm_program.key(), false),
        ];
        let infos = [
            a.damm_pool_authority.to_account_info(),
            a.pool.to_account_info(),
            a.breakout_wsol.to_account_info(),
            a.breakout_token.to_account_info(),
            a.token_a_vault.to_account_info(),
            a.token_b_vault.to_account_info(),
            a.mint.to_account_info(),
            a.wsol_mint.to_account_info(),
            auth_info.clone(),
            token_prog.clone(),
            a.damm_event_authority.to_account_info(),
            a.damm_program.to_account_info(),
        ];
        invoke_signed(
            &Instruction { program_id: DAMM_PROGRAM_ID, accounts: metas, data },
            &infos,
            &[auth_seeds],
        )?;
    }

    // 3) Gelen token'i (ve olasi bagislari) yak, gecici hesaplari kapat (rent caller'a)
    ctx.accounts.breakout_token.reload()?;
    let held = ctx.accounts.breakout_token.amount;
    let bought = held.checked_sub(token_before).ok_or(HerdError::MathOverflow)?;
    require!(bought >= min_out, HerdError::SlippageExceeded);
    burn(
        CpiContext::new_with_signer(
            token_prog.clone(),
            Burn {
                mint: ctx.accounts.mint.to_account_info(),
                from: ctx.accounts.breakout_token.to_account_info(),
                authority: auth_info.clone(),
            },
            &[auth_seeds],
        ),
        held,
    )?;
    for ata in [
        ctx.accounts.breakout_token.to_account_info(),
        ctx.accounts.breakout_wsol.to_account_info(),
    ] {
        close_account(CpiContext::new_with_signer(
            token_prog.clone(),
            CloseAccount { account: ata, destination: caller_info.clone(), authority: auth_info.clone() },
            &[auth_seeds],
        ))?;
    }

    let l = &mut ctx.accounts.launch;
    l.total_bought_back = l.total_bought_back.checked_add(bought).ok_or(HerdError::MathOverflow)?;
    emit!(BuybackBurned {
        launch: launch_key,
        caller: ctx.accounts.caller.key(),
        sol_spent: amount,
        tokens_burned: bought,
        reserve_left: l.burn_reserve,
    });
    Ok(())
}
