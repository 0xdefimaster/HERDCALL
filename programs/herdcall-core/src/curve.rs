//! Bonding curve, trading, fees and breakout (CURVE_SPEC §2–§5).
use crate::fees::{fee_on, split_fee, FeeParts, Kind};
use crate::{add, ceil_div, div, mul, sub, Error, Result, BPS};

/// Constant product with virtual reserves. Invariants: reserve = v − V0, t + sold = T0, v·t ≥ k.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CurveState {
    pub v: u128,
    pub t: u128,
    pub k: u128,
    pub reserve: u128,
    pub sold: u128,
}

impl CurveState {
    pub fn new(v: u128, t: u128, reserve: u128, sold: u128) -> Result<Self> {
        if v == 0 || t == 0 {
            return Err(Error::InvalidParams);
        }
        Ok(Self {
            v,
            t,
            k: mul(v, t)?,
            reserve,
            sold,
        })
    }

    /// Native cost (rounded up) to buy `dt` tokens.
    pub fn quote_buy(&self, dt: u128) -> Result<u128> {
        if dt == 0 || dt >= self.t {
            return Err(Error::OutOfRange);
        }
        sub(ceil_div(self.k, sub(self.t, dt)?)?, self.v)
    }

    /// Native returned (rounded down) for selling `dt` tokens.
    pub fn quote_sell(&self, dt: u128) -> Result<u128> {
        if dt == 0 || dt > self.sold {
            return Err(Error::OutOfRange);
        }
        sub(self.v, ceil_div(self.k, add(self.t, dt)?)?)
    }

    pub fn buy(&mut self, dt: u128) -> Result<u128> {
        let cost = self.quote_buy(dt)?;
        self.v = add(self.v, cost)?;
        self.t = sub(self.t, dt)?;
        self.reserve = add(self.reserve, cost)?;
        self.sold = add(self.sold, dt)?;
        Ok(cost)
    }

    pub fn sell(&mut self, dt: u128) -> Result<u128> {
        let out = self.quote_sell(dt)?;
        if out > self.reserve {
            return Err(Error::Insolvent);
        }
        self.v = sub(self.v, out)?;
        self.t = add(self.t, dt)?;
        self.reserve = sub(self.reserve, out)?;
        self.sold = sub(self.sold, dt)?;
        Ok(out)
    }

