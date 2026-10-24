//! Meteora DAMM v2 pool math for the breakout (document/CURVE_SPEC.md §5.4).
//!
//! The pool is a concentrated-liquidity pool over a price range; token A is the launch token, token B is SOL. The
//! amounts DAMM pulls for a liquidity `L` are reproduced exactly (same formulas, rounding up), so the program can pick
//! the largest `L` that fits the tokens and SOL set aside, and open the pool at the curve's final price.
use crate::{Error, Result};
use ruint::aliases::{U256, U512};

/// DAMM v2 price bounds (Q64.64 square roots), as in the program's `constants.rs`.
pub const MIN_SQRT_PRICE: u128 = 4_295_048_016;
pub const MAX_SQRT_PRICE: u128 = 79_226_673_521_066_979_257_578_248_091;

/// Liquidity and the token amounts DAMM takes for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Deposit {
    pub liquidity: u128,
    pub sqrt_price: u128,
    /// Token A (launch token) units.
    pub a: u64,
    /// Token B (SOL) lamports.
    pub b: u64,
}

fn to_u128_256(x: U256) -> Result<u128> {
    u128::try_from(x).map_err(|_| Error::Overflow)
}

fn to_u128_512(x: U512) -> Result<u128> {
    u128::try_from(x).map_err(|_| Error::Overflow)
}

/// floor(√x), Newton's method.
fn isqrt(x: U256) -> U256 {
    if x.is_zero() {
        return x;
    }
    let mut r = U256::from(1u8) << x.bit_len().div_ceil(2);
    loop {
        let next = (r + x / r) >> 1usize;
        if next >= r {
            return r;
        }
        r = next;
    }
}

/// Smallest Q64.64 square root whose price is at least `num / den` (B per A). The pool never opens below the curve.
pub fn sqrt_price_ceil(num: u128, den: u128) -> Result<u128> {
    if num == 0 || den == 0 {
        return Err(Error::InvalidParams);
    }
    let q = (U256::from(num) << 128usize).div_ceil(U256::from(den));
    let mut r = isqrt(q);
    if r * r < q {
        r += U256::from(1u8);
    }
    to_u128_256(r)
}

/// Token A for `liquidity` between `sqrt_price` and `sqrt_max`: ceil(L·(√max − √p) / (√p·√max)).
pub fn amount_a(liquidity: u128, sqrt_price: u128, sqrt_max: u128) -> Result<u128> {
    let num = U512::from(liquidity) * U512::from(sqrt_max - sqrt_price);
    let den = U512::from(sqrt_price) * U512::from(sqrt_max);
    to_u128_512(num.div_ceil(den))
}

/// Token B for `liquidity` between `sqrt_min` and `sqrt_price`: ceil(L·(√p − √min) / 2^128).
pub fn amount_b(liquidity: u128, sqrt_min: u128, sqrt_price: u128) -> Result<u128> {
    let prod = U256::from(liquidity) * U256::from(sqrt_price - sqrt_min);
    to_u128_256(prod.div_ceil(U256::from(1u8) << 128usize))
}

/// The largest liquidity at `sqrt_price` whose amounts fit `max_a` and `max_b`, with those amounts.
pub fn fit(max_a: u64, max_b: u64, sqrt_min: u128, sqrt_price: u128, sqrt_max: u128) -> Result<Deposit> {
    if !(sqrt_min < sqrt_price && sqrt_price < sqrt_max) || sqrt_min < MIN_SQRT_PRICE || sqrt_max > MAX_SQRT_PRICE {
        return Err(Error::InvalidParams);
    }
    if max_a == 0 || max_b == 0 {
        return Err(Error::ZeroAmount);
    }
    // Exact amounts of these L are at most max_a and max_b, so DAMM's rounded-up amounts are too.
    let la = U512::from(max_a) * U512::from(sqrt_price) * U512::from(sqrt_max) / U512::from(sqrt_max - sqrt_price);
    let lb = (U512::from(max_b) << 128usize) / U512::from(sqrt_price - sqrt_min);
    let liquidity = to_u128_512(la.min(lb))?;
    if liquidity == 0 {
        return Err(Error::ZeroAmount);
    }
    let a = amount_a(liquidity, sqrt_price, sqrt_max)?;
    let b = amount_b(liquidity, sqrt_min, sqrt_price)?;
    if a > max_a as u128 || b > max_b as u128 {
        return Err(Error::Overflow);
    }
    Ok(Deposit { liquidity, sqrt_price, a: a as u64, b: b as u64 })
}

// ---------------------------------------------------------------- reinvesting the "back to liquidity" share
//
// CURVE_SPEC §5.7: fees arrive as SOL; part of it buys tokens in the same pool, then both are added to the locked
// position. DAMM v2 has no TWAP or oracle, so the program cannot tell a fair pool price from one moved just before
// the call. A keeper therefore passes a price band from its own off-chain observations; the program rejects a pool
// price outside it. These limits bound what one call puts at risk; they are not a guarantee.

