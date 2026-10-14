//! breakout: Curve dolunca birikimi Meteora DAMM v2 (cp-amm) havuzuna tasir.
//!
//! Hesap sirasi ve argumanlar resmi IDL'den (cp_amm v0.2.5) ve `initialize_pool` kaynagindan
//! dogrulandi. Akis (hepsi tek islemde, atomik):
//!  1. launch PDA'sindan `accepted_total + (vs - vs0)` kadar SOL breakout_authority PDA'sina
//!     (system-owned) tasinir. Claim edilmemis cap-fazlasi iadeleri launch'ta KALIR.
//!  2. SOL'un (maliyet butcesi haric) kismi wSOL'e sarilir; %20 LP token'i mint edilir.
//!  3. DAMM `initialize_pool` CPI: payer + creator = breakout_authority PDA, yani pozisyon
//!     NFT'sini program tutar. token_a = bizim mint, token_b = wSOL (ucretler SOL'de birikir).
//!  4. Havuza giren miktar planin >= %99'u degilse islem geri alinir.
//!  5. Pozisyon KALICI kilitlenir (`permanent_lock_position`).
//!  6. Artik token yakilir, artik wSOL/lamport protokol havuzuna doner.
//!  7. Mint yetkisi iptal edilir (Stampede token'lari settle'da zaten basildi).
//!
//! UYARI: SBF derlemesi ve devnet testi YAPILMADI. DAMM CPI hesap sirasi IDL'den alindi ama
//! entegrasyon testi (localnet/devnet, SDK ile uretilen instruction ile meta karsilastirmasi)
//! mainnet oncesi zorunlu.

use anchor_lang::prelude::*;
use anchor_lang::solana_program::instruction::{AccountMeta, Instruction};
use anchor_lang::solana_program::program::invoke_signed;
use anchor_lang::system_program::{transfer, Transfer};
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::spl_token::instruction::AuthorityType;
use anchor_spl::token::{
    burn, close_account, mint_to, set_authority, sync_native, Burn, CloseAccount, Mint, MintTo,
    SetAuthority, SyncNative, Token, TokenAccount,
};

use crate::breakout::{lp_tokens, pool_sol};
use crate::constants::*;
use crate::errors::HerdError;
use crate::events::*;
use crate::state::*;
use crate::trade::assert_launch_rent;

const DISC_INITIALIZE_POOL: [u8; 8] = [95, 180, 10, 172, 84, 174, 232, 40];
const DISC_PERMANENT_LOCK: [u8; 8] = [165, 176, 125, 6, 231, 171, 186, 213];
/// Position (bytemuck) icinde unlocked_liquidity ofseti: 8 (discriminator) + 144 (IDL'den hesaplandi)
const POSITION_UNLOCKED_OFFSET: usize = 152;