    /// Native needed to buy back every sold token now (worst-case sell-off).
    pub fn sell_all_payout(&self) -> Result<u128> {
        if self.sold == 0 {
            return Ok(0);
        }
        sub(self.v, ceil_div(self.k, add(self.t, self.sold)?)?)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Curve,
    BreakingOut,
    Migrated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarketParams {
    pub v0: u128,
    pub t0: u128,
    pub total_supply: u128,
    pub curve_supply: u128,
    pub lp_supply: u128,
    pub fee_bps: u128,
    pub migration_fee: u128,
    pub kind: Kind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuyQuote {
    pub tokens: u128,
    pub cost: u128,
    pub fee: u128,
    pub gross: u128,
    /// Exact-in only: native not used (clamp at the boundary, rounding).
    pub refund: u128,
    pub clamped: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SellQuote {
    pub out: u128,
    pub fee: u128,
    pub received: u128,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Migration {
    pub keeper_fee: u128,
    pub lp_native: u128,
    pub lp_tokens: u128,
    pub burned: u128,
    /// Final curve price v / t.
    pub final_spot: (u128, u128),
}

/// A token after its Stampede.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Market {
    pub curve: CurveState,
    pub p: MarketParams,
    pub phase: Phase,
    pub fees: FeeParts,
    pub fee_total: u128,
}

impl Market {
    pub fn new(curve: CurveState, p: MarketParams) -> Result<Self> {
        if add(p.curve_supply, p.lp_supply)? != p.total_supply
            || p.t0 > p.total_supply
            || mul(2, p.curve_supply)? < p.t0
            || p.fee_bps >= BPS
        {
            return Err(Error::InvalidParams);
        }
        if curve.reserve != sub(curve.v, p.v0)?
            || add(curve.t, curve.sold)? != p.t0
            || curve.sold > p.curve_supply
        {
            return Err(Error::InvalidParams);
        }
        let phase = if curve.sold == p.curve_supply {
            Phase::BreakingOut
        } else {
            Phase::Curve
        };
        Ok(Self {
            curve,
            p,
            phase,
            fees: FeeParts::default(),
            fee_total: 0,
        })
    }

    pub fn remaining(&self) -> u128 {
        self.p.curve_supply - self.curve.sold
    }

    /// Routes a fee (curve trades and the Stampede fee) to its parts. Never touches the reserve.
    pub fn route(&mut self, fee: u128, has_interface: bool) -> Result<FeeParts> {
        let parts = split_fee(fee, self.p.kind, has_interface, BPS)?;
        self.fees.accumulate(&parts)?;
        self.fee_total = add(self.fee_total, fee)?;
        Ok(parts)
    }

    fn require_curve(&self) -> Result<()> {
        if self.phase != Phase::Curve {
            return Err(Error::TradingClosed);
        }
        Ok(())
    }

    fn after_buy(&mut self) {
        if self.curve.sold == self.p.curve_supply {
            self.phase = Phase::BreakingOut;
        }
    }

    pub fn quote_buy_exact_out(&self, dt: u128) -> Result<BuyQuote> {
        let tokens = dt.min(self.remaining());
        let cost = self.curve.quote_buy(tokens)?;
        let fee = fee_on(cost, self.p.fee_bps)?;
        Ok(BuyQuote {
            tokens,
            cost,
            fee,
            gross: add(cost, fee)?,
            refund: 0,
            clamped: tokens < dt,
        })
    }

    pub fn buy_exact_out(
        &mut self,
        dt: u128,
        max_gross: u128,
        has_interface: bool,
    ) -> Result<BuyQuote> {
        self.require_curve()?;
        if dt == 0 {
            return Err(Error::ZeroAmount);
        }
        let q = self.quote_buy_exact_out(dt)?;
        if q.gross > max_gross {
            return Err(Error::Slippage);
        }
        self.curve.buy(q.tokens)?;
        self.route(q.fee, has_interface)?;
        self.after_buy();
        Ok(q)
    }

    pub fn quote_buy_exact_in(&self, gross: u128) -> Result<BuyQuote> {
        let c = &self.curve;
        let net = div(mul(gross, BPS)?, add(BPS, self.p.fee_bps)?)?;
        let after = ceil_div(c.k, add(c.v, net)?)?;
        let mut tokens = c.t.saturating_sub(after).min(self.remaining());
        if tokens == 0 {
            return Ok(BuyQuote {
                tokens: 0,
                cost: 0,
                fee: 0,
                gross: 0,
                refund: gross,
                clamped: false,
            });
        }
        let mut cost = c.quote_buy(tokens)?;
        let mut fee = fee_on(cost, self.p.fee_bps)?;
        while add(cost, fee)? > gross && tokens > 0 {
            tokens -= 1;
            cost = if tokens > 0 { c.quote_buy(tokens)? } else { 0 };
            fee = fee_on(cost, self.p.fee_bps)?;
        }
        let spent = add(cost, fee)?;
        Ok(BuyQuote {
            tokens,
            cost,
            fee,
            gross: spent,
            refund: sub(gross, spent)?,
            clamped: tokens == self.remaining(),
        })
    }

    pub fn buy_exact_in(
        &mut self,
        gross: u128,
        min_tokens: u128,
        has_interface: bool,
    ) -> Result<BuyQuote> {
        self.require_curve()?;
        let q = self.quote_buy_exact_in(gross)?;
        if q.tokens == 0 || q.tokens < min_tokens {
            return Err(Error::Slippage);
        }
        self.curve.buy(q.tokens)?;
        self.route(q.fee, has_interface)?;
        self.after_buy();
        Ok(q)
    }

    pub fn quote_sell(&self, dt: u128) -> Result<SellQuote> {
        let out = self.curve.quote_sell(dt)?;
        let fee = fee_on(out, self.p.fee_bps)?;
        Ok(SellQuote {
            out,
            fee,
            received: sub(out, fee)?,
        })
    }

    pub fn sell(&mut self, dt: u128, min_received: u128, has_interface: bool) -> Result<SellQuote> {
        self.require_curve()?;
        let q = self.quote_sell(dt)?;
        if q.received < min_received {
            return Err(Error::Slippage);
        }
        self.curve.sell(dt)?;
        self.route(q.fee, has_interface)?;
        Ok(q)
    }

    /// Permissionless, once. Moves the reserve and the LP tokens to the DEX at the final curve price; LP tokens that
    /// are not needed are burned.
    pub fn breakout(&mut self) -> Result<Migration> {
        if self.phase != Phase::BreakingOut {
            return Err(Error::NotBreakingOut);
        }
        let c = &mut self.curve;
        let keeper_fee = if self.p.migration_fee < c.reserve {
            self.p.migration_fee
        } else {
            0
        };
        let lp_native = sub(c.reserve, keeper_fee)?;
        let at_price = div(mul(lp_native, c.t)?, c.v)?;
        let lp_tokens = at_price.min(self.p.lp_supply);
        let m = Migration {
            keeper_fee,
            lp_native,
            lp_tokens,
            burned: self.p.lp_supply - lp_tokens,
            final_spot: (c.v, c.t),
        };
        c.reserve = 0;
        self.phase = Phase::Migrated;
        Ok(m)
    }
}