/// Pending SOL below this waits for more fees.
pub const REINVEST_MIN_LAMPORTS: u64 = 10_000_000;
/// SOL reinvested in one call: at most this share of the SOL in the pool's liquidity, and at most half the pool fee
/// (a quarter of the ~2x-fee level where a sandwich starts to pay in sim/lp-reinvest.js).
pub const REINVEST_MAX_POOL_BPS: u128 = 50;
/// Price impact allowed on top of the pool fee when checking the swap's output.
pub const REINVEST_IMPACT_BPS: u128 = 50;
/// The swap may move the pool price up by at most this.
pub const REINVEST_MAX_PRICE_MOVE_BPS: u128 = 60;
/// Tokens bought in one call: at most this share of the tokens in the pool's liquidity.
pub const REINVEST_MAX_TOKEN_BPS: u128 = 50;
/// The keeper's price band may span at most this (max price over min price).
pub const REINVEST_MAX_BAND_BPS: u128 = 200;
/// DAMM fee numerators are over 1e9.
pub const FEE_DENOMINATOR: u128 = 1_000_000_000;

/// SOL (token B) and tokens (token A) held by `liquidity` at `sqrt_price` over the range.
pub fn reserves(liquidity: u128, sqrt_min: u128, sqrt_price: u128, sqrt_max: u128) -> Result<(u128, u128)> {
    Ok((amount_b(liquidity, sqrt_min, sqrt_price)?, amount_a(liquidity, sqrt_price, sqrt_max)?))
}

/// Cap per call in bps of the pool's SOL: min(0.5%, pool fee / 2).
pub fn reinvest_cap_bps(fee_numerator: u64) -> u128 {
    let fee_bps = fee_numerator as u128 * crate::BPS / FEE_DENOMINATOR;
    REINVEST_MAX_POOL_BPS.min(fee_bps / 2)
}

/// How much of `pending` to reinvest now: `None` below the minimum, otherwise capped.
pub fn reinvest_amount(pending: u64, pool_sol: u128, cap_bps: u128) -> Option<u64> {
    if pending < REINVEST_MIN_LAMPORTS {
        return None;
    }
    let cap = pool_sol * cap_bps / crate::BPS;
    let amount = (pending as u128).min(cap) as u64;
    (amount >= REINVEST_MIN_LAMPORTS).then_some(amount)
}

/// Tokens `sol` buys at `sqrt_price` with no fee and no price impact: sol · 2^128 / √p².
pub fn spot_tokens(sol: u64, sqrt_price: u128) -> Result<u128> {
    let p2 = U512::from(sqrt_price) * U512::from(sqrt_price);
    to_u128_512((U512::from(sol) << 128usize) / p2)
}

/// Least tokens the reinvest swap must return: the spot amount less the pool fee and `REINVEST_IMPACT_BPS`.
pub fn reinvest_min_out(sol: u64, sqrt_price: u128, fee_numerator: u64) -> Result<u64> {
    let spot = spot_tokens(sol, sqrt_price)?;
    let after_fee = spot * (FEE_DENOMINATOR - fee_numerator as u128) / FEE_DENOMINATOR;
    u64::try_from(after_fee * (crate::BPS - REINVEST_IMPACT_BPS) / crate::BPS).map_err(|_| Error::Overflow)
}

fn sq(x: u128) -> U512 {
    U512::from(x) * U512::from(x)
}

/// Whether the price after the swap stays within the allowed move: √after² ≤ √before² · (1 + move).
pub fn price_move_ok(sqrt_before: u128, sqrt_after: u128) -> bool {
    sq(sqrt_after) * U512::from(crate::BPS) <= sq(sqrt_before) * U512::from(crate::BPS + REINVEST_MAX_PRICE_MOVE_BPS)
}

