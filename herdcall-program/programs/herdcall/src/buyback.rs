//! Buyback-burn matematigi (saf fonksiyonlar, birim testli).
//! DAMM v2 `sqrt_price` = sqrt(token_b / token_a) (Q64.64). Token_a = launch token'i, token_b = wSOL.

use anchor_lang::prelude::*;

use crate::constants::*;
use crate::errors::HerdError;

/// `amount_in` (wSOL lamport) ile spot fiyattan alinacak token_a miktari (ucretsiz, etkisiz):
/// out = in * 2^128 / sqrt_price^2. Tasma durumunda hata (islem geri alinir, fon kaybi olmaz).
pub fn spot_tokens_out(amount_in: u64, sqrt_price: u128) -> Result<u128> {
    require!(sqrt_price > 0, HerdError::DammPoolMismatch);
    let x = ((amount_in as u128) << 64) / sqrt_price;
    let y = x.checked_mul(1u128 << 64).ok_or(HerdError::MathOverflow)? / sqrt_price;
    Ok(y)
}

/// Swap'in kabul edecegi minimum cikis: spot beklentinin `BUYBACK_TOLERANCE_BPS` altina inemez.
/// Tolerans havuz ucretini ve fiyat etkisini kapsamali; fazlasini yutan on-run saldirisini sinirlar.
pub fn buyback_min_out(amount_in: u64, sqrt_price: u128) -> Result<u64> {
    let spot = spot_tokens_out(amount_in, sqrt_price)?;
    let min = spot
        .checked_mul((BPS_DENOM - BUYBACK_TOLERANCE_BPS) as u128)
        .ok_or(HerdError::MathOverflow)?
        / BPS_DENOM as u128;
    let min = u64::try_from(min).map_err(|_| HerdError::MathOverflow)?;
    require!(min > 0, HerdError::BuybackSizeOutOfBounds);
    Ok(min)
}

/// Tek islemde harcanabilecek en fazla SOL: havuzun SOL rezervinin `BUYBACK_MAX_POOL_BPS`'i.
pub fn max_buyback(pool_sol_reserve: u64) -> u64 {
    ((pool_sol_reserve as u128) * (BUYBACK_MAX_POOL_BPS as u128) / BPS_DENOM as u128) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spot_matches_price() {
        // fiyat = 1e-4 lamport/ham-token => sqrt = 1e-2 * 2^64
        let sqrt = (1u128 << 64) / 100;
        let out = spot_tokens_out(1_000_000_000, sqrt).unwrap();
        // 1e9 / 1e-4 = 1e13 (yuvarlama payi ile)
        assert!((out as i128 - 10_000_000_000_000i128).abs() < 10_000_000_000);
    }

    #[test]
    fn min_out_is_below_spot_by_tolerance() {
        let sqrt = (1u128 << 64) / 100;
        let spot = spot_tokens_out(1_000_000_000, sqrt).unwrap();
        let min = buyback_min_out(1_000_000_000, sqrt).unwrap() as u128;
        assert_eq!(min, spot * (BPS_DENOM - BUYBACK_TOLERANCE_BPS) as u128 / BPS_DENOM as u128);
    }

    #[test]
    fn zero_price_or_dust_rejected() {
        assert!(spot_tokens_out(1, 0).is_err());
        assert!(buyback_min_out(1, u128::MAX / 2).is_err());
    }

    #[test]
    fn cap_is_one_percent() {
        assert_eq!(max_buyback(1_000_000_000_000), 10_000_000_000);
    }
}
