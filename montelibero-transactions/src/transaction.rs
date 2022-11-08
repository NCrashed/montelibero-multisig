use super::account::*;
use super::constants::*;
use super::error::*;
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};
use substrate_stellar_sdk::{
    network::PUBLIC_NETWORK,
    types::{
        SignatureHint, TimePoint, TransactionSignaturePayload,
        TransactionSignaturePayloadTaggedTransaction, TransactionV1Envelope,
    },
    AccountId, IntoHash, IntoMuxedAccountId, MuxedAccount, PublicKey, Transaction,
    TransactionEnvelope, XdrCodec,
};

#[derive(Debug, Clone)]
pub struct MtlTransaction(TransactionV1Envelope);

pub fn is_mtl_account(mtl_account: &AccountResponse, acc_id: &MuxedAccount) -> Result<bool> {
    let mtl_foundation = MTL_FOUNDATION.as_bytes().into_muxed_account_id()?;
    let mtl_issuerer = MTL_ISSUERER.as_bytes().into_muxed_account_id()?;
    let mtlcity_issuerer = MTLCITY_ISSUERER.as_bytes().into_muxed_account_id()?;
    let btc_treasury = BTC_TREASURY.as_bytes().into_muxed_account_id()?;
    let mtl_additional = MTL_ADDITIONAL_ACCOUNT.as_bytes().into_muxed_account_id()?;
    let btc_foundation = BTC_FOUNDATION.as_bytes().into_muxed_account_id()?;
    let rect_foundation = MTL_RECT_ACCOUNT.as_bytes().into_muxed_account_id()?;
    let multisig_storage = MTL_MULTISIG_STORAGE_ACCOUNT.as_bytes().into_muxed_account_id()?;
    let signers: Vec<PublicKey> = get_mtl_signers(&mtl_account)?
        .iter()
        .map(|s| s.0.clone())
        .collect();
    Ok(*acc_id == mtl_foundation
        || *acc_id == mtl_issuerer
        || *acc_id == mtlcity_issuerer
        || *acc_id == btc_treasury
        || *acc_id == mtl_additional
        || *acc_id == btc_foundation
        || *acc_id == rect_foundation
        || *acc_id == multisig_storage
        || signers
            .iter()
            .any(|s| account_pubkey(acc_id).unwrap() == *s))
}

fn account_pubkey(account: &MuxedAccount) -> Result<PublicKey> {
    match account {
        MuxedAccount::KeyTypeEd25519(k) => Ok(AccountId::PublicKeyTypeEd25519(*k)),
        _ => Err(MtlError::WrongSourceAccount),
    }
}

pub fn guard_mtl_account(tx: &Transaction) -> Result<()> {
    let acc = &tx.source_account;
    let pubkey = get_account(account_pubkey(acc)?)?;
    if !is_mtl_account(&pubkey, acc)? {
        return Err(MtlError::WrongSourceAccount);
    }
    Ok(())
}

pub fn guard_fee(tx: &Transaction) -> Result<()> {
    if tx.fee < MIN_FEE || tx.fee > MAX_FEE {
        return Err(MtlError::NonStandardFee);
    }
    Ok(())
}

/// Parse and validate a raw MTL transaction
pub fn parse_mtl_tx<T: AsRef<[u8]>>(raw_tx: &T) -> Result<MtlTransaction> {
    let tx_envelope = TransactionEnvelope::from_base64_xdr(raw_tx)?;
    match tx_envelope {
        TransactionEnvelope::EnvelopeTypeTx(envelope) => {
            let tx = &envelope.tx;
            guard_mtl_account(tx)?;
            guard_fee(tx)?;
            Ok(MtlTransaction(envelope))
        }
        TransactionEnvelope::EnvelopeTypeTxV0(_) => Err(MtlError::DeprecatedTxVersion),
        _ => Err(MtlError::UnsupportedTx),
    }
}

pub fn validate_mtl_tx<T: AsRef<[u8]>>(raw_tx: &T) -> Result<MtlTransaction> {
    let tx = parse_mtl_tx(raw_tx)?;
    tx.validate_create()?;
    Ok(tx)
}

