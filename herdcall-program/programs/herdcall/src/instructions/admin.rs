use anchor_lang::prelude::*;

use crate::constants::*;
use crate::errors::HerdError;
use crate::events::*;
use crate::state::*;

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct InitializeConfigArgs {
    pub treasury: Pubkey,
    pub builders_fund: Pubkey,
    pub ops_wallet: Pubkey,
    pub params: ParamsArgs,
}

#[derive(Accounts)]
pub struct InitializeConfig<'info> {
    /// Ilk authority. Mainnet'te bir Squads multisig PDA'si olmali.
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(init, payer = authority, space = 8 + Config::INIT_SPACE, seeds = [CONFIG_SEED], bump)]
    pub config: Account<'info, Config>,
    #[account(init, payer = authority, space = 8 + RektPool::INIT_SPACE, seeds = [REKT_POOL_SEED], bump)]
    pub rekt_pool: Account<'info, RektPool>,
    #[account(init, payer = authority, space = 8 + HerdVault::INIT_SPACE, seeds = [HERD_VAULT_SEED], bump)]
    pub herd_vault: Account<'info, HerdVault>,
    #[account(init, payer = authority, space = 8 + ProtocolPool::INIT_SPACE, seeds = [PROTOCOL_POOL_SEED], bump)]
    pub protocol_pool: Account<'info, ProtocolPool>,
    #[account(init, payer = authority, space = 8 + DaoPool::INIT_SPACE, seeds = [DAO_POOL_SEED], bump)]
    pub dao_pool: Account<'info, DaoPool>,
    pub system_program: Program<'info, System>,
}

pub fn initialize_config(ctx: Context<InitializeConfig>, args: InitializeConfigArgs) -> Result<()> {
    args.params.validate()?;
    let c = &mut ctx.accounts.config;
    c.authority = ctx.accounts.authority.key();
    c.pending_authority = Pubkey::default();
    c.treasury = args.treasury;
    c.builders_fund = args.builders_fund;
    c.ops_wallet = args.ops_wallet;
    c.rekt_select_bps = args.params.rekt_select_bps;
    c.rekt_payout_cap_bps = args.params.rekt_payout_cap_bps;
    c.rekt_reserve_floor = args.params.rekt_reserve_floor;
    c.min_stampede_lamports = args.params.min_stampede_lamports;
    c.stampede_cap_lamports = args.params.stampede_cap_lamports;
    c.claim_fee_bps = args.params.claim_fee_bps;
    c.damm_config = args.params.damm_config;
    c.breakout_cost_lamports = args.params.breakout_cost_lamports;
    c.min_creator_commit_bps = args.params.min_creator_commit_bps;
    c.paused = args.params.paused;
    c.bump = ctx.bumps.config;

    ctx.accounts.rekt_pool.bump = ctx.bumps.rekt_pool;
    ctx.accounts.herd_vault.bump = ctx.bumps.herd_vault;
    ctx.accounts.protocol_pool.bump = ctx.bumps.protocol_pool;
    ctx.accounts.dao_pool.bump = ctx.bumps.dao_pool;
    Ok(())
}

#[derive(Accounts)]
pub struct UpdateParams<'info> {
    pub authority: Signer<'info>,
    #[account(mut, seeds = [CONFIG_SEED], bump = config.bump, has_one = authority)]
    pub config: Account<'info, Config>,
}

pub fn update_params(ctx: Context<UpdateParams>, params: ParamsArgs) -> Result<()> {
    params.validate()?;
    let c = &mut ctx.accounts.config;
    c.rekt_select_bps = params.rekt_select_bps;
    c.rekt_payout_cap_bps = params.rekt_payout_cap_bps;
    c.rekt_reserve_floor = params.rekt_reserve_floor;
    c.min_stampede_lamports = params.min_stampede_lamports;
    c.stampede_cap_lamports = params.stampede_cap_lamports;
    c.claim_fee_bps = params.claim_fee_bps;
    c.damm_config = params.damm_config;
    c.breakout_cost_lamports = params.breakout_cost_lamports;
    c.min_creator_commit_bps = params.min_creator_commit_bps;
    c.paused = params.paused;
    emit!(ConfigUpdated { authority: ctx.accounts.authority.key() });
    Ok(())
}

#[derive(Accounts)]
pub struct ProposeAuthority<'info> {
    pub authority: Signer<'info>,
    #[account(mut, seeds = [CONFIG_SEED], bump = config.bump, has_one = authority)]
    pub config: Account<'info, Config>,
}

pub fn propose_authority(ctx: Context<ProposeAuthority>, new_authority: Pubkey) -> Result<()> {
    ctx.accounts.config.pending_authority = new_authority;
    emit!(AuthorityProposed {
        current: ctx.accounts.authority.key(),
        pending: new_authority
    });
    Ok(())
}

#[derive(Accounts)]
pub struct AcceptAuthority<'info> {
    pub new_authority: Signer<'info>,
    #[account(mut, seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, Config>,
}

pub fn accept_authority(ctx: Context<AcceptAuthority>) -> Result<()> {
    let c = &mut ctx.accounts.config;
    require!(c.pending_authority != Pubkey::default(), HerdError::NoPendingAuthority);
    require_keys_eq!(
        c.pending_authority,
        ctx.accounts.new_authority.key(),
        HerdError::NotPendingAuthority
    );
    c.authority = c.pending_authority;
    c.pending_authority = Pubkey::default();
    emit!(AuthorityAccepted { new_authority: c.authority });
    Ok(())
}
