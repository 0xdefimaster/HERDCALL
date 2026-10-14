//! claim_position_fee: Kalici kilitli DAMM v2 pozisyonunun biriken ucretini toplar ve dagitir.
//! Permissionless: herkes cagirabilir; para sadece asagidaki kurallara gore hareket eder.
//!
//! Pozisyon NFT'sini `breakout_authority` PDA'si tutar; `claim_position_fee` CPI'sinin sahibi
//! (signer) odur. Akis (tek islemde, atomik):
//!  1. DAMM `claim_position_fee` CPI: ucretler PDA'nin gecici token hesaplarina (ATA) gelir.
//!     Dagitilacak miktar = CPI oncesi/sonrasi bakiye FARKI (bagis dagitima karismaz).
//!  2. Her iki token icin `split_lp_fees`: %60 creator / %20 protokol / %20 DaoPool
//!     (yuvarlama artigi protokole).
//!  3. Creator payi: `creator_eligible()` (taahhut tamam + hak kaybi yok) ise
//!     `creator_burn_bps` kismi YAKIM'a (token: dogrudan yakilir; SOL: buyback-burn rezervine),
//!     kalani `creator_payout` cuzdanina gider. Hak yoksa payin TAMAMI yakima gider.
//!  4. Protokol ve DAO'nun TOKEN payi, havuzlarinin ATA'larinda birikir (yakilmaz);
//!     `withdraw_dao_tokens` / `withdraw_protocol_tokens` ile authority ceker.
//!     SOL payi dogrudan havuz PDA'larina gider.
//!  5. SOL (wSOL) hesabi kapatilarak unwrap edilir; SOL caller uzerinden dagitilir.
//!     Cuzdan rent-muafiyetinin altinda kalacaksa creator payi protokol havuzuna yonlenir.
//!  6. Gecici ATA'lar kapatilir; rent'i yatiran caller geri alir (net maliyet 0).
//!
//! Hesap sayisi fazla: istemci address lookup table kullanmali.
//! UYARI: SBF derlemesi ve devnet testi YAPILMADI. Hesap sirasi/diskriminator IDL'den
//! (cp_amm v0.2.5) dogrulandi; entegrasyon testi mainnet oncesi zorunlu.

use anchor_lang::prelude::*;
use anchor_lang::solana_program::instruction::{AccountMeta, Instruction};
use anchor_lang::solana_program::program::invoke_signed;
use anchor_lang::system_program::{transfer as sys_transfer, Transfer as SysTransfer};
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{
    burn, close_account, transfer as token_transfer, Burn, CloseAccount, Mint, Token, TokenAccount,
    Transfer as TokenTransfer,
};

use crate::constants::*;
use crate::errors::HerdError;
use crate::events::*;
use crate::fees::{split_creator_share, split_lp_fees};
use crate::state::*;

const DISC_CLAIM_POSITION_FEE: [u8; 8] = [180, 38, 154, 17, 133, 33, 162, 211];
/// DAMM `initialize_pool`'daki position_nft_account PDA tohumu (IDL)
const POSITION_NFT_ACCOUNT_SEED: &[u8] = b"position_nft_account";