/// Whether the keeper's band is narrow enough (max price ≤ min price · 1.02).
pub fn band_ok(sqrt_min: u128, sqrt_max: u128) -> bool {
    sqrt_min <= sqrt_max && sq(sqrt_max) * U512::from(crate::BPS) <= sq(sqrt_min) * U512::from(crate::BPS + REINVEST_MAX_BAND_BPS)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOL: u64 = 1_000_000_000;
    const TOKEN: u64 = 1_000_000;

    #[test]
    fn isqrt_is_floor() {
        for x in [0u128, 1, 2, 3, 4, 15, 16, 17, u64::MAX as u128, u128::MAX] {
            let r = isqrt(U256::from(x));
            assert!(r * r <= U256::from(x) && (r + U256::from(1u8)) * (r + U256::from(1u8)) > U256::from(x), "{x}");
        }
    }

    #[test]
    fn price_is_never_below_the_target() {
        for (num, den) in [(150 * SOL as u128, 200_000_000 * TOKEN as u128), (1, 3), (7, 1), (123_456_789, 987_654_321_000)] {
            let s = sqrt_price_ceil(num, den).unwrap();
            let s2 = U256::from(s) * U256::from(s);
            let target = (U256::from(num) << 128usize).div_ceil(U256::from(den));
            assert!(s2 >= target, "{num}/{den}");
            let below = U256::from(s - 1) * U256::from(s - 1);
            assert!(below < target, "{num}/{den}: smallest such root");
        }
    }

    #[test]
    fn breakout_deposit_fits_and_uses_almost_everything() {
        // Final curve of the example market: v = 150 SOL, t = 200M tokens, LP SOL = reserve − migration fee.
        let (v, t) = (150 * SOL as u128, 200_000_000 * TOKEN as u128);
        let lp_native = 120 * SOL - SOL / 50;
        let lp_tokens = (lp_native as u128 * t / v) as u64;
        let s = sqrt_price_ceil(v, t).unwrap();
        let d = fit(lp_tokens, lp_native, MIN_SQRT_PRICE, s, MAX_SQRT_PRICE).unwrap();
        assert!(d.a <= lp_tokens && d.b <= lp_native);
        // The token side binds; on the SOL side the range's lower bound (√min > 0) leaves about √min/√p ≈ 1e-8 out.
        assert!(lp_tokens - d.a <= 2, "tokens left {}", lp_tokens - d.a);
        assert!(lp_native - d.b <= lp_native / 50_000_000, "lamports left {}", lp_native - d.b);
    }

    #[test]
    fn random_inputs_always_fit() {
        let mut x: u64 = 0x9e3779b97f4a7c15;
        let mut next = || {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x
        };
        for _ in 0..2_000 {
            let a = next() % (1_000_000_000 * TOKEN) + 1;
            let b = next() % (10_000 * SOL) + 1;
            let s = match sqrt_price_ceil(b as u128, a as u128) {
                Ok(s) if s > MIN_SQRT_PRICE && s < MAX_SQRT_PRICE => s,
                _ => continue,
            };
            if let Ok(d) = fit(a, b, MIN_SQRT_PRICE, s, MAX_SQRT_PRICE) {
                assert!(d.a <= a && d.b <= b);
            }
        }
    }

    #[test]
    fn reinvest_limits() {
        let pool_sol = 120 * SOL as u128;
        let one_pct = 10_000_000; // fee numerator of a 1% pool
        assert_eq!(reinvest_cap_bps(one_pct), 50);
        assert_eq!(reinvest_cap_bps(2_500_000), 12, "0.25% pool: half its fee");
        assert_eq!(reinvest_cap_bps(20_000_000), 50, "2% pool: still 0.5%");
        assert_eq!(reinvest_amount(REINVEST_MIN_LAMPORTS - 1, pool_sol, 50), None, "below the minimum");
        assert_eq!(reinvest_amount(SOL / 10, pool_sol, 50), Some(SOL / 10));
        assert_eq!(reinvest_amount(5 * SOL, pool_sol, 50), Some(6 * SOL / 10), "capped at 0.5% of the pool's SOL");
        assert_eq!(reinvest_amount(SOL, (REINVEST_MIN_LAMPORTS as u128) * 100, 50), None, "pool too small for the minimum");
        let s = sqrt_price_ceil(150 * SOL as u128, 200_000_000 * TOKEN as u128).unwrap();
        // 1 SOL at 150 SOL / 200M tokens is about 1.33M tokens; the swap must return 1% fee and 0.5% impact less.
        let spot = spot_tokens(SOL, s).unwrap();
        assert!(spot > 1_333_000 * TOKEN as u128 && spot <= 1_333_334 * TOKEN as u128, "{spot}");
        let min = reinvest_min_out(SOL, s, one_pct).unwrap() as u128;
        assert!(min >= spot * 9850 / 10_000 - 1 && min <= spot * 9851 / 10_000, "{min} vs {spot}");
        assert!(price_move_ok(s, s) && price_move_ok(s, s + s / 400));
        assert!(!price_move_ok(s, s + s / 300));
        assert!(band_ok(s, s + s / 200) && !band_ok(s, s + s / 90) && !band_ok(s + 1, s));
    }

    #[test]
    fn rejects_bad_ranges() {
        assert_eq!(fit(1, 1, MIN_SQRT_PRICE, MIN_SQRT_PRICE, MAX_SQRT_PRICE), Err(Error::InvalidParams));
        assert_eq!(fit(0, 1, MIN_SQRT_PRICE, 1 << 64, MAX_SQRT_PRICE), Err(Error::ZeroAmount));
    }
}
