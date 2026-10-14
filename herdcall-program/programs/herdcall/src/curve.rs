//! Bonding curve matematigi: sabit carpim (x*y=k), sanal rezervlerle. Saf fonksiyonlar.
//!
//! * Baslangic fiyati = Stampede clearing price'i: vs0 / vt0 = accepted_total / stampede_tokens.
//! * Tum yuvarlamalar kullanicinin aleyhine / protokolun lehine (alista yukari, satista asagi),
//!   bu yuzden al-sat dongusu asla kar ettirmez ve curve her zaman cozucudur.
//! * Curve solvency degismezi:  gercek_SOL = accepted_total + (vs - vs0)  >= 0.

use anchor_lang::prelude::*;

use crate::constants::*;
use crate::errors::HerdError;
use crate::fees::fee_total;

fn mul(a: u64, b: u64) -> u128 {
    (a as u128) * (b as u128)
}

fn ceil_div(a: u128, b: u128) -> Result<u128> {
    require!(b > 0, HerdError::MathOverflow);
    Ok((a + b - 1) / b)
}

fn to_u64(x: u128) -> Result<u64> {
    u64::try_from(x).map_err(|_| HerdError::MathOverflow.into())
}

/// Doner: (vs0, vt0, vt_floor). vt_floor = vt0 - curve_tokens: bu degere inince curve dolmustur.
pub fn init_reserves(accepted_total: u64, stampede_tokens: u64, curve_tokens: u64) -> Result<(u64, u64, u64)> {
    require!(accepted_total > 0 && stampede_tokens > 0 && curve_tokens > 0, HerdError::InvalidState);
    let vt0 = curve_tokens.checked_mul(CURVE_VT_MULT).ok_or(HerdError::MathOverflow)?;
    let vs0 = to_u64(mul(accepted_total, vt0) / stampede_tokens as u128)?;
    let floor = vt0.checked_sub(curve_tokens).ok_or(HerdError::MathOverflow)?;
    Ok((vs0, vt0, floor))
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct BuyQuote {
    /// Gercekten tahsil edilecek brut SOL (curve dolarsa girilenden az olabilir)
    pub gross: u64,
    pub fee: u64,
    pub net: u64,
    pub tokens_out: u64,
    pub new_vs: u64,
    pub new_vt: u64,
    pub filled: bool,
}

pub fn quote_buy(vs: u64, vt: u64, floor: u64, gross_in: u64) -> Result<BuyQuote> {
    require!(vt > floor, HerdError::InvalidState);
    let k = mul(vs, vt);
    let fee = fee_total(gross_in)?;
    let net = gross_in.checked_sub(fee).ok_or(HerdError::MathOverflow)?;
    let new_vs = vs.checked_add(net).ok_or(HerdError::MathOverflow)?;
    let new_vt = to_u64(ceil_div(k, new_vs as u128)?)?; // yukari: kullanicinin aleyhine

    if new_vt >= floor {
        let tokens_out = vt.checked_sub(new_vt).ok_or(HerdError::MathOverflow)?;
        return Ok(BuyQuote { gross: gross_in, fee, net, tokens_out, new_vs, new_vt, filled: new_vt == floor });
    }

    // Curve'de kalan tokenlardan fazlasi istendi: tam dolacak kadarini al.
    let need_vs = to_u64(ceil_div(k, floor as u128)?)?;
    let net_needed = need_vs.checked_sub(vs).ok_or(HerdError::MathOverflow)?;
    let gross = to_u64(ceil_div(
        mul(net_needed, BPS_DENOM),
        (BPS_DENOM - FEE_TOTAL_BPS) as u128,
    )?)?;
    require!(gross <= gross_in, HerdError::MathOverflow);
    let fee = fee_total(gross)?;
    let net = gross - fee; // >= net_needed (ucret asagi yuvarlandigi icin)
    require!(net >= net_needed, HerdError::MathOverflow);
    Ok(BuyQuote {
        gross,
        fee,
        net,
        tokens_out: vt - floor,
        new_vs: vs.checked_add(net).ok_or(HerdError::MathOverflow)?,
        new_vt: floor,
        filled: true,
    })
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct SellQuote {
    pub gross_out: u64,
    pub new_vs: u64,
    pub new_vt: u64,
}

pub fn quote_sell(vs: u64, vt: u64, tokens_in: u64) -> Result<SellQuote> {
    require!(tokens_in > 0, HerdError::InvalidState);
    let k = mul(vs, vt);
    let new_vt = vt.checked_add(tokens_in).ok_or(HerdError::MathOverflow)?;
    let new_vs = to_u64(ceil_div(k, new_vt as u128)?)?; // yukari: odenecek SOL asagi yuvarlanir
    let gross_out = vs.checked_sub(new_vs).ok_or(HerdError::MathOverflow)?;
    Ok(SellQuote { gross_out, new_vs, new_vt })
}

/// Curve'in gercek SOL rezervi (rent haric, iade edilmemis Stampede fazlasi haric).
pub fn real_sol(accepted_total: u64, vs: u64, vs0: u64) -> i128 {
    accepted_total as i128 + vs as i128 - vs0 as i128
}

#[cfg(test)]
mod tests {
    use super::*;

    const S: u64 = 100_000_000 * 1_000_000; // stampede tokens
    const C: u64 = 700_000_000 * 1_000_000; // curve tokens
    const ACC: u64 = 5_000_000_000; // 5 SOL kabul edilen

    fn start() -> (u64, u64, u64) {
        init_reserves(ACC, S, C).unwrap()
    }

    #[test]
    fn start_price_equals_clearing_price() {
        let (vs0, vt0, floor) = start();
        assert_eq!(floor, C);
        // vs0/vt0 == ACC/S  <=>  vs0*S == ACC*vt0 (yuvarlama payi 1 birim)
        let lhs = vs0 as u128 * S as u128;
        let rhs = ACC as u128 * vt0 as u128;
        assert!(lhs <= rhs && rhs - lhs < S as u128);
    }

    #[test]
    fn buy_then_sell_back_never_profits() {
        let (vs0, vt0, floor) = start();
        for gross in [MIN_TRADE_LAMPORTS, 50_000_000, 1_000_000_000, 4_000_000_000] {
            let b = quote_buy(vs0, vt0, floor, gross).unwrap();
            let s = quote_sell(b.new_vs, b.new_vt, b.tokens_out).unwrap();
            assert!(s.gross_out <= b.net, "kar: out {} > net {}", s.gross_out, b.net);
        }
    }

    #[test]
    fn oversized_buy_fills_curve_and_never_overshoots() {
        let (vs0, vt0, floor) = start();
        let b = quote_buy(vs0, vt0, floor, 1_000_000_000_000).unwrap();
        assert!(b.filled);
        assert_eq!(b.new_vt, floor);
        assert_eq!(b.tokens_out, C);
        assert!(b.gross < 1_000_000_000_000);
        // dolu curve'de tekrar alim yok
        assert!(quote_buy(b.new_vs, b.new_vt, floor, 10_000_000).is_err());
    }

    #[test]
    fn stampede_holders_can_all_sell_and_curve_stays_solvent() {
        let (vs0, vt0, _floor) = start();
        // Stampede alicilari S token'in hepsini satiyor
        let s = quote_sell(vs0, vt0, S).unwrap();
        assert!(real_sol(ACC, s.new_vs, vs0) >= 0, "insolvent");
        assert!(s.gross_out <= ACC);
    }

    #[test]
    fn random_trades_keep_invariants() {
        let (vs0, vt0, floor) = start();
        let (mut vs, mut vt) = (vs0, vt0);
        let mut alive: u64 = S; // dolasimdaki token: stampede + curve alimlari
        let mut x: u64 = 0x9E3779B97F4A7C15;
        let mut rnd = || { x ^= x << 13; x ^= x >> 7; x ^= x << 17; x };
        for _ in 0..20_000 {
            if vt > floor && (rnd() % 100 < 55 || alive == 0) {
                let gross = MIN_TRADE_LAMPORTS + rnd() % 3_000_000_000;
                if let Ok(b) = quote_buy(vs, vt, floor, gross) {
                    vs = b.new_vs; vt = b.new_vt; alive += b.tokens_out;
                }
            } else if alive > 0 {
                let amt = 1 + rnd() % alive.min(50_000_000_000_000);
                let q = quote_sell(vs, vt, amt).unwrap();
                vs = q.new_vs; vt = q.new_vt; alive -= amt;
            }
            assert!(real_sol(ACC, vs, vs0) >= 0, "insolvent");
            assert!(vt >= floor);
            assert!(alive <= S + C);
        }
    }
}