#[derive(Accounts)]
pub struct ClaimPositionFee<'info> {
    /// Herkes cagirabilir. Gecici ATA rent'ini oder ve ayni islemde geri alir.
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

    #[account(mut, seeds = [PROTOCOL_POOL_SEED], bump = protocol_pool.bump)]
    pub protocol_pool: Box<Account<'info, ProtocolPool>>,

    #[account(mut, seeds = [DAO_POOL_SEED], bump = dao_pool.bump)]
    pub dao_pool: Box<Account<'info, DaoPool>>,

    /// CHECK: system-owned PDA; pozisyon NFT'sinin sahibi (breakout'ta olusturuldu).
    #[account(seeds = [BREAKOUT_AUTH_SEED, launch.key().as_ref()], bump)]
    pub breakout_authority: UncheckedAccount<'info>,

    #[account(address = anchor_spl::token::spl_token::native_mint::ID)]
    pub wsol_mint: Box<Account<'info, Mint>>,

    /// Gecici: DAMM'den gelen token ucreti buraya iner; islem sonunda kapanir.
    #[account(
        init_if_needed, payer = caller,
        associated_token::mint = mint, associated_token::authority = breakout_authority
    )]
    pub breakout_token: Box<Account<'info, TokenAccount>>,

    /// Gecici: DAMM'den gelen wSOL ucreti buraya iner; islem sonunda kapanir (unwrap).
    #[account(
        init_if_needed, payer = caller,
        associated_token::mint = wsol_mint, associated_token::authority = breakout_authority
    )]
    pub breakout_wsol: Box<Account<'info, TokenAccount>>,

    /// CHECK: creator'in belirledigi odeme cuzdani (`Launch.creator_payout`).
    #[account(mut, address = launch.creator_payout)]
    pub payout_wallet: UncheckedAccount<'info>,

    /// Odeme cuzdaninin token ATA'si. Yoksa caller olusturur (bir kerelik rent).
    #[account(
        init_if_needed, payer = caller,
        associated_token::mint = mint, associated_token::authority = payout_wallet
    )]
    pub payout_token: Box<Account<'info, TokenAccount>>,

    /// Protokol / DAO token paylarinin biriktigi ATA'lar (sahipleri havuz PDA'lari). Yoksa caller acar.
    #[account(
        init_if_needed, payer = caller,
        associated_token::mint = mint, associated_token::authority = protocol_pool
    )]
    pub protocol_token: Box<Account<'info, TokenAccount>>,
    #[account(
        init_if_needed, payer = caller,
        associated_token::mint = mint, associated_token::authority = dao_pool
    )]
    pub dao_token: Box<Account<'info, TokenAccount>>,

    // ---- Meteora DAMM v2 (IDL: claim_position_fee) ----
    /// CHECK: havuz; breakout'ta kaydedilen adresle birebir eslesmeli.
    #[account(address = launch.dex_pool)]
    pub pool: UncheckedAccount<'info>,
    /// CHECK: pozisyon; breakout'ta kaydedilen adresle birebir eslesmeli.
    #[account(mut, address = launch.dex_position)]
    pub position: UncheckedAccount<'info>,
    /// CHECK: DAMM, pool'a bagli oldugunu CPI'da dogrular.
    #[account(mut)]
    pub token_a_vault: UncheckedAccount<'info>,
    /// CHECK: DAMM, pool'a bagli oldugunu CPI'da dogrular.
    #[account(mut)]
    pub token_b_vault: UncheckedAccount<'info>,
    /// CHECK: breakout'ta kaydedilen NFT mint'inden turetilen DAMM PDA'si (Token-2022 hesabi).
    #[account(
        seeds = [POSITION_NFT_ACCOUNT_SEED, launch.position_nft_mint.as_ref()],
        bump, seeds::program = DAMM_PROGRAM_ID
    )]
    pub position_nft_account: UncheckedAccount<'info>,
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

