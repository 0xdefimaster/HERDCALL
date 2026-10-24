use anchor_lang::prelude::*;

#[error_code]
pub enum HerdError {
    #[msg("Arithmetic overflow")]
    MathOverflow,
    #[msg("Amount out of range for the curve")]
    OutOfRange,
    #[msg("Curve would become insolvent")]
    Insolvent,
    #[msg("Slippage limit exceeded")]
    Slippage,
    #[msg("Trading is closed for this phase")]
    TradingClosed,
    #[msg("Token is not breaking out")]
    NotBreakingOut,
    #[msg("Amount must be positive")]
    ZeroAmount,
    #[msg("Invalid parameters")]
    InvalidParams,
    #[msg("Lobby allocation would leave a deposit without tokens")]
    GranularityViolated,
    #[msg("Lobby is not open")]
    LobbyClosed,
    #[msg("Lobby is still open")]
    LobbyOpen,
    #[msg("Below the minimum deposit")]
    BelowMinimum,
    #[msg("Lobby cap reached")]
    LobbyCap,
    #[msg("Wallet cap reached")]
    WalletCap,
    #[msg("Settlement window has passed")]
    SettleDeadlinePassed,
    #[msg("Not settled")]
    NotSettled,
    #[msg("Settlement window has not passed yet")]
    CannotCancelYet,
    #[msg("Already withdrawn")]
    AlreadyWithdrawn,
    #[msg("Wrong account")]
    WrongAccount,
    #[msg("Vault would drop below rent exemption")]
    VaultUnderfunded,
    #[msg("Ticker must be 1-10 characters of A-Z and 0-9")]
    InvalidTicker,
    #[msg("Name must be 1-32 bytes without control characters")]
    InvalidName,
    #[msg("Name hash does not match the name")]
    NameHashMismatch,
    #[msg("Name and ticker are held until the launch is cancelled or breaks out")]
    NameNotReleasable,
}

pub fn core_err(e: herdcall_core::Error) -> Error {
    use herdcall_core::Error as E;
    match e {
        E::Overflow => error!(HerdError::MathOverflow),
        E::OutOfRange => error!(HerdError::OutOfRange),
        E::Insolvent => error!(HerdError::Insolvent),
        E::Slippage => error!(HerdError::Slippage),
        E::TradingClosed => error!(HerdError::TradingClosed),
        E::NotBreakingOut => error!(HerdError::NotBreakingOut),
        E::ZeroAmount => error!(HerdError::ZeroAmount),
        E::InvalidParams => error!(HerdError::InvalidParams),
        E::GranularityViolated => error!(HerdError::GranularityViolated),
    }
}
