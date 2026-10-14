//! distribute_protocol: ProtocolPool'da biriken SOL'u §7'ye gore boler:
//! %50 HerdVault (PDA, $HERD cikana kadar birikir) / %30 Builders Fund / %20 Ops.
//! Yuvarlama artigi Ops'a gider (`split_protocol_surplus`).
//!
//! Sadece `Config.authority`: dokumandaki "operasyon tabani" (ops'a 50/30/20 oncesi ayrilan
//! taban) zincir uzerinde henuz tanimli degil; ne kadarinin ne zaman dagitilacagini
//! authority (multisig) `amount` ile belirler. Hedef adresler Config'ten zorunlu tutulur.
//! Havuz rent-muafiyetinin altina inemez; her dagitim event'lenir.

use anchor_lang::prelude::*;

use crate::constants::*;
use crate::errors::HerdError;
use crate::events::*;
use crate::fees::split_protocol_surplus;
use crate::state::*;
use crate::trade::{credit, debit};

#[derive(Accounts)]
pub struct DistributeProtocol<'info> {
    pub authority: Signer<'info>,

    #[account(seeds = [CONFIG_SEED], bump = config.bump, has_one = authority)]
    pub config: Account<'info, Config>,

    #[account(mut, seeds = [PROTOCOL_POOL_SEED], bump = protocol_pool.bump)]
    pub protocol_pool: Account<'info, ProtocolPool>,

    #[account(mut, seeds = [HERD_VAULT_SEED], bump = herd_vault.bump)]
    pub herd_vault: Account<'info, HerdVault>,

    /// CHECK: adres Config'ten dogrulanir
    #[account(mut, address = config.builders_fund @ HerdError::InvalidWallet)]
    pub builders_fund: UncheckedAccount<'info>,

    /// CHECK: adres Config'ten dogrulanir
    #[account(mut, address = config.ops_wallet @ HerdError::InvalidWallet)]
    pub ops_wallet: UncheckedAccount<'info>,
}

/// Havuzdan dagitilabilecek en fazla SOL (rent-muaf tutar haric).
pub fn distributable(pool_lamports: u64, rent_min: u64) -> u64 {
    pool_lamports.saturating_sub(rent_min)
}

pub fn distribute_protocol(ctx: Context<DistributeProtocol>, amount: u64) -> Result<()> {
    require!(amount > 0, HerdError::TradeTooSmall);
    let pool_info = ctx.accounts.protocol_pool.to_account_info();
    let rent_min = Rent::get()?.minimum_balance(8 + ProtocolPool::INIT_SPACE);
    require!(
        amount <= distributable(pool_info.lamports(), rent_min),
        HerdError::InsufficientProtocolPool
    );

    let (vault, builders, ops) = split_protocol_surplus(amount)?;
    // Hepsi dogrudan lamport hareketi; bu instruction'da CPI yok.
    debit(&pool_info, amount)?;
    credit(&ctx.accounts.herd_vault.to_account_info(), vault)?;
    credit(&ctx.accounts.builders_fund.to_account_info(), builders)?;
    credit(&ctx.accounts.ops_wallet.to_account_info(), ops)?;

    let hv = &mut ctx.accounts.herd_vault;
    hv.total_in = hv.total_in.checked_add(vault).ok_or(HerdError::MathOverflow)?;
    let pp = &mut ctx.accounts.protocol_pool;
    pp.total_distributed = pp.total_distributed.checked_add(amount).ok_or(HerdError::MathOverflow)?;

    emit!(ProtocolDistributed {
        authority: ctx.accounts.authority.key(),
        amount,
        to_herd_vault: vault,
        to_builders: builders,
        to_ops: ops,
        pool_total_distributed: pp.total_distributed,
    });
    Ok(())
}

#[derive(Accounts)]
pub struct UpdateWallets<'info> {
    pub authority: Signer<'info>,
    #[account(mut, seeds = [CONFIG_SEED], bump = config.bump, has_one = authority)]
    pub config: Account<'info, Config>,
}

/// Sadece authority: §7 hedef cuzdanlarini gunceller (onceden degistirilemiyordu).
pub fn update_wallets(
    ctx: Context<UpdateWallets>,
    treasury: Pubkey,
    builders_fund: Pubkey,
    ops_wallet: Pubkey,
) -> Result<()> {
    crate::instructions::admin::validate_wallets(&treasury, &builders_fund, &ops_wallet)?;
    let c = &mut ctx.accounts.config;
    c.treasury = treasury;
    c.builders_fund = builders_fund;
    c.ops_wallet = ops_wallet;
    emit!(WalletsUpdated {
        authority: ctx.accounts.authority.key(),
        treasury,
        builders_fund,
        ops_wallet,
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distributable_keeps_rent() {
        assert_eq!(distributable(1_000, 300), 700);
        assert_eq!(distributable(200, 300), 0);
    }
}
