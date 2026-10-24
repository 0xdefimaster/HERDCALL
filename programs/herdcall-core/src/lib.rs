//! Core math of the HerdCall program (document/STAMPEDE_SPEC.md, document/CURVE_SPEC.md).
//!
//! Integer only, checked arithmetic, no Solana dependencies: the on-chain program and off-chain tools share this
//! one implementation. Tested against vectors exported from the JS reference model (`npm run vectors`).
#![forbid(unsafe_code)]

pub mod curve;
pub mod fees;
pub mod names;
pub mod stampede;

/// Basis-point denominator.
pub const BPS: u128 = 10_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Overflow,
    /// Amount outside what the curve can take (zero, or at least the whole virtual reserve).
    OutOfRange,
    /// A sell would pay out more than the real reserve. Unreachable when the invariants hold.
    Insolvent,
    Slippage,
    TradingClosed,
    NotBreakingOut,
    ZeroAmount,
    InvalidParams,
    /// The Lobby allocation would leave a minimum deposit without a token unit.
    GranularityViolated,
}

pub type Result<T> = core::result::Result<T, Error>;

pub(crate) fn add(a: u128, b: u128) -> Result<u128> {
    a.checked_add(b).ok_or(Error::Overflow)
}

pub(crate) fn sub(a: u128, b: u128) -> Result<u128> {
    a.checked_sub(b).ok_or(Error::Overflow)
}

pub(crate) fn mul(a: u128, b: u128) -> Result<u128> {
    a.checked_mul(b).ok_or(Error::Overflow)
}

pub(crate) fn div(a: u128, b: u128) -> Result<u128> {
    a.checked_div(b).ok_or(Error::Overflow)
}

/// Ceiling division.
pub fn ceil_div(a: u128, b: u128) -> Result<u128> {
    let q = div(a, b)?;
    Ok(if a.is_multiple_of(b) { q } else { q + 1 })
}
