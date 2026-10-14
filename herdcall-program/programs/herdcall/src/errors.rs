use anchor_lang::prelude::*;

#[error_code]
pub enum HerdError {
    #[msg("Protocol is paused")]
    Paused,
    #[msg("Invalid lobby duration (allowed: 15 / 30 / 60 seconds)")]
    InvalidLobbyDuration,
    #[msg("Bonded creators are not supported in v0")]
    BondedNotYetSupported,
    #[msg("Lobby is not open")]
    LobbyNotOpen,
    #[msg("Lobby has not ended yet")]
    LobbyNotEnded,
    #[msg("Launch is not in the expected state")]
    InvalidState,
    #[msg("Deposit below minimum")]
    DepositTooSmall,
    #[msg("Arithmetic overflow")]
    MathOverflow,
    #[msg("Parameter outside the allowed bounds")]
    ParamOutOfBounds,
    #[msg("Signer is not the pending authority")]
    NotPendingAuthority,
    #[msg("Trade below minimum size")]
    TradeTooSmall,
    #[msg("Slippage: output below the minimum requested")]
    SlippageExceeded,
    #[msg("Curve cannot cover this payout")]
    InsufficientCurveReserve,
    #[msg("Launch account would fall below rent exemption")]
    RentViolation,
    #[msg("No pending authority transfer")]
    NoPendingAuthority,
    #[msg("DAMM config does not match the configured one")]
    InvalidDammConfig,
    #[msg("DAMM pool received less than the planned liquidity (bad liquidity/sqrt_price)")]
    BreakoutPoolMismatch,
    #[msg("DAO pool balance is insufficient (rent exemption must be kept)")]
    InsufficientDaoPool,
    #[msg("Position has no claimable fees")]
    NothingToClaim,
    #[msg("Invalid creator fee preferences")]
    InvalidFeePrefs,
    #[msg("Creator already forfeited rewards; committing again has no effect")]
    CreatorForfeited,
    #[msg("Commit amount exceeds what is committed")]
    InsufficientCommit,
    #[msg("Burn reserve is smaller than the requested buyback")]
    InsufficientBurnReserve,
    #[msg("Buyback size outside allowed bounds")]
    BuybackSizeOutOfBounds,
    #[msg("DAMM pool account does not match this launch")]
    DammPoolMismatch,
    #[msg("Token pool balance is insufficient")]
    InsufficientPoolTokens,
    #[msg("Burn ratio can only be increased, never lowered")]
    BurnBpsCannotDecrease,
}
