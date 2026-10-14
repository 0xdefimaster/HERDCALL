use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_lang::system_program::{transfer as sol_transfer, Transfer as SolTransfer};
use anchor_spl::token::{transfer, Mint, Token, TokenAccount, Transfer};

use crate::constants::*;
use crate::errors::HerdError;
use crate::events::*;
use crate::fees::claim_fee;
use crate::state::*;

#[derive(Accounts)]
pub struct ClaimStampede<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(mut, seeds = [LAUNCH_SEED, launch.mint.as_ref()], bump = launch.bump)]
    pub launch: Account<'info, Launch>,

    #[account(mut, address = launch.mint)]
    pub mint: Account<'info, Mint>,

    /// Emir claim'de kapanir; rent kullaniciya doner (cift claim imkansiz).
    #[account(
        mut,
        close = owner,
        seeds = [ORDER_SEED, launch.key().as_ref(), owner.key().as_ref()],
        bump = order.bump,
        has_one = owner,
        constraint = order.launch == launch.key() @ HerdError::InvalidState
    )]
    pub order: Account<'info, LobbyOrder>,

    #[account(
        init_if_needed,
        payer = owner,
        associated_token::mint = mint,
        associated_token::authority = owner
    )]
    pub owner_token: Account<'info, TokenAccount>,

    /// Settle'da basilan Stampede token'larinin escrow'u (launch PDA'sinin ATA'si).
    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = launch
    )]
    pub stampede_escrow: Account<'info, TokenAccount>,

    /// Claim ucreti (SOL) buraya gider; herkese acik sayac.
    #[account(mut, seeds = [PROTOCOL_POOL_SEED], bump = protocol_pool.bump)]
    pub protocol_pool: Account<'info, ProtocolPool>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn claim_stampede(ctx: Context<ClaimStampede>) -> Result<()> {
    let l = &ctx.accounts.launch;
    let deposit = ctx.accounts.order.amount;

    let (accepted, tokens) = match l.state {
        LaunchState::Curve | LaunchState::CurveFull => crate::stampede::claim_amounts(
            deposit,
            l.total_deposited,
            l.cap_lamports,
            l.accepted_total,
            l.stampede_tokens,
        )?,
        LaunchState::Failed => (0, 0), // tam iade
        _ => return err!(HerdError::InvalidState),
    };
    let refund = deposit.checked_sub(accepted).ok_or(HerdError::MathOverflow)?;

    // Iade: launch PDA'sindan dogrudan lamport transferi (program-owned hesap).
    if refund > 0 {
        let launch_info = ctx.accounts.launch.to_account_info();
        let owner_info = ctx.accounts.owner.to_account_info();
        **launch_info.try_borrow_mut_lamports()? = launch_info
            .lamports()
            .checked_sub(refund)
            .ok_or(HerdError::MathOverflow)?;
        **owner_info.try_borrow_mut_lamports()? = owner_info
            .lamports()
            .checked_add(refund)
            .ok_or(HerdError::MathOverflow)?;
    }

    if tokens > 0 {
        let mint_key = ctx.accounts.mint.key();
        let seeds: &[&[u8]] = &[LAUNCH_SEED, mint_key.as_ref(), &[l.bump]];
        transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.stampede_escrow.to_account_info(),
                    to: ctx.accounts.owner_token.to_account_info(),
                    authority: ctx.accounts.launch.to_account_info(),
                },
                &[seeds],
            ),
            tokens,
        )?;
    }

    // Claim ucreti: kabul edilen SOL uzerinden, launch'taki rezerve DOKUNMADAN
    // kullanicinin cuzdanindan protokol havuzuna (curve solvency degismezi korunur).
    let fee = claim_fee(accepted, l.claim_fee_bps)?;
    if fee > 0 {
        sol_transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                SolTransfer {
                    from: ctx.accounts.owner.to_account_info(),
                    to: ctx.accounts.protocol_pool.to_account_info(),
                },
            ),
            fee,
        )?;
        let pp = &mut ctx.accounts.protocol_pool;
        pp.total_in = pp.total_in.checked_add(fee).ok_or(HerdError::MathOverflow)?;
    }

    emit!(StampedeClaimed {
        launch: ctx.accounts.launch.key(),
        owner: ctx.accounts.owner.key(),
        accepted_lamports: accepted,
        refund_lamports: refund,
        fee_lamports: fee,
        tokens,
    });
    Ok(())
}
