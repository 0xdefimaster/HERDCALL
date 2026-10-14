//! Ucret bolme ve Rekt-back odeme tavani.
//! Saf fonksiyonlar: kontrata zorunlu kural + birim testleri.
//! Dagilim herkese acik ve program tarafindan zorunlu (HERDCALL.md §5).

use anchor_lang::prelude::*;

use crate::constants::*;
use crate::errors::HerdError;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct FeeSplit {
    pub total: u64,
    pub creator: u64,
    pub ui: u64,
    pub rekt: u64,
    pub protocol: u64,
}

fn bps_of(amount: u64, bps: u64) -> Result<u64> {
    let v = (amount as u128)
        .checked_mul(bps as u128)
        .ok_or(HerdError::MathOverflow)?
        / BPS_DENOM as u128;
    u64::try_from(v).map_err(|_| HerdError::MathOverflow.into())
}

/// Toplam %1 ucret (asagi yuvarlanir).
pub fn fee_total(volume: u64) -> Result<u64> {
    bps_of(volume, FEE_TOTAL_BPS)
}

/// `volume`: islem hacmi (lamport). Toplam %1; yuvarlama artigi protokole gider,
/// boylece creator+ui+rekt+protocol == total her zaman.
pub fn split_fee(volume: u64, bonded: bool) -> Result<FeeSplit> {
    let (c, u, r) = if bonded {
        (BONDED_CREATOR_BPS, BONDED_UI_BPS, BONDED_REKT_BPS)
    } else {
        (UNBONDED_CREATOR_BPS, UNBONDED_UI_BPS, UNBONDED_REKT_BPS)
    };
    let total = bps_of(volume, FEE_TOTAL_BPS)?;
    let creator = bps_of(volume, c)?;
    let ui = bps_of(volume, u)?;
    let rekt = bps_of(volume, r)?;
    let protocol = total
        .checked_sub(creator)
        .and_then(|x| x.checked_sub(ui))
        .and_then(|x| x.checked_sub(rekt))
        .ok_or(HerdError::MathOverflow)?;
    Ok(FeeSplit { total, creator, ui, rekt, protocol })
}

/// Stampede claim ucreti: kabul edilen SOL'un `bps`'i (asagi yuvarlanir).
pub fn claim_fee(accepted: u64, bps: u16) -> Result<u64> {
    bps_of(accepted, bps as u64)
}

/// Kilitli LP pozisyonunun ucreti: %60 creator / %20 protokol / %20 DAO.
/// Yuvarlama artigi protokole gider (toplam her zaman korunur). Doner: (creator, protocol, dao)
pub fn split_lp_fees(amount: u64) -> Result<(u64, u64, u64)> {
    let creator = bps_of(amount, LP_FEE_CREATOR_BPS)?;
    let dao = bps_of(amount, LP_FEE_DAO_BPS)?;
    let protocol = amount
        .checked_sub(creator)
        .and_then(|x| x.checked_sub(dao))
        .ok_or(HerdError::MathOverflow)?;
    Ok((creator, protocol, dao))
}

/// Creator'in token payi: `burn_bps` kadari yakilir, kalani odenir (yuvarlama artigi odemeye gider).
/// Doner: (burn, payout). burn + payout == amount her zaman.
pub fn split_creator_share(amount: u64, burn_bps: u16) -> Result<(u64, u64)> {
    require!(burn_bps as u64 <= BPS_DENOM, HerdError::InvalidFeePrefs);
    let burn = bps_of(amount, burn_bps as u64)?;
    let payout = amount.checked_sub(burn).ok_or(HerdError::MathOverflow)?;
    Ok((burn, payout))
}

/// Rekt-back: bir turda odenebilecek toplam.
/// available = bakiye - rezerv tabani; odeme <= available * cap_bps; talep'ten fazla odenmez.
pub fn rekt_payable(requested: u64, pool_balance: u64, reserve_floor: u64, cap_bps: u16) -> u64 {
    let available = pool_balance.saturating_sub(reserve_floor);
    let cap = ((available as u128) * (cap_bps as u128) / BPS_DENOM as u128) as u64;
    requested.min(cap)
}

/// Talep odenebilirden buyukse herkesin payi orantili kuculur (asagi yuvarlanir).
pub fn prorate(user_request: u64, requested_total: u64, payable: u64) -> u64 {
    if requested_total == 0 || payable >= requested_total {
        return user_request.min(payable);
    }
    ((user_request as u128) * (payable as u128) / (requested_total as u128)) as u64
}

