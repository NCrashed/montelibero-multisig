use super::constants::*;
use super::error::*;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs::File;
pub use substrate_stellar_sdk::horizon::json_response_types::{AccountResponse, Signer};
pub use substrate_stellar_sdk::types::SignatureHint;
use substrate_stellar_sdk::{IntoAccountId, IntoPublicKey, PublicKey, StellarSdkError};
use thiserror::Error;

pub fn get_account<T: IntoAccountId>(acc_id: T) -> Result<AccountResponse> {
    Ok(horizon_mainnet().fetch_account(acc_id, FETCH_TIMEOUT)?)
}

pub fn get_mtl_foundation() -> Result<AccountResponse> {
    get_account(MTL_FOUNDATION)
}

pub fn get_mtlcity_foundation() -> Result<AccountResponse> {
    get_account(MTLCITY_ISSUERER)
}

pub fn get_btc_treasury() -> Result<AccountResponse> {
    get_account(BTC_TREASURY)
}

pub fn get_mtl_additional() -> Result<AccountResponse> {
    get_account(MTL_ADDITIONAL_ACCOUNT)
}

pub fn get_btc_foundation() -> Result<AccountResponse> {
    get_account(BTC_FOUNDATION)
}

pub fn get_rect_foundation() -> Result<AccountResponse> {
    get_account(MTL_RECT_ACCOUNT)
}

pub fn get_mtl_signers(account: &AccountResponse) -> Result<Vec<(PublicKey, i32)>> {
    let mut keys = Vec::new();
    for sk in account.signers.iter() {
        keys.push((signer_key(&sk)?, sk.weight));
    }
    Ok(keys)
}

fn signer_key(sk: &Signer) -> Result<PublicKey> {
    Ok(sk.key.as_bytes().into_public_key()?)
}

pub fn get_required_weight(account: &AccountResponse) -> u8 {
    account.thresholds.high_threshold
}

#[derive(Deserialize)]
struct Accounts {
    accounts: Vec<AccMapping>,
}

#[derive(Deserialize)]
struct AccMapping {
    pubkey: String,
    telegram: String,
}

#[derive(Debug, Error)]
pub enum MappingError {
    #[error("IO error: {0}")]
    IO(#[from] std::io::Error),
    #[error("Failed to decode mapping: {0}")]
    Decode(#[from] serde_json::Error),
    #[error("Failed to ")]
    PublicKey(#[from] StellarSdkError),
}

pub type UsersMapping = HashMap<PublicKey, String>;

pub fn get_telegram_mapping(file_name: &str) -> std::result::Result<UsersMapping, MappingError> {
    fn decode(value: &str) -> std::result::Result<PublicKey, StellarSdkError> {
        PublicKey::from_encoding(value)
    }

    let mapping: Accounts = serde_json::from_reader(File::open(file_name)?)?;
    let mut result = HashMap::new();
    for acc in mapping.accounts {
        let pk = decode(&acc.pubkey)?;
        result.insert(pk, acc.telegram);
    }
    Ok(result)
}
