//! Lobby → Stampede settlement (STAMPEDE_SPEC §4–§5).
use crate::curve::CurveState;
use crate::{add, div, mul, sub, Error, Result, BPS};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LobbyParams {
    pub v0: u128,
    pub t0: u128,
    pub curve_supply: u128,
    pub fee_bps: u128,
    pub min_deposit: u128,
    pub min_total: u128,
}

impl LobbyParams {
    /// The Lobby can take up to t0 / 2 tokens; the curve must hold at least that.
    pub fn validate(&self) -> Result<()> {
        if self.v0 == 0
            || self.t0 == 0
            || self.fee_bps >= BPS
            || mul(2, self.curve_supply)? < self.t0
        {
            return Err(Error::InvalidParams);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Settlement {
    /// Below the minimum or no token unit to give: everyone gets the full deposit back, no fee.
    pub empty: bool,
    /// Tokens to the Lobby.
    pub x: u128,
    /// Net native that buys the tokens (goes to the curve reserve).
    pub n: u128,
    /// Fee.
    pub f: u128,
    /// Refunded in total (empty Lobby only).
    pub refund: u128,
}

/// N = floor(D·(B−f)/B), F = D − N, X = floor(N·T0 / (V0 + 2N)).
pub fn settle_math(p: &LobbyParams, total: u128) -> Result<Settlement> {
    let empty = Settlement {
        empty: true,
        x: 0,
        n: 0,
        f: 0,
        refund: total,
    };
    if total < p.min_total {
        return Ok(empty);
    }
    let n = div(mul(total, sub(BPS, p.fee_bps)?)?, BPS)?;
    let f = sub(total, n)?;
    let x = div(mul(n, p.t0)?, add(p.v0, mul(2, n)?)?)?;
    if x == 0 {
        return Ok(empty);
    }
    if x > p.curve_supply {
        return Err(Error::InvalidParams);
    }
    Ok(Settlement {
        empty: false,
        x,
        n,
        f,
        refund: 0,
    })
}

/// Settles the Lobby and returns the re-anchored curve: v = V0 + N, t = T0 − X, reserve = N, sold = X.
pub fn settle(p: &LobbyParams, total: u128) -> Result<(Settlement, CurveState)> {
    p.validate()?;
    let s = settle_math(p, total)?;
    if s.empty {
        return Ok((s, CurveState::new(p.v0, p.t0, 0, 0)?));
    }
    if mul(s.x, p.min_deposit)? < total {
        return Err(Error::GranularityViolated);
    }
    let curve = CurveState::new(add(p.v0, s.n)?, sub(p.t0, s.x)?, s.n, s.x)?;
    Ok((s, curve))
}

/// A depositor's tokens: floor(d · X / D). Rounding dust stays in escrow until everyone has claimed.
pub fn claim_tokens(deposit: u128, total: u128, x: u128) -> Result<u128> {
    div(mul(deposit, x)?, total)
}
