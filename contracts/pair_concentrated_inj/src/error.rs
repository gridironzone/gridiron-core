use cosmwasm_std::{ConversionOverflowError, OverflowError, StdError};
use thiserror::Error;

use gridiron::asset::MINIMUM_LIQUIDITY_AMOUNT;
use gridiron_circular_buffer::error::BufferError;
use gridiron_pcl_common::error::PclError;

/// This enum describes pair contract errors
#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("{0}")]
    ConversionOverflowError(#[from] ConversionOverflowError),

    #[error("{0}")]
    OverflowError(#[from] OverflowError),

    #[error("{0}")]
    CircularBuffer(#[from] BufferError),

    #[error("{0}")]
    PclError(#[from] PclError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("You need to provide init params")]
    InitParamsNotFound {},

    #[error("Initial provide can not be one-sided")]
    InvalidZeroAmount {},

    #[error("Initial liquidity must be more than {}", MINIMUM_LIQUIDITY_AMOUNT)]
    MinimumLiquidityAmountError {},

    #[error("Failed to parse or process reply message")]
    FailedToParseReply {},

    #[error("Pair is not registered in the factory. Only swap and withdraw are allowed")]
    PairIsNotRegistered {},

    #[error("Invalid number of assets. This pair supports only {0} assets")]
    InvalidNumberOfAssets(usize),

    #[error("The asset {0} does not belong to the pair")]
    InvalidAsset(String),

    #[error("Operation is not supported")]
    NotSupported {},
}
