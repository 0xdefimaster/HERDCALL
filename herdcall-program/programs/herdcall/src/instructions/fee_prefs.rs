use anchor_lang::prelude::*;

use crate::constants::*;
use crate::errors::HerdError;
use crate::events::*;
use crate::state::*;

/// Creator, LP ucret payinin (%60) nereye gidecegini ve token payinin ne kadarinin
/// yakilacagini belirler. Breakout'tan once de sonra da cagrilabilir; sonraki her
/// `claim_position_fee` guncel degerleri kullanir. `burn_bps` YALNIZCA artirilabilir.
#[derive(Accounts)]
pub struct SetFeePrefs<'info> {
    pub creator: Signer<'info>,

    #[account(
        mut,
        seeds = [LAUNCH_SEED, launch.mint.as_ref()], bump = launch.bump,
        has_one = creator
    )]
    pub launch: Account<'info, Launch>,
}

pub fn set_fee_prefs(ctx: Context<SetFeePrefs>, payout_wallet: Pubkey, burn_bps: u16) -> Result<()> {
    require!(payout_wallet != Pubkey::default(), HerdError::InvalidFeePrefs);
    require!(burn_bps as u64 <= BPS_DENOM, HerdError::InvalidFeePrefs);
    // Yakim orani sadece artirilabilir: alici, creator'in sonradan dusurmeyecegine guvenebilir.
    // Odeme cuzdani serbestce degistirilebilir (burn_bps ayni gecilir).
    require!(burn_bps >= ctx.accounts.launch.creator_burn_bps, HerdError::BurnBpsCannotDecrease);
    // Pozisyon sahibi PDA'nin kendisi odeme cuzdani olamaz (ATA'lar cakisirdi).
    let (auth_pda, _) = Pubkey::find_program_address(
        &[BREAKOUT_AUTH_SEED, ctx.accounts.launch.key().as_ref()],
        ctx.program_id,
    );
    require!(payout_wallet != auth_pda, HerdError::InvalidFeePrefs);

    let l = &mut ctx.accounts.launch;
    l.creator_payout = payout_wallet;
    l.creator_burn_bps = burn_bps;

    emit!(FeePrefsUpdated {
        launch: l.key(),
        creator: ctx.accounts.creator.key(),
        payout_wallet,
        burn_bps,
    });
    Ok(())
}
