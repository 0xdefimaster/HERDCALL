//! Fee splits and the creator lock (CURVE_SPEC §4 and §5.8).
use crate::{add, ceil_div, div, mul, sub, Result, BPS};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Bonded,
    Unbonded,
}

/// Shares of the curve fee, in bps of the fee.
struct CurveSplit {
    creator: u128,
    interface: u128,
    rekt_back: u128,
}

const fn curve_split(kind: Kind) -> CurveSplit {
    match kind {
        Kind::Bonded => CurveSplit {
            creator: 5_000,
            interface: 1_000,
            rekt_back: 1_000,
        },
        Kind::Unbonded => CurveSplit {
            creator: 3_000,
            interface: 1_000,
            rekt_back: 1_500,
        },
    }
}

/// Shares of fees claimed from the locked Meteora position, in bps of the claim.
struct LpSplit {
    liquidity: u128,
    creator: u128,
    rekt_back: u128,
}

const fn lp_split(kind: Kind) -> LpSplit {
    match kind {
        Kind::Bonded => LpSplit {
            liquidity: 2_500,
            creator: 5_000,
            rekt_back: 1_000,
        },
        Kind::Unbonded => LpSplit {
            liquidity: 2_500,
            creator: 3_000,
            rekt_back: 1_500,
        },
    }
}

/// Bonded: half of the creator share waits in escrow.
pub const CREATOR_ESCROW_BPS: u128 = 5_000;

