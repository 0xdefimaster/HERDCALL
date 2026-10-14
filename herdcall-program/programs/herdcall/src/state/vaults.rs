use anchor_lang::prelude::*;

/// Rekt-back havuzu. SOL, bu PDA'nin kendi lamport bakiyesinde durur;
/// sayaclar seffaflik panelini besler.
#[account]
#[derive(InitSpace)]
pub struct RektPool {
    pub total_in: u64,
    pub total_paid: u64,
    pub bump: u8,
}

/// Token oncesi birikim kasasi (Herd Vault). $HERD cikana kadar birikir.
#[account]
#[derive(InitSpace)]
pub struct HerdVault {
    pub total_in: u64,
    pub bump: u8,
}

/// DAO payi (kilitli LP ucretinin %20'si). ProtocolPool'a KARISMAZ; 50/30/20 dagitimi bozulmaz.
/// SOL bu PDA'nin lamport bakiyesinde birikir; sadece `Config.authority` (multisig, sonra DAO)
/// `withdraw_dao` ile cikarabilir. Bakiye + sayaclar herkese acik.
#[account]
#[derive(InitSpace)]
pub struct DaoPool {
    pub total_in: u64,
    pub total_withdrawn: u64,
    pub bump: u8,
}

/// Protokol geliri (%0.45) once burada birikir; `distribute_protocol` (sonraki adim)
/// operasyon tabanini ayirip kalani 50/30/20 dagitir. Bakiye + sayac herkese acik.
#[account]
#[derive(InitSpace)]
pub struct ProtocolPool {
    pub total_in: u64,
    pub total_distributed: u64,
    pub bump: u8,
}
