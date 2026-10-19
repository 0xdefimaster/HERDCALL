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
