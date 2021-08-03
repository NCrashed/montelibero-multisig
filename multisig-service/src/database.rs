use diesel::{self, prelude::*, result::QueryResult};
use montelibero_transactions::transaction::MtlTransaction;
use montelibero_transactions::error::MtlError;
use rocket::serde::Serialize;
use chrono::NaiveDateTime;
use thiserror::Error;

#[database("transactions")]
pub struct TransactionsDb(diesel::SqliteConnection);

use super::schema::*;
use super::schema::transactions::dsl::{transactions as all_transactions};
use super::schema::transaction_updates::dsl::{transaction_updates as all_transaction_updates};

#[derive(Serialize, Queryable, Insertable, Debug, Clone)]
#[serde(crate = "rocket::serde")]
#[table_name = "transactions"]
pub struct Transaction {
    pub id: String,
    pub title: String,
    pub description: String,
    pub body: Vec<u8>,
    pub created: NaiveDateTime,
}

#[derive(Serialize, Queryable, Insertable, Debug, Clone)]
#[serde(crate = "rocket::serde")]
#[table_name = "transaction_updates"]
pub struct TransactionUpdate {
    pub id: i32,
    pub txid: String,
    pub body: Vec<u8>,
    pub updated: NaiveDateTime,
}

#[derive(Serialize, Queryable, Insertable, Debug, Clone)]
#[serde(crate = "rocket::serde")]
#[table_name = "transaction_updates"]
pub struct TransactionUpdateCreate {
    pub txid: String,
    pub body: Vec<u8>,
    pub updated: NaiveDateTime,
}

pub async fn store_transaction(
    conn: &TransactionsDb,
    tx: MtlTransaction,
    title: String,
    description: String
) -> QueryResult<()> {
    conn.run(move |c| {
        let t = Transaction {
            id: hex::encode(tx.txid()),
            title,
            description,
            body: tx.into_bytes(),
            created: chrono::Utc::now().naive_utc(),
        };
        diesel::insert_into(transactions::table).values(&t).execute(c)
    })
    .await?;
    Ok(())
}

pub async fn store_transaction_update(
    conn: &TransactionsDb,
    tx: MtlTransaction,
) -> QueryResult<()> {
    conn.run(move |c| {
        let t = TransactionUpdateCreate {
            txid: hex::encode(tx.txid()),
            body: tx.into_bytes(),
            updated: chrono::Utc::now().naive_utc(),
        };
        diesel::insert_into(transaction_updates::table).values(&t).execute(c)
    })
    .await?;
    Ok(())
}

pub struct MtlTxMeta {
    pub title: String, 
    pub description: String,
    pub history: Vec<(MtlTransaction, NaiveDateTime)>,
}

impl MtlTxMeta {
    pub fn current(&self) -> (MtlTransaction, NaiveDateTime) {
        self.history[0].clone()
    }
}

#[derive(Debug, Error)]
pub enum TxLoadError {
    #[error("Failed to load tx due Database error: {0}")]
    Diesel(#[from] diesel::result::Error),
    #[error("Failed to load transaction: {0}")]
    Transaction(#[from] MtlError),
}

pub async fn get_transaction(
    conn: &TransactionsDb,
    txid: Vec<u8>,
) -> Result<MtlTxMeta, TxLoadError> {
    conn.run(move |c| {
        let tid = hex::encode(txid);
        let tx_created = all_transactions.find(tid.clone()).get_result::<Transaction>(c)?;
        let updates = all_transaction_updates
            .order(transaction_updates::updated.desc())
            .filter(transaction_updates::txid.eq(tid))
            .load::<TransactionUpdate>(c)?;

        let mut history: Vec<(MtlTransaction, NaiveDateTime)> = Vec::new(); 
        for u in updates.iter() {
            history.push((MtlTransaction::from_bytes(&u.body)?, u.updated));
        }
        history.push((MtlTransaction::from_bytes(&tx_created.body)?, tx_created.created));

        Ok(MtlTxMeta {
            title: tx_created.title,
            description: tx_created.description,
            history: history,
        })
    }).await
}