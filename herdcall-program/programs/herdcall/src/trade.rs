//! fee_router: bir islemin %1 ucretini §5 tablosuna gore dagitir. Hepsi zincir uzerinde,
//! program tarafindan zorunlu. SOL, launch PDA'sindan (program-owned) dogrudan lamport
//! hareketiyle cikar; sistem hesaplarina kredi yazmak serbesttir.

use anchor_lang::prelude::*;

use crate::errors::HerdError;
use crate::fees::FeeSplit;
use crate::state::{Launch, ProtocolPool, RektPool};

pub fn debit(from: &AccountInfo, amt: u64) -> Result<()> {
    if amt == 0 {
        return Ok(());
    }
    let cur = from.lamports();
    **from.try_borrow_mut_lamports()? = cur.checked_sub(amt).ok_or(HerdError::MathOverflow)?;
    Ok(())
}

pub fn credit(to: &AccountInfo, amt: u64) -> Result<()> {
    if amt == 0 {
        return Ok(());
    }
    let cur = to.lamports();
    **to.try_borrow_mut_lamports()? = cur.checked_add(amt).ok_or(HerdError::MathOverflow)?;
    Ok(())
}

/// Cuzdan alicisina odeme. Sifir bakiyeli bir hesaba rent-muafiyeti altinda kredi yazmak
/// islemi dusurur; bu durumda pay protokol havuzuna yonlendirilir. Doner: yonlendirilen miktar.
fn pay_wallet(from: &AccountInfo, wallet: &AccountInfo, amt: u64) -> Result<u64> {
    if amt == 0 {
        return Ok(0);
    }
    let min = Rent::get()?.minimum_balance(0);
    if wallet.lamports().checked_add(amt).ok_or(HerdError::MathOverflow)? < min {
        return Ok(amt);
    }
    debit(from, amt)?;
    credit(wallet, amt)?;
    Ok(0)
}

#[allow(clippy::too_many_arguments)]
pub fn route_fees<'info>(
    launch: &Account<'info, Launch>,
    creator: &AccountInfo<'info>,
    ui_receiver: &AccountInfo<'info>,
    rekt_pool: &mut Account<'info, RektPool>,
    protocol_pool: &mut Account<'info, ProtocolPool>,
    split: &FeeSplit,
) -> Result<()> {
    let from = launch.to_account_info();

    let redirected_creator = pay_wallet(&from, creator, split.creator)?;
    let redirected_ui = pay_wallet(&from, ui_receiver, split.ui)?;
    let protocol_total = split
        .protocol
        .checked_add(redirected_creator)
        .and_then(|x| x.checked_add(redirected_ui))
        .ok_or(HerdError::MathOverflow)?;

    debit(&from, split.rekt)?;
    credit(&rekt_pool.to_account_info(), split.rekt)?;
    rekt_pool.total_in = rekt_pool.total_in.checked_add(split.rekt).ok_or(HerdError::MathOverflow)?;

    debit(&from, protocol_total)?;
    credit(&protocol_pool.to_account_info(), protocol_total)?;
    protocol_pool.total_in = protocol_pool
        .total_in
        .checked_add(protocol_total)
        .ok_or(HerdError::MathOverflow)?;
    Ok(())
}

/// Launch PDA'si islem sonunda rent-muaf kalmali.
pub fn assert_launch_rent(launch: &Account<Launch>) -> Result<()> {
    let min = Rent::get()?.minimum_balance(8 + Launch::INIT_SPACE);
    require!(launch.to_account_info().lamports() >= min, HerdError::RentViolation);
    Ok(())
}
