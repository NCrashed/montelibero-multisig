use thiserror::Error;

#[derive(Error, Debug)]
pub enum MtlError {
    #[error("Failed to decode transaction: {0}")]
    Decode(#[from] substrate_stellar_sdk::DecodeError),
    #[error("Failed with sdk error: {0}")]
    Sdk(#[from] substrate_stellar_sdk::StellarSdkError),
    #[error("Source account is not MTL related")]
    WrongSourceAccount,
    #[error("Used version 0 transaction")]
    DeprecatedTxVersion,
    #[error("Unsupported transaction type")]
    UnsupportedTx,
    #[error("Unsupported signer key in foundation")]
    UnsupportedSignerKey,
    #[error("Transaction has non standard fee")]
    NonStandardFee,
    #[error("Transaction has overdue sequence number")]
    SequenceNumber,
    #[error("Transaction has too little time window for signing")]
    TooLittleTimeBound, 
    #[error("Failed to request from Horizon server: {0}")]
    FetchError(#[from] substrate_stellar_sdk::horizon::FetchError),
}

pub type Result<T> = std::result::Result<T, MtlError>;
