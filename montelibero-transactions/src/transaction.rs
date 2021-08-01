use super::constants::*;
use super::error::*;
use sha2::{Digest, Sha256};
use substrate_stellar_sdk::{
    horizon::Horizon,
    network::PUBLIC_NETWORK,
    types::{
        TransactionSignaturePayload, TransactionSignaturePayloadTaggedTransaction,
        TransactionV1Envelope,
    },
    IntoHash, IntoMuxedAccountId, MuxedAccount, TransactionEnvelope, XdrCodec,
};

pub struct MtlTransaction(TransactionV1Envelope);

pub fn is_mtl_account(acc_id: &MuxedAccount) -> Result<bool> {
    let mtl_foundation = MTL_FOUNDATION.as_bytes().into_muxed_account_id()?;
    let mtl_issuerer = MTL_ISSUERER.as_bytes().into_muxed_account_id()?;
    Ok(*acc_id == mtl_foundation || *acc_id == mtl_issuerer)
}

/// Parse and validate a raw MTL transaction
pub fn parse_mtl_tx(raw_tx: &str) -> Result<MtlTransaction> {
    let tx_envelope = TransactionEnvelope::from_base64_xdr(raw_tx)?;
    match tx_envelope {
        TransactionEnvelope::EnvelopeTypeTx(tx) => {
            if !is_mtl_account(&tx.tx.source_account)? {
                return Err(MtlError::WrongSourceAccount);
            }
            Ok(MtlTransaction(tx))
        }
        TransactionEnvelope::EnvelopeTypeTxV0(_) => {
            return Err(MtlError::DeprecatedTxVersion);
        }
        _ => {
            return Err(MtlError::UnsupportedTx);
        }
    }
}

impl MtlTransaction {
    pub fn txid(&self) -> Vec<u8> {
        let payload = TransactionSignaturePayload {
            network_id: PUBLIC_NETWORK.get_id().into_hash().unwrap(),
            tagged_transaction: TransactionSignaturePayloadTaggedTransaction::EnvelopeTypeTx(
                self.0.tx.clone(),
            ),
        };
        let mut hasher = Sha256::new();
        hasher.update(payload.to_xdr());
        hasher.finalize().to_vec()
    }
}
