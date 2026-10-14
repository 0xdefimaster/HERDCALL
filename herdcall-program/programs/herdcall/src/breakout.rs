//! Breakout planlama matematigi (saf fonksiyonlar). Henuz bir instruction'a BAGLI DEGIL:
//! DEX CPI'si (Meteora DAMM v2) IDL'den dogrulanmadan yarim bir `breakout` yayinlamak
//! (SOL tasinip mint yetkisi iptal edilip havuz acilmamasi) fonlari kilitleyebilir.
//!
//! Kararlar (Pump.fun ile ayni cekirdek, bizim eklerimizle):
//! * Curve'de biriken SOLUN TAMAMI + %20 LP token'i havuza girer (Pump.fun: 80/20, tum SOL havuza).
//! * Havuz fiyati curve sonundan yuksek acilir (~1.9x); curve kapali oldugu icin arbitraj yok,
//!   holder lehine ve havuz daha derin olur. Fiyat esitlemek icin SOL'u geri tutmayiz.
//! * LP kalici kilitlenir; mint yetkisi ayni islemde iptal edilir.
//! * Havuz acma maliyeti (rent + DEX ucreti) havuz SOL'undan duser (Pump.fun'in 0.015 SOL
//!   mig. ucreti gibi), protokolden degil.
//! * Supurulecek SOL lamport bakiyesinden DEGIL durumdan hesaplanir: launch PDA'sinda
//!   claim edilmemis cap-fazlasi iadeleri de durur ve onlara dokunulamaz.

use anchor_lang::prelude::*;

use crate::constants::*;
use crate::errors::HerdError;

// Arz paylari tam 100% etmeli: Stampede + Curve + LP.
const _: () = assert!(STAMPEDE_SUPPLY_BPS + CURVE_SUPPLY_BPS + LP_RESERVE_BPS == BPS_DENOM);

/// Breakout'ta mint edilecek LP token miktari (toplam arzin %20'si).
pub fn lp_tokens() -> Result<u64> {
    u64::try_from((TOTAL_SUPPLY as u128) * (LP_RESERVE_BPS as u128) / BPS_DENOM as u128)
        .map_err(|_| HerdError::MathOverflow.into())
}

/// Havuza girecek SOL = accepted_total + (vs - vs0) - dex_cost.
/// (curve solvency degismezi: gercek_SOL = accepted_total + (vs - vs0))
pub fn pool_sol(
    accepted_total: u64,
    virtual_sol: u64,
    initial_virtual_sol: u64,
    dex_cost: u64,
) -> Result<u64> {
    let curve_net = virtual_sol
        .checked_sub(initial_virtual_sol)
        .ok_or(HerdError::MathOverflow)?;
    accepted_total
        .checked_add(curve_net)
        .and_then(|x| x.checked_sub(dex_cost))
        .ok_or(HerdError::InsufficientCurveReserve.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::curve;

    #[test]
    fn lp_is_twenty_percent() {
        assert_eq!(lp_tokens().unwrap(), 200_000_000 * 1_000_000);
    }

    #[test]
    fn cost_cannot_exceed_reserve() {
        assert!(pool_sol(5, 10, 10, 6).is_err());
        assert_eq!(pool_sol(5, 15, 10, 3).unwrap(), 7);
    }

    /// Curve'u sonuna kadar doldur: havuz SOL'u = 15 * accepted_total (v0 parametreleriyle).
    #[test]
    fn full_curve_pool_sol_is_15x_accepted() {
        let accepted: u64 = 5_000_000_000; // 5 SOL
        let stampede = ((TOTAL_SUPPLY as u128) * (STAMPEDE_SUPPLY_BPS as u128) / BPS_DENOM as u128) as u64;
        let curve_tokens = ((TOTAL_SUPPLY as u128) * (CURVE_SUPPLY_BPS as u128) / BPS_DENOM as u128) as u64;
        let (vs0, vt0, floor) = curve::init_reserves(accepted, stampede, curve_tokens).unwrap();
        let (mut vs, mut vt) = (vs0, vt0);
        loop {
            let q = curve::quote_buy(vs, vt, floor, 10_000_000_000).unwrap();
            vs = q.new_vs;
            vt = q.new_vt;
            if q.filled {
                break;
            }
        }
        assert_eq!(pool_sol(accepted, vs, vs0, 0).unwrap(), 15 * accepted);
    }
}
