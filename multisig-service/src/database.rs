use diesel::{self, prelude::*, result::QueryResult};
use montelibero_transactions::transaction::MtlTransaction;
use rocket::serde::Serialize;
use chrono::NaiveDateTime;

#[database("transactions")]
pub struct TransactionsDb(diesel::SqliteConnection);

mod schema {
    table! {
        transactions {
            id -> Text,
            title -> Text,
            description -> Text,
            body -> Blob,
            created -> Timestamp,
        }
    }

    table! {
        transaction_updates {
            id -> Integer,
            txid -> Text,
            body -> Blob,
            updated -> Timestamp,
        }
    }
}

use self::schema::*;

#[derive(Serialize, Queryable, Insertable, Debug, Clone)]
#[serde(crate = "rocket::serde")]
#[table_name = "transactions"]
pub struct Transaction {
    pub id: String,
    pub title: String,
    pub description: String,
    pub body: Vec<u8>,
    pub created: Option<NaiveDateTime>,
}

#[derive(Serialize, Queryable, Insertable, Debug, Clone)]
#[serde(crate = "rocket::serde")]
#[table_name = "transaction_updates"]
pub struct TransactionUpdate {
    pub id: Option<i32>,
    pub txid: String,
    pub body: Vec<u8>,
    pub updated: Option<NaiveDateTime>,
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
            created: None,
        };
        diesel::insert_into(transactions::table).values(&t).execute(c)
    })
    .await?;
    Ok(())
}
