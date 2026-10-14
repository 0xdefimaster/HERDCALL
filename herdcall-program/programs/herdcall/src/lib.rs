use anchor_lang::prelude::*;

pub mod breakout;
pub mod buyback;
pub mod constants;
pub mod curve;
pub mod errors;
pub mod events;
pub mod fees;
pub mod instructions;
pub mod stampede;
pub mod state;
pub mod trade;

use instructions::*;
use state::ParamsArgs;

declare_id!("8XTPwS47mzBUPWP1GhGH7M5dMBwA4pJ8eCJQqffzk4yd");

#[program]
pub mod herdcall {
    use super::*;

    // ---- yonetim ----
    pub fn initialize_config(ctx: Context<InitializeConfig>, args: InitializeConfigArgs) -> Result<()> {
        instructions::admin::initialize_config(ctx, args)
    }
    pub fn update_params(ctx: Context<UpdateParams>, params: ParamsArgs) -> Result<()> {
        instructions::admin::update_params(ctx, params)
    }
    pub fn propose_authority(ctx: Context<ProposeAuthority>, new_authority: Pubkey) -> Result<()> {
        instructions::admin::propose_authority(ctx, new_authority)
    }
    pub fn accept_authority(ctx: Context<AcceptAuthority>) -> Result<()> {
        instructions::admin::accept_authority(ctx)
    }

    /// Curve dolunca birikimi DAMM v2 havuzuna tasir, LP'yi kalici kilitler, mint yetkisini iptal eder.
    pub fn breakout<'info>(
        ctx: Context<'_, '_, 'info, 'info, BreakoutLaunch<'info>>,
        liquidity: u128,
        sqrt_price: u128,
    ) -> Result<()> {
        instructions::breakout_launch::breakout_launch(ctx, liquidity, sqrt_price)
    }

    /// Kilitli pozisyonun ucretini toplar; creator / protokol / DaoPool arasinda boler (permissionless).
    pub fn claim_position_fee(ctx: Context<ClaimPositionFee>) -> Result<()> {
        instructions::claim_position_fee::claim_position_fee(ctx)
    }

    /// Creator: LP ucret payinin odeme cuzdani ve token payindan yakim orani.
    pub fn set_fee_prefs(ctx: Context<SetFeePrefs>, payout_wallet: Pubkey, burn_bps: u16) -> Result<()> {
        instructions::fee_prefs::set_fee_prefs(ctx, payout_wallet, burn_bps)
    }

    /// Creator: token'i program escrow'una kilitler (gelir hakkinin sarti).
    pub fn commit_tokens(ctx: Context<CommitTokens>, amount: u64) -> Result<()> {
        instructions::creator_commit::commit_tokens(ctx, amount)
    }

    /// Creator: escrow'dan token ceker. Taahhut esigin altina inerse gelir hakki KALICI kaybolur.
    pub fn withdraw_commit(ctx: Context<WithdrawCommit>, amount: u64) -> Result<()> {
        instructions::creator_commit::withdraw_commit(ctx, amount)
    }

    /// Permissionless: burn_reserve'deki SOL ile havuzdan token alip yakar.
    pub fn buyback_burn(ctx: Context<BuybackBurn>, amount: u64) -> Result<()> {
        instructions::buyback_burn::buyback_burn(ctx, amount)
    }

    /// Sadece authority: DaoPool'un token ATA'sindan token ceker.
    pub fn withdraw_dao_tokens(ctx: Context<WithdrawDaoTokens>, amount: u64) -> Result<()> {
        instructions::withdraw_pool_tokens::withdraw_dao_tokens(ctx, amount)
    }

    /// Sadece authority: ProtocolPool'un token ATA'sindan token ceker.
    pub fn withdraw_protocol_tokens(ctx: Context<WithdrawProtocolTokens>, amount: u64) -> Result<()> {
        instructions::withdraw_pool_tokens::withdraw_protocol_tokens(ctx, amount)
    }

    pub fn withdraw_dao(ctx: Context<WithdrawDao>, amount: u64) -> Result<()> {
        instructions::withdraw_dao::withdraw_dao(ctx, amount)
    }

    // ---- launch yasam dongusu ----
    pub fn create_launch(ctx: Context<CreateLaunch>, lobby_secs: i64, bonded: bool) -> Result<()> {
        instructions::create_launch::create_launch(ctx, lobby_secs, bonded)
    }
    pub fn lobby_deposit(ctx: Context<LobbyDeposit>, amount: u64) -> Result<()> {
        instructions::lobby_deposit::lobby_deposit(ctx, amount)
    }
    pub fn settle_stampede(ctx: Context<SettleStampede>) -> Result<()> {
        instructions::settle_stampede::settle_stampede(ctx)
    }
    pub fn claim_stampede(ctx: Context<ClaimStampede>) -> Result<()> {
        instructions::claim_stampede::claim_stampede(ctx)
    }

    // ---- bonding curve ----
    pub fn buy(ctx: Context<Buy>, gross_in: u64, min_tokens_out: u64) -> Result<()> {
        instructions::buy::buy(ctx, gross_in, min_tokens_out)
    }
    pub fn sell(ctx: Context<Sell>, tokens_in: u64, min_sol_out: u64) -> Result<()> {
        instructions::sell::sell(ctx, tokens_in, min_sol_out)
    }
}