/// Fee on a native amount, rounded up.
pub fn fee_on(amount: u128, fee_bps: u128) -> Result<u128> {
    ceil_div(mul(amount, fee_bps)?, BPS)
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FeeParts {
    pub creator_now: u128,
    pub creator_escrow: u128,
    pub interface: u128,
    pub rekt_back: u128,
    pub protocol: u128,
}

impl FeeParts {
    pub fn total(&self) -> Result<u128> {
        add(
            add(
                add(add(self.creator_now, self.creator_escrow)?, self.interface)?,
                self.rekt_back,
            )?,
            self.protocol,
        )
    }

    pub fn accumulate(&mut self, other: &FeeParts) -> Result<()> {
        self.creator_now = add(self.creator_now, other.creator_now)?;
        self.creator_escrow = add(self.creator_escrow, other.creator_escrow)?;
        self.interface = add(self.interface, other.interface)?;
        self.rekt_back = add(self.rekt_back, other.rekt_back)?;
        self.protocol = add(self.protocol, other.protocol)?;
        Ok(())
    }
}

/// Splits a curve fee. Parts round down; the protocol takes the remainder, so the parts always add up to the fee.
/// Without an interface the interface share goes to the protocol. A bonded creator's share is scaled by the lock
/// ratio; the forfeited part goes to the Rekt-back pool.
pub fn split_fee(
    fee: u128,
    kind: Kind,
    has_interface: bool,
    lock_ratio_bps: u128,
) -> Result<FeeParts> {
    let s = curve_split(kind);
    let creator_full = div(mul(fee, s.creator)?, BPS)?;
    let creator = match kind {
        Kind::Bonded => div(mul(creator_full, lock_ratio_bps.min(BPS))?, BPS)?,
        Kind::Unbonded => creator_full,
    };
    let interface = if has_interface {
        div(mul(fee, s.interface)?, BPS)?
    } else {
        0
    };
    let rekt_back_base = div(mul(fee, s.rekt_back)?, BPS)?;
    let creator_escrow = match kind {
        Kind::Bonded => div(mul(creator, CREATOR_ESCROW_BPS)?, BPS)?,
        Kind::Unbonded => 0,
    };
    Ok(FeeParts {
        creator_now: sub(creator, creator_escrow)?,
        creator_escrow,
        interface,
        rekt_back: add(rekt_back_base, sub(creator_full, creator)?)?,
        protocol: sub(sub(sub(fee, creator_full)?, interface)?, rekt_back_base)?,
    })
}

/// The protocol's share of a fee as revenue (HERDCALL.md §7, REKT_MINING.md §7.1): its nominal share, rounded down.
/// What the protocol receives beyond it (`FeeParts::protocol` takes the rounding remainder of the other parts) is a
/// rounding leftover, not revenue. Without an interface the interface share is the protocol's, so it counts.
pub fn curve_protocol_revenue(fee: u128, kind: Kind, has_interface: bool) -> Result<u128> {
    let s = curve_split(kind);
    let mut bps = BPS - s.creator - s.interface - s.rekt_back;
    if !has_interface {
        bps += s.interface;
    }
    div(mul(fee, bps)?, BPS)
}

/// The same for fees claimed from the locked Meteora position.
pub fn lp_protocol_revenue(fee: u128, kind: Kind) -> Result<u128> {
    let s = lp_split(kind);
    div(mul(fee, BPS - s.liquidity - s.creator - s.rekt_back)?, BPS)
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LpFeeParts {
    pub liquidity: u128,
    pub creator: u128,
    pub rekt_back: u128,
    pub protocol: u128,
    pub forfeited: u128,
}

/// Splits a fee claimed from the locked Meteora position. Same remainder and lock-ratio rules as `split_fee`.
pub fn split_lp_fees(fee: u128, kind: Kind, lock_ratio_bps: u128) -> Result<LpFeeParts> {
    let s = lp_split(kind);
    let liquidity = div(mul(fee, s.liquidity)?, BPS)?;
    let creator_full = div(mul(fee, s.creator)?, BPS)?;
    let creator = match kind {
        Kind::Bonded => div(mul(creator_full, lock_ratio_bps.min(BPS))?, BPS)?,
        Kind::Unbonded => creator_full,
    };
    let rekt_back_base = div(mul(fee, s.rekt_back)?, BPS)?;
    let forfeited = sub(creator_full, creator)?;
    Ok(LpFeeParts {
        liquidity,
        creator,
        rekt_back: add(rekt_back_base, forfeited)?,
        protocol: sub(sub(sub(fee, liquidity)?, creator_full)?, rekt_back_base)?,
        forfeited,
    })
}

/// "Hold to earn": the share of a bonded creator's allocation still locked in the vault. Only ever goes down.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CreatorLock {
    pub allocation: u128,
    pub ratio_bps: u128,
}

impl CreatorLock {
    pub fn new(allocation: u128) -> Self {
        Self {
            allocation,
            ratio_bps: if allocation > 0 { BPS } else { 0 },
        }
    }

    /// Records the vault balance; returns the lock ratio to use from now on.
    pub fn update(&mut self, locked: u128) -> Result<u128> {
        let r = if self.allocation == 0 {
            0
        } else if locked >= self.allocation {
            BPS
        } else {
            div(mul(locked, BPS)?, self.allocation)?
        };
        if r < self.ratio_bps {
            self.ratio_bps = r;
        }
        Ok(self.ratio_bps)
    }
}

#[cfg(test)]
mod revenue_tests {
    use super::*;

    #[test]
    fn revenue_is_the_nominal_share_and_never_more_than_received() {
        let mut x: u64 = 0x2545f4914f6cdd1d;
        for _ in 0..20_000 {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            let fee = (x % 10_000_000_000) as u128;
            for kind in [Kind::Bonded, Kind::Unbonded] {
                for iface in [true, false] {
                    for lock in [BPS, 6_543, 0] {
                        let p = split_fee(fee, kind, iface, lock).unwrap();
                        let r = curve_protocol_revenue(fee, kind, iface).unwrap();
                        assert!(r <= p.protocol && p.protocol - r <= 3, "curve {fee} {kind:?} {iface}: {} vs {r}", p.protocol);
                    }
                }
                let p = split_lp_fees(fee, kind, BPS).unwrap();
                let r = lp_protocol_revenue(fee, kind).unwrap();
                assert!(r <= p.protocol && p.protocol - r <= 3, "lp {fee} {kind:?}");
            }
        }
        // Nominal shares: 0.30 / 0.45 of the 1% fee (plus the interface share without an interface); 15% / 30% of LP fees.
        assert_eq!(curve_protocol_revenue(10_000, Kind::Bonded, true).unwrap(), 3_000);
        assert_eq!(curve_protocol_revenue(10_000, Kind::Unbonded, false).unwrap(), 5_500);
        assert_eq!(lp_protocol_revenue(10_000, Kind::Bonded).unwrap(), 1_500);
        assert_eq!(lp_protocol_revenue(10_000, Kind::Unbonded).unwrap(), 3_000);
    }
}
