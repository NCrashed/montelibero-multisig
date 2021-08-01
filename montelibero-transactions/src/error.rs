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
}

pub type Result<T> = std::result::Result<T, MtlError>;