fn get_current_time() -> TimePoint {
    let start = SystemTime::now();
    let since_the_epoch = start
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards");
    since_the_epoch.as_secs()
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

    pub fn fetch_source_account(&self) -> Result<AccountResponse> {
        get_account(self.source_account()?)
    }

    pub fn fetch_sequence_number(&self) -> Result<i64> {
        Ok(horizon_mainnet().fetch_next_sequence_number(self.source_account()?, FETCH_TIMEOUT)?)
    }

    pub fn source_account(&self) -> Result<AccountId> {
        match self.0.tx.source_account {
            MuxedAccount::KeyTypeEd25519(k) => Ok(AccountId::PublicKeyTypeEd25519(k)),
            _ => Err(MtlError::WrongSourceAccount),
        }
    }

    pub fn has_time_window(&self) -> bool {
        match &self.0.tx.time_bounds {
            None => true,
            Some(bounds) => {
                let current = get_current_time();
                if bounds.min_time == 0 && bounds.max_time > 0 {
                    if bounds.max_time < current + SIGNING_TIME_WINDOW {
                        return false;
                    }
                } else if bounds.max_time > 0 {
                    let adjust_min = u64::max(current, bounds.min_time);
                    if bounds.max_time < adjust_min
                        || adjust_min + SIGNING_TIME_WINDOW > bounds.max_time
                    {
                        return false;
                    }
                }
                true
            }
        }
    }

    pub fn guard_time_window(&self) -> Result<()> {
        if !self.has_time_window() {
            return Err(MtlError::TooLittleTimeBound);
        }
        Ok(())
    }

    /// Statefull validation if the TX is valid for future publishing
    pub fn validate_create(&self) -> Result<()> {
        let seq_num = self.fetch_sequence_number()?;
        if seq_num > self.0.tx.seq_num {
            return Err(MtlError::SequenceNumber);
        }
        self.guard_time_window()?;
        let account = self.fetch_source_account()?;
        let signers: Vec<PublicKey> = get_mtl_signers(&account)?
            .iter()
            .map(|s| s.0.clone())
            .collect();
        TransactionEnvelope::EnvelopeTypeTx(self.0.clone())
            .check_signatures(&PUBLIC_NETWORK, &signers)?;
        self.guard_excess_signatures(&account)?;
        Ok(())
    }

    /// Check that the transaction has just enough number of signatures to sign
    pub fn guard_excess_signatures(&self, account: &AccountResponse) -> Result<()> {
        let required = get_required_weight(account) as i32;
        let mut accum: i32 = 0;
        for (_, w) in self.get_signed_keys(account)? {
            if accum >= required && accum + w >= required {
                return Err(MtlError::SignaturesExcess);
            }
            accum += w;
        }
        Ok(())
    }

    pub fn into_bytes(&self) -> Vec<u8> {
        TransactionEnvelope::EnvelopeTypeTx(self.0.clone()).to_xdr()
    }

    pub fn into_encoding(&self) -> String {
        std::str::from_utf8(&TransactionEnvelope::EnvelopeTypeTx(self.0.clone()).to_base64_xdr())
            .unwrap()
            .to_owned()
    }

    pub fn from_bytes<T: AsRef<[u8]>>(bytes: &T) -> Result<Self> {
        let tx_envelope = TransactionEnvelope::from_xdr(bytes)?;
        match tx_envelope {
            TransactionEnvelope::EnvelopeTypeTx(envelope) => Ok(MtlTransaction(envelope)),
            TransactionEnvelope::EnvelopeTypeTxV0(_) => Err(MtlError::DeprecatedTxVersion),
            _ => Err(MtlError::UnsupportedTx),
        }
    }

    pub fn signatures(&self) -> Vec<SignatureHint> {
        self.0.signatures.get_vec().iter().map(|s| s.hint).collect()
    }

    pub fn get_signed_keys(&self, account: &AccountResponse) -> Result<Vec<(PublicKey, i32)>> {
        let signers = get_mtl_signers(account)?;
        let signs: Vec<SignatureHint> =
            self.0.signatures.get_vec().iter().map(|s| s.hint).collect();

        Ok(signers
            .iter()
            .filter(|(pk, _)| signs.contains(&pk.get_signature_hint()))
            .cloned()
            .collect())
    }

    pub fn is_published(&self) -> Result<bool> {
        let res = horizon_mainnet().query_transaction(&self.txid(), FETCH_TIMEOUT)?;
        Ok(res.successful)
    }

    pub fn validate_update(&self, update: &Self) -> Result<()> {
        if self.txid() != update.txid() {
            return Err(MtlError::UpdateContentChanged);
        }
        let update_signs = update.signatures();
        for s in self.signatures() {
            if !update_signs.contains(&s) {
                return Err(MtlError::UpdateSignatureRemoved);
            }
        }
        let account = update.fetch_source_account()?;
        update.guard_excess_signatures(&account)?;
        Ok(())
    }
}
