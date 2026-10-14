//! Stampede dagitim matematigi (saf fonksiyon, test edilebilir).
//! Tek clearing price: accepted_total / stampede_tokens. Herkes ayni fiyati oder;
//! cap asilirsa fazla SOL orantili iade edilir. Tum bolmeler asagi yuvarlanir:
//! toplam dagitilan token asla stampede_tokens'i, toplam kabul edilen SOL asla cap'i asamaz.

use anchor_lang::prelude::*;

use crate::errors::HerdError;

/// Doner: (kabul edilen lamport, verilecek token)
pub fn claim_amounts(
    deposit: u64,
    total_deposited: u64,
    cap: u64,
    accepted_total: u64,
    stampede_tokens: u64,
) -> Result<(u64, u64)> {
    require!(total_deposited > 0 && accepted_total > 0, HerdError::InvalidState);
    let accepted = if total_deposited <= cap {
        deposit
    } else {
        u64::try_from((deposit as u128) * (cap as u128) / (total_deposited as u128))
            .map_err(|_| HerdError::MathOverflow)?
    };
    let tokens = u64::try_from(
        (accepted as u128) * (stampede_tokens as u128) / (accepted_total as u128),
    )
    .map_err(|_| HerdError::MathOverflow)?;
    Ok((accepted, tokens))
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOKENS: u64 = 100_000_000 * 1_000_000;

    fn run(deposits: &[u64], cap: u64) -> (u64, u64) {
        let total: u64 = deposits.iter().sum();
        let accepted_total = total.min(cap);
        let (mut sa, mut st) = (0u64, 0u64);
        for d in deposits {
            let (a, t) = claim_amounts(*d, total, cap, accepted_total, TOKENS).unwrap();
            assert!(a <= *d, "accepted > deposit");
            sa += a;
            st += t;
        }
        (sa, st)
    }

    #[test]
    fn undersubscribed_everyone_same_price_no_refund() {
        let deps = [1_000_000_000u64, 2_000_000_000, 500_000_000];
        let (sa, st) = run(&deps, 100_000_000_000);
        assert_eq!(sa, deps.iter().sum::<u64>());
        assert!(st <= TOKENS);
        // ayni fiyat: 2x yatiran 2x token alir
        let total: u64 = deps.iter().sum();
        let (_, t1) = claim_amounts(1_000_000_000, total, 100_000_000_000, total, TOKENS).unwrap();
        let (_, t2) = claim_amounts(2_000_000_000, total, 100_000_000_000, total, TOKENS).unwrap();
        assert!(t2 >= 2 * t1 - 1 && t2 <= 2 * t1 + 1);
    }

    #[test]
    fn oversubscribed_prorata_never_exceeds_cap_or_supply() {
        let deps = [30_000_000_000u64, 50_000_000_000, 70_000_000_000, 1_000_000_007];
        let cap = 50_000_000_000;
        let (sa, st) = run(&deps, cap);
        assert!(sa <= cap, "accepted {sa} > cap {cap}");
        assert!(st <= TOKENS);
    }

    #[test]
    fn many_small_wallets_property() {
        // Sybil dahil: 5000 cuzdan, pseudo-random miktarlar
        let mut x: u64 = 88172645463325252;
        let mut deps = Vec::new();
        for _ in 0..5000 {
            x ^= x << 13; x ^= x >> 7; x ^= x << 17;
            deps.push(10_000_000 + x % 3_000_000_000);
        }
        for cap in [100_000_000_000u64, 2_000_000_000_000, u64::MAX / 4] {
            let (sa, st) = run(&deps, cap);
            assert!(sa <= cap);
            assert!(st <= TOKENS);
        }
    }
}
