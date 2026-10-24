//! Ticker reservation rules (document/HERDCALL.md §12, "İsim rezervasyonu").
//!
//! A ticker is held while its launch is active, becomes permanent at breakout and can be released by anyone once the
//! launch is cancelled or its hold time passes without a breakout.

pub const MAX_TICKER_LEN: usize = 10;

/// Canonical tickers only: 1–10 characters of A–Z and 0–9. Clients upper-case user input before sending it, so
/// "pepe", "Pepe" and "PEPE" all map to one record.
pub fn is_canonical_ticker(t: &[u8]) -> bool {
    (1..=MAX_TICKER_LEN).contains(&t.len()) && t.iter().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
}

/// Where a launch stands, as far as its ticker is concerned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchStatus {
    /// Lobby, before its settlement deadline.
    Lobby,
    /// The Lobby was never settled and its deadline has passed (cancelled, whether or not anyone has refunded yet).
    LobbyExpired,
    /// Trading on the curve.
    Curve,
    /// Reached breakout (breaking out, migrating or migrated).
    BrokeOut,
}

/// Whether a non-permanent reservation can be released at `now`.
pub fn releasable(status: LaunchStatus, now: i64, expires_at: i64) -> bool {
    match status {
        LaunchStatus::LobbyExpired => true,
        LaunchStatus::Curve => now >= expires_at,
        LaunchStatus::Lobby | LaunchStatus::BrokeOut => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_tickers() {
        for ok in ["A", "PEPE", "HERD2", "0123456789"] {
            assert!(is_canonical_ticker(ok.as_bytes()), "{ok}");
        }
        for bad in ["", "pepe", "Pepe", "PE PE", "PEPE!", "ÇAY", "ABCDEFGHIJK", "$PEPE"] {
            assert!(!is_canonical_ticker(bad.as_bytes()), "{bad}");
        }
    }

    #[test]
    fn release_rules() {
        use LaunchStatus::*;
        assert!(!releasable(Lobby, 100, 50));
        assert!(releasable(LobbyExpired, 0, 50));
        assert!(!releasable(Curve, 49, 50));
        assert!(releasable(Curve, 50, 50));
        assert!(!releasable(BrokeOut, i64::MAX, 50));
    }
}