#[derive(Accounts)]
pub struct BreakoutLaunch<'info> {
    /// Herkes cagirabilir (permissionless); sadece islem ucretini ve ATA rent'ini oder.
    #[account(mut)]
    pub caller: Signer<'info>,

    #[account(
        seeds = [CONFIG_SEED], bump = config.bump,
        constraint = config.damm_config != Pubkey::default() @ HerdError::InvalidDammConfig
    )]
    pub config: Box<Account<'info, Config>>,

    #[account(mut, seeds = [LAUNCH_SEED, launch.mint.as_ref()], bump = launch.bump)]
    pub launch: Box<Account<'info, Launch>>,

    #[account(mut, address = launch.mint)]
    pub mint: Box<Account<'info, Mint>>,

    #[account(mut, seeds = [PROTOCOL_POOL_SEED], bump = protocol_pool.bump)]
    pub protocol_pool: Box<Account<'info, ProtocolPool>>,

    /// CHECK: system-owned PDA; DAMM'de payer + creator. Hicbir veri tasimaz (system-owned kalmali).
    #[account(mut, seeds = [BREAKOUT_AUTH_SEED, launch.key().as_ref()], bump)]
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

    // ---- Meteora DAMM v2 hesaplari (IDL sirasi; seed dogrulamasini DAMM yapar) ----
    /// CHECK: adres Config'teki damm_config ile birebir eslesmeli.
    #[account(address = config.damm_config @ HerdError::InvalidDammConfig)]
    pub damm_config: UncheckedAccount<'info>,
    /// CHECK: cp-amm program
    #[account(address = DAMM_PROGRAM_ID)]
    pub damm_program: UncheckedAccount<'info>,
    /// CHECK: cp-amm global pool authority
    #[account(address = DAMM_POOL_AUTHORITY)]
    pub damm_pool_authority: UncheckedAccount<'info>,
    /// CHECK: ["__event_authority"] @ cp-amm
    #[account(seeds = [b"__event_authority"], bump, seeds::program = DAMM_PROGRAM_ID)]
    pub damm_event_authority: UncheckedAccount<'info>,
    /// Yeni pozisyon NFT mint'i: istemci taze keypair uretir ve imzalar.
    #[account(mut)]
    pub position_nft_mint: Signer<'info>,
    /// CHECK: DAMM olusturur
    #[account(mut)]
    pub position_nft_account: UncheckedAccount<'info>,
    /// CHECK: DAMM olusturur
    #[account(mut)]
    pub pool: UncheckedAccount<'info>,
    /// CHECK: DAMM olusturur
    #[account(mut)]
    pub position: UncheckedAccount<'info>,
    /// CHECK: DAMM olusturur (token_a = bizim mint)
    #[account(mut)]
    pub token_a_vault: UncheckedAccount<'info>,
    /// CHECK: DAMM olusturur (token_b = wSOL)
    #[account(mut)]
    pub token_b_vault: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
    /// CHECK: Token-2022 (position NFT icin)
    #[account(address = TOKEN_2022_PROGRAM_ID)]
    pub token_2022_program: UncheckedAccount<'info>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub(crate) fn token_amount(ai: &AccountInfo) -> Result<u64> {
    let d = ai.try_borrow_data()?;
    Ok(TokenAccount::try_deserialize(&mut &d[..])?.amount)
}