/// Protokol payi (operasyon tabani sonrasi) 50 / 30 / 20 bolunur; artik ops'a gider.
pub fn split_protocol_surplus(surplus: u64) -> Result<(u64, u64, u64)> {
    let vault = bps_of(surplus, VAULT_SHARE_BPS)?;
    let builders = bps_of(surplus, BUILDERS_SHARE_BPS)?;
    let ops = surplus
        .checked_sub(vault)
        .and_then(|x| x.checked_sub(builders))
        .ok_or(HerdError::MathOverflow)?;
    Ok((vault, builders, ops))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bonded_split_matches_doc() {
        let s = split_fee(1_000_000_000, true).unwrap(); // 1 SOL
        assert_eq!(s.total, 10_000_000); // %1
        assert_eq!(s.creator, 3_000_000); // %0.30
        assert_eq!(s.ui, 1_000_000); // %0.10
        assert_eq!(s.rekt, 1_500_000); // %0.15
        assert_eq!(s.protocol, 4_500_000); // %0.45
    }

    #[test]
    fn unbonded_split_matches_doc() {
        let s = split_fee(1_000_000_000, false).unwrap();
        assert_eq!((s.creator, s.ui, s.rekt, s.protocol), (1_000_000, 1_000_000, 3_500_000, 4_500_000));
    }

    #[test]
    fn split_always_sums_to_total() {
        for vol in [0u64, 1, 7, 99, 101, 12_345, 987_654_321, u64::MAX / 10_000] {
            for bonded in [true, false] {
                let s = split_fee(vol, bonded).unwrap();
                assert_eq!(s.creator + s.ui + s.rekt + s.protocol, s.total, "vol={vol}");
            }
        }
    }

    #[test]
    fn rekt_payout_never_exceeds_cap_or_floor() {
        // bakiye 100, rezerv 20, cap %50 => en fazla 40
        assert_eq!(rekt_payable(1_000, 100, 20, 5_000), 40);
        assert_eq!(rekt_payable(10, 100, 20, 5_000), 10);
        // bakiye rezervin altinda => 0
        assert_eq!(rekt_payable(1_000, 15, 20, 5_000), 0);
    }

    #[test]
    fn prorate_never_overpays() {
        let (req, payable) = (1_000u64, 400u64);
        let users = [500u64, 300, 200];
        let paid: u64 = users.iter().map(|u| prorate(*u, req, payable)).sum();
        assert!(paid <= payable);
        assert_eq!(prorate(500, req, payable), 200);
    }

    #[test]
    fn claim_fee_is_bps_of_accepted() {
        assert_eq!(claim_fee(1_000_000_000, 100).unwrap(), 10_000_000); // %1
        assert_eq!(claim_fee(1_000_000_000, 0).unwrap(), 0);
        assert!(claim_fee(12_345, 300).unwrap() <= 12_345);
    }

    #[test]
    fn lp_fee_split_sums_and_matches() {
        assert_eq!(split_lp_fees(1_000).unwrap(), (600, 200, 200));
        for a in [0u64, 1, 3, 7, 101, 1_000_003] {
            let (c, p, d) = split_lp_fees(a).unwrap();
            assert_eq!(c + p + d, a);
        }
    }

    #[test]
    fn creator_share_burn_split() {
        assert_eq!(split_creator_share(1_000, 0).unwrap(), (0, 1_000));
        assert_eq!(split_creator_share(1_000, 2_500).unwrap(), (250, 750));
        assert_eq!(split_creator_share(1_000, 10_000).unwrap(), (1_000, 0));
        assert_eq!(split_creator_share(7, 5_000).unwrap(), (3, 4)); // artik odemeye
        assert!(split_creator_share(1_000, 10_001).is_err());
        for a in [0u64, 1, 3, 101, 1_000_003, u64::MAX] {
            for b in [0u16, 1, 3_333, 9_999, 10_000] {
                let (burn, pay) = split_creator_share(a, b).unwrap();
                assert_eq!(burn as u128 + pay as u128, a as u128);
            }
        }
    }

    #[test]
    fn surplus_split_sums() {
        for s in [0u64, 1, 3, 101, 1_000_003] {
            let (v, b, o) = split_protocol_surplus(s).unwrap();
            assert_eq!(v + b + o, s);
        }
        assert_eq!(split_protocol_surplus(1_000).unwrap(), (500, 300, 200));
    }
}
