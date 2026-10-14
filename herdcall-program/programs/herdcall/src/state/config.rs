use anchor_lang::prelude::*;

use crate::constants::*;
use crate::errors::HerdError;

/// Global ayarlar. `authority` baslangicta ekip multisig'i (Squads + timelock);
/// ileride DAO'ya tek bir `propose/accept_authority` ile devredilir.
#[account]
#[derive(InitSpace)]
pub struct Config {
    pub authority: Pubkey,
    /// Iki adimli devir: default pubkey ise bekleyen devir yok.
    pub pending_authority: Pubkey,
    /// Protokol payi hedef cuzdanlari (herkese acik, §7)
    pub treasury: Pubkey,
    pub builders_fund: Pubkey,
    pub ops_wallet: Pubkey,
    /// Rekt-back secim orani (bps). Sinirlar: REKT_SELECT_BPS_MIN..MAX
    pub rekt_select_bps: u16,
    /// Bir odeme turunda havuz bakiyesinin ust siniri (bps)
    pub rekt_payout_cap_bps: u16,
    /// Bu bakiyenin altinda Rekt-back odemesi yapilmaz
    pub rekt_reserve_floor: u64,
    /// Yeni launch'lara snapshot'lanir (calisan launch etkilenmez)
    pub min_stampede_lamports: u64,
    pub stampede_cap_lamports: u64,
    /// Stampede claim ucreti (bps, kabul edilen SOL uzerinden). 0..=CLAIM_FEE_BPS_MAX
    pub claim_fee_bps: u16,
    /// Breakout'ta kullanilacak Meteora DAMM v2 STATIK config hesabi (public, havuz olusturma
    /// izni herkese acik olmali). Authority belirler; breakout bu adresle birebir eslesmeli.
    pub damm_config: Pubkey,
    /// Havuz acma maliyeti butcesi (lamport): breakout SOL'undan ayrilir, artan protokol havuzuna doner.
    pub breakout_cost_lamports: u64,
    /// Yeni launch'lara snapshot'lanir: creator'in gelir hakki icin kilitlemesi gereken minimum
    /// token (toplam arzin bps'i). 0 => taahhut zorunlulugu yok. Sinir: CREATOR_COMMIT_BPS_MAX
    pub min_creator_commit_bps: u16,
    pub paused: bool,
    pub bump: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct ParamsArgs {
    pub rekt_select_bps: u16,
    pub rekt_payout_cap_bps: u16,
    pub rekt_reserve_floor: u64,
    pub min_stampede_lamports: u64,
    pub stampede_cap_lamports: u64,
    pub claim_fee_bps: u16,
    pub damm_config: Pubkey,
    pub breakout_cost_lamports: u64,
    pub min_creator_commit_bps: u16,
    pub paused: bool,
}

impl ParamsArgs {
    pub fn validate(&self) -> Result<()> {
        require!(
            (REKT_SELECT_BPS_MIN..=REKT_SELECT_BPS_MAX).contains(&self.rekt_select_bps),
            HerdError::ParamOutOfBounds
        );
        require!(
            (REKT_PAYOUT_CAP_BPS_MIN..=REKT_PAYOUT_CAP_BPS_MAX).contains(&self.rekt_payout_cap_bps),
            HerdError::ParamOutOfBounds
        );
        require!(self.claim_fee_bps <= CLAIM_FEE_BPS_MAX, HerdError::ParamOutOfBounds);
        require!(
            (BREAKOUT_COST_MIN..=BREAKOUT_COST_MAX).contains(&self.breakout_cost_lamports),
            HerdError::ParamOutOfBounds
        );
        require!(self.min_creator_commit_bps <= CREATOR_COMMIT_BPS_MAX, HerdError::ParamOutOfBounds);
        require!(
            self.min_stampede_lamports > 0
                && self.stampede_cap_lamports >= self.min_stampede_lamports,
            HerdError::ParamOutOfBounds
        );
        Ok(())
    }
}