pub fn breakout_launch<'info>(
    ctx: Context<'_, '_, 'info, 'info, BreakoutLaunch<'info>>,
    liquidity: u128,
    sqrt_price: u128,
) -> Result<()> {
    require!(ctx.accounts.launch.state == LaunchState::CurveFull, HerdError::InvalidState);
    require!(liquidity > 0 && sqrt_price > 0, HerdError::BreakoutPoolMismatch);

    let l = &ctx.accounts.launch;
    let cost = ctx.accounts.config.breakout_cost_lamports;
    let total_sol = pool_sol(l.accepted_total, l.virtual_sol, l.initial_virtual_sol, 0)?;
    let wsol_amount = pool_sol(l.accepted_total, l.virtual_sol, l.initial_virtual_sol, cost)?;
    let lp = lp_tokens()?;

    let launch_key = ctx.accounts.launch.key();
    let mint_key = ctx.accounts.mint.key();
    let launch_bump = ctx.accounts.launch.bump;
    let auth_bump = ctx.bumps.breakout_authority;
    let launch_seeds: &[&[u8]] = &[LAUNCH_SEED, mint_key.as_ref(), &[launch_bump]];
    let auth_seeds: &[&[u8]] = &[BREAKOUT_AUTH_SEED, launch_key.as_ref(), &[auth_bump]];

    let auth_info = ctx.accounts.breakout_authority.to_account_info();
    let token_prog = ctx.accounts.token_program.to_account_info();
    let sys_prog = ctx.accounts.system_program.to_account_info();

    // 1) SOL: launch PDA -> breakout_authority (launch'taki iade emanetlerine dokunulmaz)
    {
        let launch_info = ctx.accounts.launch.to_account_info();
        **launch_info.try_borrow_mut_lamports()? = launch_info
            .lamports()
            .checked_sub(total_sol)
            .ok_or(HerdError::MathOverflow)?;
        **auth_info.try_borrow_mut_lamports()? = auth_info
            .lamports()
            .checked_add(total_sol)
            .ok_or(HerdError::MathOverflow)?;
    }
    assert_launch_rent(&ctx.accounts.launch)?;

    // 2) wSOL'e sar + LP token'i mint et
    transfer(
        CpiContext::new_with_signer(
            sys_prog.clone(),
            Transfer { from: auth_info.clone(), to: ctx.accounts.breakout_wsol.to_account_info() },
            &[auth_seeds],
        ),
        wsol_amount,
    )?;
    sync_native(CpiContext::new(
        token_prog.clone(),
        SyncNative { account: ctx.accounts.breakout_wsol.to_account_info() },
    ))?;
    mint_to(
        CpiContext::new_with_signer(
            token_prog.clone(),
            MintTo {
                mint: ctx.accounts.mint.to_account_info(),
                to: ctx.accounts.breakout_token.to_account_info(),
                authority: ctx.accounts.launch.to_account_info(),
            },
            &[launch_seeds],
        ),
        lp,
    )?;

    // 3) DAMM initialize_pool (hesap sirasi: IDL)
    let mut data = Vec::with_capacity(8 + 16 + 16 + 1);
    data.extend_from_slice(&DISC_INITIALIZE_POOL);
    data.extend_from_slice(&liquidity.to_le_bytes());
    data.extend_from_slice(&sqrt_price.to_le_bytes());
    data.push(0); // activation_point = None -> havuz hemen aktif (gecikmeli aktivasyon yok)

    let a = &ctx.accounts;
    let mut metas = vec![
        AccountMeta::new_readonly(auth_info.key(), false), // creator (NFT sahibi)
        AccountMeta::new(a.position_nft_mint.key(), true),
        AccountMeta::new(a.position_nft_account.key(), false),
        AccountMeta::new(auth_info.key(), true), // payer
        AccountMeta::new_readonly(a.damm_config.key(), false),
        AccountMeta::new_readonly(a.damm_pool_authority.key(), false),
        AccountMeta::new(a.pool.key(), false),
        AccountMeta::new(a.position.key(), false),
        AccountMeta::new_readonly(a.mint.key(), false), // token_a_mint
        AccountMeta::new_readonly(a.wsol_mint.key(), false), // token_b_mint
        AccountMeta::new(a.token_a_vault.key(), false),
        AccountMeta::new(a.token_b_vault.key(), false),
        AccountMeta::new(a.breakout_token.key(), false), // payer_token_a
        AccountMeta::new(a.breakout_wsol.key(), false), // payer_token_b
        AccountMeta::new_readonly(a.token_program.key(), false), // token_a_program
        AccountMeta::new_readonly(a.token_program.key(), false), // token_b_program
        AccountMeta::new_readonly(a.token_2022_program.key(), false),
        AccountMeta::new_readonly(a.system_program.key(), false),
        AccountMeta::new_readonly(a.damm_event_authority.key(), false),
        AccountMeta::new_readonly(a.damm_program.key(), false),
    ];
    let mut infos: Vec<AccountInfo<'info>> = vec![
        auth_info.clone(),
        a.position_nft_mint.to_account_info(),
        a.position_nft_account.to_account_info(),
        a.damm_config.to_account_info(),
        a.damm_pool_authority.to_account_info(),
        a.pool.to_account_info(),
        a.position.to_account_info(),
        a.mint.to_account_info(),
        a.wsol_mint.to_account_info(),
        a.token_a_vault.to_account_info(),
        a.token_b_vault.to_account_info(),
        a.breakout_token.to_account_info(),
        a.breakout_wsol.to_account_info(),
        token_prog.clone(),
        a.token_2022_program.to_account_info(),
        sys_prog.clone(),
        a.damm_event_authority.to_account_info(),
        a.damm_program.to_account_info(),
    ];
    // Token badge gerekiyorsa (permissionless olmayan mint) remaining_accounts[0..2] olarak iletilir.
    for r in ctx.remaining_accounts.iter() {
        metas.push(AccountMeta::new_readonly(r.key(), false));
        infos.push(r.clone());
    }
    invoke_signed(
        &Instruction { program_id: DAMM_PROGRAM_ID, accounts: metas, data },
        &infos,
        &[auth_seeds],
    )?;

    // 4) Havuza giren miktar planin >= %99'u olmali
    let sol_in_pool = token_amount(&ctx.accounts.token_b_vault.to_account_info())?;
    let tokens_in_pool = token_amount(&ctx.accounts.token_a_vault.to_account_info())?;
    let min_sol = ((wsol_amount as u128) * (BREAKOUT_MIN_FILL_BPS as u128) / BPS_DENOM as u128) as u64;
    let min_tok = ((lp as u128) * (BREAKOUT_MIN_FILL_BPS as u128) / BPS_DENOM as u128) as u64;
    require!(sol_in_pool >= min_sol && tokens_in_pool >= min_tok, HerdError::BreakoutPoolMismatch);

    // 5) Pozisyonu kalici kilitle: kilitlenecek miktar = position.unlocked_liquidity
    let unlocked: u128 = {
        let pos = ctx.accounts.position.to_account_info();
        require_keys_eq!(*pos.owner, DAMM_PROGRAM_ID, HerdError::BreakoutPoolMismatch);
        let d = pos.try_borrow_data()?;
        require!(d.len() >= POSITION_UNLOCKED_OFFSET + 16, HerdError::BreakoutPoolMismatch);
        let mut b = [0u8; 16];
        b.copy_from_slice(&d[POSITION_UNLOCKED_OFFSET..POSITION_UNLOCKED_OFFSET + 16]);
        u128::from_le_bytes(b)
    };
    require!(unlocked > 0, HerdError::BreakoutPoolMismatch);
    let mut lock_data = Vec::with_capacity(8 + 16);
    lock_data.extend_from_slice(&DISC_PERMANENT_LOCK);
    lock_data.extend_from_slice(&unlocked.to_le_bytes());
    let a = &ctx.accounts;
    invoke_signed(
        &Instruction {
            program_id: DAMM_PROGRAM_ID,
            accounts: vec![
                AccountMeta::new(a.pool.key(), false),
                AccountMeta::new(a.position.key(), false),
                AccountMeta::new_readonly(a.position_nft_account.key(), false),
                AccountMeta::new_readonly(auth_info.key(), true), // signer = NFT sahibi
                AccountMeta::new_readonly(a.damm_event_authority.key(), false),
                AccountMeta::new_readonly(a.damm_program.key(), false),
            ],
            data: lock_data,
        },
        &[
            a.pool.to_account_info(),
            a.position.to_account_info(),
            a.position_nft_account.to_account_info(),
            auth_info.clone(),
            a.damm_event_authority.to_account_info(),
            a.damm_program.to_account_info(),
        ],
        &[auth_seeds],
    )?;

    // 6) Artiklari temizle: artik token yak, ATA'lari kapat (lamport'lar authority'ye doner)
    ctx.accounts.breakout_token.reload()?;
    let left_tok = ctx.accounts.breakout_token.amount;
    if left_tok > 0 {
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
            left_tok,
        )?;
    }
    for ata in [
        ctx.accounts.breakout_token.to_account_info(),
        ctx.accounts.breakout_wsol.to_account_info(),
    ] {
        close_account(CpiContext::new_with_signer(
            token_prog.clone(),
            CloseAccount { account: ata, destination: auth_info.clone(), authority: auth_info.clone() },
            &[auth_seeds],
        ))?;
    }

    // 7) Mint yetkisini iptal et (LP basildi; Stampede token'lari settle'da basilmisti)
    set_authority(
        CpiContext::new_with_signer(
            token_prog.clone(),
            SetAuthority {
                current_authority: ctx.accounts.launch.to_account_info(),
                account_or_mint: ctx.accounts.mint.to_account_info(),
            },
            &[launch_seeds],
        ),
        AuthorityType::MintTokens,
        None,
    )?;

    // 8) Artan SOL (maliyet butcesinden kalan + artik wSOL) protokol havuzuna
    let leftover = auth_info.lamports();
    if leftover > 0 {
        transfer(
            CpiContext::new_with_signer(
                sys_prog,
                Transfer { from: auth_info.clone(), to: ctx.accounts.protocol_pool.to_account_info() },
                &[auth_seeds],
            ),
            leftover,
        )?;
        let pp = &mut ctx.accounts.protocol_pool;
        pp.total_in = pp.total_in.checked_add(leftover).ok_or(HerdError::MathOverflow)?;
    }

    {
        let l = &mut ctx.accounts.launch;
        l.state = LaunchState::Breakout;
        l.dex_pool = ctx.accounts.pool.key();
        l.dex_position = ctx.accounts.position.key();
        l.position_nft_mint = ctx.accounts.position_nft_mint.key();
    }

    emit!(BreakoutDone {
        launch: launch_key,
        pool: ctx.accounts.pool.key(),
        position: ctx.accounts.position.key(),
        sol_in_pool,
        tokens_in_pool,
        locked_liquidity: unlocked,
        leftover_to_protocol: leftover,
    });
    Ok(())
}
