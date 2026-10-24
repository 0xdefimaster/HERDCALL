//! Name and ticker reservation rules (document/HERDCALL.md §12, "Ad ve ticker rezervasyonu").
//!
//! A launch reserves both its name and its ticker when it opens, against a refundable bond. When the Lobby reaches the
//! minimum and the curve opens, they become permanent and the bond goes back. They are released only when the launch
//! is cancelled: its Lobby stayed below the minimum, or nobody settled it in time.

pub const MAX_TICKER_LEN: usize = 10;
/// Bytes of the name as given (same limit as Metaplex token metadata).
pub const MAX_NAME_LEN: usize = 32;

/// Canonical tickers only: 1–10 characters of A–Z and 0–9. Clients upper-case user input before sending it, so
/// "pepe", "Pepe" and "PEPE" all map to one record.
pub fn is_canonical_ticker(t: &[u8]) -> bool {
    (1..=MAX_TICKER_LEN).contains(&t.len()) && t.iter().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
}

/// The form two names are compared in: leading and trailing whitespace removed, Unicode lower case (no locale).
/// `None` if the name is longer than 32 bytes, empty after trimming, or contains a control character.
pub fn canonical_name(name: &str) -> Option<String> {
    let t = name.trim();
    if name.len() > MAX_NAME_LEN || t.is_empty() || t.chars().any(char::is_control) {
        return None;
    }
    Some(t.to_lowercase())
}

/// Where a launch stands, as far as its name and ticker are concerned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchStatus {
    /// Lobby, before its settlement deadline.
    Lobby,
    /// The Lobby stayed below the minimum, or was not settled before its deadline: no curve, refunds only.
    Cancelled,
    /// On the curve: name and ticker are permanent.
    Curve,
    /// Reached breakout (breaking out, migrating or migrated).
    BrokeOut,
}

/// Whether a reservation can be released: only once the launch is cancelled and can never break out.
pub fn releasable(status: LaunchStatus) -> bool {
    status == LaunchStatus::Cancelled
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
    fn canonical_names() {
        let c = |s: &str| canonical_name(s);
        assert_eq!(c("Pepe Coin").as_deref(), Some("pepe coin"));
        assert_eq!(c("  PEPE COIN \t"), c("pepe coin"));
        assert_eq!(c("Çay Coin"), c("çay coin"));
        // Only the ends are trimmed: inner spaces still count.
        assert_ne!(c("pepe  coin"), c("pepe coin"));
        assert_ne!(c("pepecoin"), c("pepe coin"));
        for bad in ["", "   ", "pepe\ncoin", "pepe\u{0}", &"x".repeat(33)] {
            assert_eq!(c(bad), None, "{bad:?}");
        }
        assert!(c(&"x".repeat(32)).is_some());
    }

    #[test]
    fn release_only_when_cancelled() {
        use LaunchStatus::*;
        assert!(releasable(Cancelled));
        for held in [Lobby, Curve, BrokeOut] {
            assert!(!releasable(held), "{held:?}");
        }
    }
}
