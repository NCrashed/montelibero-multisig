use super::constants::*;
use super::error::*;
use sha2::{Digest, Sha256};
use substrate_stellar_sdk::{
    horizon::Horizon, Transaction,
    network::PUBLIC_NETWORK,
    types::{
        TransactionSignaturePayload, TransactionSignaturePayloadTaggedTransaction,
        TransactionV1Envelope,
    },
    IntoHash, IntoMuxedAccountId, MuxedAccount, TransactionEnvelope, XdrCodec, AccountId
};

pub struct MtlTransaction(TransactionV1Envelope);

pub fn is_mtl_account(acc_id: &MuxedAccount) -> Result<bool> {
    let mtl_foundation = MTL_FOUNDATION.as_bytes().into_muxed_account_id()?;
    let mtl_issuerer = MTL_ISSUERER.as_bytes().into_muxed_account_id()?;
    Ok(*acc_id == mtl_foundation || *acc_id == mtl_issuerer)
}

pub fn guard_mtl_account(tx: &Transaction) -> Result<()> {
    if !is_mtl_account(&tx.source_account)? {
        return Err(MtlError::WrongSourceAccount);
    }
    Ok(())
}

pub fn guard_fee(tx: &Transaction) -> Result<()> {
    if tx.fee != BASE_FEE {
        return Err(MtlError::NonStandardFee);
    }
    Ok(())
}

fn horizon_mainnet() -> Horizon {
    Horizon::new("https://horizon.stellar.org")
}

/// Parse and validate a raw MTL transaction
pub fn parse_mtl_tx(raw_tx: &str) -> Result<MtlTransaction> {
    let tx_envelope = TransactionEnvelope::from_base64_xdr(raw_tx)?;
    match tx_envelope {
        TransactionEnvelope::EnvelopeTypeTx(envelope) => {
            let tx = &envelope.tx;
            guard_mtl_account(tx)?;
            guard_fee(tx)?;
            Ok(MtlTransaction(envelope))
        }
        TransactionEnvelope::EnvelopeTypeTxV0(_) => {
            return Err(MtlError::DeprecatedTxVersion);
        }
        _ => {
            return Err(MtlError::UnsupportedTx);
        }
    }
}

pub fn validate_mtl_tx(raw_tx: &str) -> Result<MtlTransaction> {
    let tx = parse_mtl_tx(raw_tx)?;
    tx.validate_publish()?;
    Ok(tx)
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


    pub fn fetch_sequence_number(&self) -> Result<i64> {
        Ok(horizon_mainnet().fetch_next_sequence_number(self.source_account()?, FETCH_TIMEOUT)?)
    }
    
    pub fn source_account(&self) -> Result<AccountId> {
        match self.0.tx.source_account {
            MuxedAccount::KeyTypeEd25519(k) => Ok(AccountId::PublicKeyTypeEd25519(k)),
            _ => Err(MtlError::WrongSourceAccount)
        }
    }

    /// Statefull validation if the TX is valid for future publishing
    pub fn validate_publish(&self) -> Result<()> {
        let seq_num = self.fetch_sequence_number()?;
        if seq_num > self.0.tx.seq_num {
            return Err(MtlError::SequenceNumber);
        }
        Ok(())
    }
}