pub fn claim_position_fee(ctx: Context<ClaimPositionFee>) -> Result<()> {
    let launch_key = ctx.accounts.launch.key();
    let auth_bump = ctx.bumps.breakout_authority;
    let auth_seeds: &[&[u8]] = &[BREAKOUT_AUTH_SEED, launch_key.as_ref(), &[auth_bump]];

    let auth_info = ctx.accounts.breakout_authority.to_account_info();
    let token_prog = ctx.accounts.token_program.to_account_info();
    let sys_prog = ctx.accounts.system_program.to_account_info();
    let caller_info = ctx.accounts.caller.to_account_info();

    let token_before = ctx.accounts.breakout_token.amount;
    let sol_before = ctx.accounts.breakout_wsol.amount;

    // 1) DAMM claim_position_fee (hesap sirasi: IDL)
    {
        let a = &ctx.accounts;
        let metas = vec![
            AccountMeta::new_readonly(a.damm_pool_authority.key(), false),
            AccountMeta::new_readonly(a.pool.key(), false),
            AccountMeta::new(a.position.key(), false),
            AccountMeta::new(a.breakout_token.key(), false), // token_a_account
            AccountMeta::new(a.breakout_wsol.key(), false),  // token_b_account
            AccountMeta::new(a.token_a_vault.key(), false),
            AccountMeta::new(a.token_b_vault.key(), false),
            AccountMeta::new_readonly(a.mint.key(), false),      // token_a_mint
            AccountMeta::new_readonly(a.wsol_mint.key(), false), // token_b_mint
            AccountMeta::new_readonly(a.position_nft_account.key(), false),
            AccountMeta::new_readonly(auth_info.key(), true), // owner (NFT sahibi)
            AccountMeta::new_readonly(a.token_program.key(), false), // token_a_program
            AccountMeta::new_readonly(a.token_program.key(), false), // token_b_program
            AccountMeta::new_readonly(a.damm_event_authority.key(), false),
            AccountMeta::new_readonly(a.damm_program.key(), false),
        ];
        let infos = [
            a.damm_pool_authority.to_account_info(),
            a.pool.to_account_info(),
            a.position.to_account_info(),
            a.breakout_token.to_account_info(),
            a.breakout_wsol.to_account_info(),
            a.token_a_vault.to_account_info(),
            a.token_b_vault.to_account_info(),
            a.mint.to_account_info(),
            a.wsol_mint.to_account_info(),
            a.position_nft_account.to_account_info(),
            auth_info.clone(),
            token_prog.clone(),
            a.damm_event_authority.to_account_info(),
            a.damm_program.to_account_info(),
        ];
        invoke_signed(
            &Instruction {
                program_id: DAMM_PROGRAM_ID,
                accounts: metas,
                data: DISC_CLAIM_POSITION_FEE.to_vec(),
            },
            &infos,
            &[auth_seeds],
        )?;
    }

    ctx.accounts.breakout_token.reload()?;
    ctx.accounts.breakout_wsol.reload()?;
    let claimed_token = ctx
        .accounts
        .breakout_token
        .amount
        .checked_sub(token_before)
        .ok_or(HerdError::MathOverflow)?;
    let claimed_sol = ctx
        .accounts
        .breakout_wsol
        .amount
        .checked_sub(sol_before)
        .ok_or(HerdError::MathOverflow)?;
    require!(claimed_token > 0 || claimed_sol > 0, HerdError::NothingToClaim);

    // 2) Bolme (her token ayri; yuvarlama artigi protokole)
    let (creator_tok, protocol_tok, dao_tok) = split_lp_fees(claimed_token)?;
    let (creator_sol, protocol_sol, dao_sol) = split_lp_fees(claimed_sol)?;

    // 3) Creator payi: hak yoksa tamami yakima; varsa burn_bps kadari yakima, kalani cuzdana
    let eligible = ctx.accounts.launch.creator_eligible();
    let burn_bps = if eligible { ctx.accounts.launch.creator_burn_bps } else { BPS_DENOM as u16 };
    let (creator_tok_burn, creator_tok_pay) = split_creator_share(creator_tok, burn_bps)?;
    let (creator_sol_burn, creator_sol_pay) = split_creator_share(creator_sol, burn_bps)?;

    // TOKEN tarafi: odemeler, kalan her sey yakilir (creator yakimi + bagislar)
    if creator_tok_pay > 0 {
        token_transfer(
            CpiContext::new_with_signer(
                token_prog.clone(),
                TokenTransfer {
                    from: ctx.accounts.breakout_token.to_account_info(),
                    to: ctx.accounts.payout_token.to_account_info(),
                    authority: auth_info.clone(),
                },
                &[auth_seeds],
            ),
            creator_tok_pay,
        )?;
    }
    for (to, amt) in [
        (ctx.accounts.protocol_token.to_account_info(), protocol_tok),
        (ctx.accounts.dao_token.to_account_info(), dao_tok),
    ] {
        if amt > 0 {
            token_transfer(
                CpiContext::new_with_signer(
                    token_prog.clone(),
                    TokenTransfer {
                        from: ctx.accounts.breakout_token.to_account_info(),
                        to,
                        authority: auth_info.clone(),
                    },
                    &[auth_seeds],
                ),
                amt,
            )?;
        }
    }
    let burn_amount = token_before
        .checked_add(claimed_token)
        .and_then(|x| x.checked_sub(creator_tok_pay))
        .and_then(|x| x.checked_sub(protocol_tok))
        .and_then(|x| x.checked_sub(dao_tok))
        .ok_or(HerdError::MathOverflow)?;
    if burn_amount > 0 {
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
            burn_amount,
        )?;
    }

    // 4) SOL tarafi: gecici ATA'lari kapat (rent + wSOL -> caller). Caller, ATA rent'ini
    //    baslangicta kendisi yatirdi; geri alir. Kalan SOL asagida dagitilir.
    for ata in [
        ctx.accounts.breakout_token.to_account_info(),
        ctx.accounts.breakout_wsol.to_account_info(),
    ] {
        close_account(CpiContext::new_with_signer(
            token_prog.clone(),
            CloseAccount {
                account: ata,
                destination: caller_info.clone(),
                authority: auth_info.clone(),
            },
            &[auth_seeds],
        ))?;
    }

    // Creator SOL payi: cuzdan rent-muafiyetinin altinda kalacaksa protokole yonlendir.
    let payout_info = ctx.accounts.payout_wallet.to_account_info();
    let rent_min = Rent::get()?.minimum_balance(0);
    let (creator_sol_paid, creator_sol_redirected) = if creator_sol_pay == 0 {
        (0, 0)
    } else if payout_info
        .lamports()
        .checked_add(creator_sol_pay)
        .ok_or(HerdError::MathOverflow)?
        < rent_min
    {
        (0, creator_sol_pay)
    } else {
        (creator_sol_pay, 0)
    };
    let protocol_total = protocol_sol
        .checked_add(creator_sol_redirected)
        .ok_or(HerdError::MathOverflow)?;

    pay(&sys_prog, &caller_info, &payout_info, creator_sol_paid)?;
    pay(&sys_prog, &caller_info, &ctx.accounts.protocol_pool.to_account_info(), protocol_total)?;
    pay(&sys_prog, &caller_info, &ctx.accounts.dao_pool.to_account_info(), dao_sol)?;
    // Yakilacak creator SOL'u launch PDA'sinda kalir; buyback_burn bunu harcar.
    pay(&sys_prog, &caller_info, &ctx.accounts.launch.to_account_info(), creator_sol_burn)?;

    {
        let pp = &mut ctx.accounts.protocol_pool;
        pp.total_in = pp.total_in.checked_add(protocol_total).ok_or(HerdError::MathOverflow)?;
        let dp = &mut ctx.accounts.dao_pool;
        dp.total_in = dp.total_in.checked_add(dao_sol).ok_or(HerdError::MathOverflow)?;
        let l = &mut ctx.accounts.launch;
        l.burn_reserve = l.burn_reserve.checked_add(creator_sol_burn).ok_or(HerdError::MathOverflow)?;
    }

    emit!(PositionFeeClaimed {
        launch: launch_key,
        caller: ctx.accounts.caller.key(),
        claimed_token,
        claimed_sol,
        creator_eligible: eligible,
        creator_sol_paid,
        creator_sol_redirected,
        creator_sol_to_burn_reserve: creator_sol_burn,
        creator_token_paid: creator_tok_pay,
        creator_token_burned: creator_tok_burn,
        protocol_sol: protocol_total,
        dao_sol,
        protocol_token: protocol_tok,
        dao_token: dao_tok,
    });
    Ok(())
}

/// Caller'in (system hesabi) lamport'undan hedefe SOL gonderir. Hedef program-owned havuz ya da
/// cuzdan olabilir; sifir miktar atlanir.
fn pay<'info>(
    sys_prog: &AccountInfo<'info>,
    from: &AccountInfo<'info>,
    to: &AccountInfo<'info>,
    amt: u64,
) -> Result<()> {
    if amt == 0 {
        return Ok(());
    }
    sys_transfer(
        CpiContext::new(sys_prog.clone(), SysTransfer { from: from.clone(), to: to.clone() }),
        amt,
    )
}
