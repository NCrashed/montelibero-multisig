#[macro_use]
extern crate rocket;
#[macro_use]
extern crate rocket_sync_db_pools;
#[macro_use]
extern crate diesel;
#[macro_use]
extern crate diesel_migrations;

pub mod database;
pub mod schema;

use database::*;

use rocket_dyn_templates::{context, Template};
use thiserror::Error;
use rocket::fairing::AdHoc;
use rocket::form::Form;
use rocket::fs::{relative, FileServer};
use rocket::response::Redirect;
use rocket::{Build, Rocket, State};
use rocket::http::{CookieJar, Cookie};
use rocket::serde::{Serialize, json::Json};
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use chrono::{NaiveDateTime, Utc, Duration};

use montelibero_transactions::account::*;
use montelibero_transactions::transaction::validate_mtl_tx;
use montelibero_transactions::error::MtlError;

struct Cache {
    blocks: Arc<Mutex<HashMap<Vec<u8>, NaiveDateTime>>>,
}

impl Cache {
    fn new() -> Self {
        Cache {
            blocks: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn is_blocked(&self, tid: &[u8]) -> bool {
        if let Some(t) = self.blocks.lock().unwrap().get(tid) {
            *t > Utc::now().naive_utc()
        } else {
            false
        }
    }

    fn block(&self, tid: &[u8], delay: Duration) {
        self.blocks.lock().unwrap().insert(tid.to_owned(), Utc::now().naive_utc() + delay);
    }

    fn unblock(&self, tid: &[u8]) {
        self.blocks.lock().unwrap().remove(tid);
    }
}

#[get("/")]
pub fn index() -> Redirect {
    Redirect::to(uri!("/", create_transaction()))
}

#[derive(Debug, Error)]
pub enum ViewError {
    #[error("Transaction id is not hex encoded")]
    InvalidTxid(#[from] hex::FromHexError),
    #[error("List of transactions is not impelemented")]
    NotImplemented,
    #[error("{0}")]
    Mtl(#[from] MtlError),
    #[error("{0}")]
    DatabaseError(#[from] TxLoadError),
}

#[derive(Serialize)]
pub struct ViewSigner {
    pub key: String, 
    pub weight: i32,
    pub signed: bool, 
}

impl ViewSigner {
    pub fn collect(account: &AccountResponse, signs: &[SignatureHint]) -> Result<Vec<Self>, MtlError> {
        let mut res = Vec::new();
        for s in get_mtl_signers(account)? {
            res.push(ViewSigner {
                key: std::str::from_utf8(&s.0.to_encoding()).unwrap().to_owned(),
                weight: s.1, 
                signed: signs.contains(&s.0.get_signature_hint()),
            })
        }
        Ok(res)
    }
}

#[get("/view?<tid>")]
async fn view_transaction(conn: TransactionsDb, cache: &State<Cache>, cookies: &CookieJar<'_>, tid: Option<String>) -> Template {

    fn render_error(err_message: &str) -> Template {
        Template::render(
            "view-tx",
            &context! {
                title: "Montelibero multisignature service",
                parent: "base",
                menu_view_tx: true,
                is_error: true,
                error_msg: err_message
            },
        )
    }

    async fn view(conn: TransactionsDb, cache: &State<Cache>, cookies: &CookieJar<'_>, mtid: Option<String>) -> Result<Template, ViewError> {
        let txid = match mtid {
            None => return Err(ViewError::NotImplemented),
            Some(v) => hex::decode(&v)?,
        };
        let tx = get_transaction(&conn, txid.clone()).await?;
        let curr_tx = tx.current().0;
        match curr_tx.is_published() {
            Ok(true) => {
                let account = curr_tx.fetch_source_account()?;
                let signs = curr_tx.get_signed_keys(&account)?;
                let hints: Vec<SignatureHint> = signs.iter().map(|s| s.0.get_signature_hint()).collect();
                let tx_collected: i32 = signs.iter().map(|s| s.1).sum();
                Ok(Template::render(
                    "view-tx",
                    &context! {
                        title: "Montelibero multisignature service",
                        parent: "base",
                        menu_view_tx: true,
                        is_error: false, 
                        tx_id: hex::encode(txid.clone()),  
                        tx_title: tx.title, 
                        tx_description: tx.description,
                        tx_last: curr_tx.into_encoding(),
                        tx_required: get_required_weight(&account),
                        tx_collected,
                        tx_signers: ViewSigner::collect(&account, &hints)?,
                        tx_published: true,
                        tx_updates: tx.history.len(),
                    },
                ))
            }
            _ => {
                match curr_tx.validate_create() {
                    Ok(_) => {
                        let is_blocked = cache.is_blocked(&txid);
                        let mut is_blocker = cookies.get("is_blocker").is_some();
                        if !is_blocked && is_blocker {
                            cookies.remove(Cookie::new("is_blocker", ""));
                            is_blocker = false;
                        }
                        let account = curr_tx.fetch_source_account()?;
                        let signs = curr_tx.get_signed_keys(&account)?;
                        let hints: Vec<SignatureHint> = signs.iter().map(|s| s.0.get_signature_hint()).collect();
                        let tx_collected: i32 = signs.iter().map(|s| s.1).sum();
                        Ok(Template::render(
                            "view-tx",
                            &context! {
                                title: "Montelibero multisignature service",
                                parent: "base",
                                menu_view_tx: true,
                                is_error: false, 
                                tx_id: hex::encode(txid.clone()),  
                                tx_title: tx.title, 
                                tx_description: tx.description,
                                tx_last: curr_tx.into_encoding(),
                                tx_required: get_required_weight(&account),
                                tx_collected,
                                is_blocked,
                                is_blocker,
                                tx_signers: ViewSigner::collect(&account, &hints)?,
                                tx_published: false,
                                tx_updates: tx.history.len(),
                            },
                        ))
                    }
                    Err(e) => {
                        let account = curr_tx.fetch_source_account()?;
                        let signs = curr_tx.get_signed_keys(&account)?;
                        let hints: Vec<SignatureHint> = signs.iter().map(|s| s.0.get_signature_hint()).collect();
                        let tx_collected: i32 = signs.iter().map(|s| s.1).sum();
                        Ok(Template::render(
                            "view-tx",
                            &context! {
                                title: "Montelibero multisignature service",
                                parent: "base",
                                menu_view_tx: true,
                                is_error: false, 
                                tx_id: hex::encode(txid.clone()),  
                                tx_title: tx.title, 
                                tx_description: tx.description,
                                tx_last: curr_tx.into_encoding(),
                                tx_required: get_required_weight(&account),
                                tx_collected,
                                tx_signers: ViewSigner::collect(&account, &hints)?,
                                tx_invalid: true,
                                tx_published: false,
                                tx_invalid_msg: format!("{}", e),
                                tx_updates: tx.history.len(),
                            },
                        ))
                    }
                }
            }
        }
    
    }
    
    match view(conn, cache, cookies, tid).await {
        Ok(t) => t,
        Err(e) => render_error(&format!("{}", e)),
    }
}

#[derive(Debug, Error)]
pub enum BlockError {
    #[error("Transaction id is not hex encoded")]
    InvalidTxid(#[from] hex::FromHexError),
    #[error("Transaction is already blocked")]
    AlreadyBlocked,
    #[error("{0}")]
    DatabaseError(#[from] TxLoadError),
}

#[derive(Serialize)]
struct BlockResp {
    error: Option<String>,
}

#[post("/block/<txid>")]
async fn block_transaction(conn: TransactionsDb, cache: &State<Cache>, cookies: &CookieJar<'_>, txid: String) -> Json<BlockResp> {
    async fn block(conn: TransactionsDb, cache: &State<Cache>, tid: &str) -> Result<(), BlockError> {
        let txid = hex::decode(tid)?;
        let _ = get_transaction(&conn, txid.clone()).await?;
        if cache.is_blocked(&txid) {
            return Err(BlockError::AlreadyBlocked);
        }
        cache.block(&txid, Duration::minutes(5));
        Ok(())
    }
    
    match block(conn, cache, &txid).await {
        Ok(_) => {
            cookies.add(Cookie::new("is_blocker", ""));
            Json(BlockResp {
                error: None,
            })
        },
        Err(e) => Json(BlockResp {
            error: Some(format!("{}", e)),
        }),
    }
}

#[derive(Debug, Error)]
pub enum UnBlockError {
    #[error("Transaction id is not hex encoded")]
    InvalidTxid(#[from] hex::FromHexError),
    #[error("Transaction is not blocked")]
    NotBlocked,
    #[error("{0}")]
    DatabaseError(#[from] TxLoadError),
}

#[derive(Serialize)]
struct UnBlockResp {
    error: Option<String>,
}

#[post("/unblock/<txid>")]
async fn unblock_transaction(conn: TransactionsDb, cache: &State<Cache>, cookies: &CookieJar<'_>, txid: String) -> Json<UnBlockResp> {
    async fn unblock(conn: TransactionsDb, cache: &State<Cache>, tid: &str) -> Result<(), UnBlockError> {
        let txid = hex::decode(tid)?;
        let _ = get_transaction(&conn, txid.clone()).await?;
        if !cache.is_blocked(&txid) {
            return Err(UnBlockError::NotBlocked);
        }
        cache.unblock(&txid);
        Ok(())
    }
    
    match unblock(conn, cache, &txid).await {
        Ok(_) => {
            cookies.remove(Cookie::new("is_blocker", ""));
            Json(UnBlockResp {
                error: None,
            })
        },
        Err(e) => Json(UnBlockResp {
            error: Some(format!("{}", e)),
        }),
    }
}

#[get("/create")]
fn create_transaction() -> Template {
    Template::render(
        "create-tx",
        &context! {
            title: "Montelibero multisignature service",
            parent: "base",
            menu_create_tx: true,
        },
    )
}

#[derive(FromForm)]
struct CreateTx {
    tx_title: String,
    tx_description: String,
    tx_body: String,
}

#[post("/create", data = "<tx>")]
async fn post_transaction(conn: TransactionsDb, tx: Form<CreateTx>) -> Template {
    fn render_error(err_message: &str) -> Template {
        Template::render(
            "create-tx-response",
            &context! {
                title: "Montelibero multisignature service",
                parent: "base",
                menu_create_tx: true,
                is_error: true,
                error_msg: err_message
            },
        )
    }

    if tx.tx_body.len() == 0 {
        render_error("Transaction body is empty")
    } else if tx.tx_title.len() == 0 {
        render_error("Transaction title is empty")
    } else {
        match validate_mtl_tx(&tx.tx_body) {
            Ok(mtx) => {
                match store_transaction(&conn, mtx.clone(), tx.tx_title.clone(), tx.tx_description.clone()).await {
                    Ok(_) => Template::render(
                        "create-tx-response",
                        &context! {
                            title: "Montelibero multisignature service",
                            parent: "base",
                            menu_create_tx: true,
                            txid: hex::encode(mtx.txid()),
                            is_error: false,
                        },
                    ),
                    Err(e) => render_error(&format!("{}", e)),
                }
                
            }
            Err(e) => render_error(&format!("{}", e)),
        }
    }
}

#[derive(FromForm)]
struct UpdateTx {
    tx_body: String,
}

#[post("/update", data = "<tx>")]
async fn update_transaction(conn: TransactionsDb, cache: &State<Cache>, tx: Form<UpdateTx>) -> Result<Redirect, Template> {
    fn render_error(err_message: &str) -> Template {
        Template::render(
            "create-tx-response",
            &context! {
                title: "Montelibero multisignature service",
                parent: "base",
                menu_create_tx: true,
                is_error: true,
                error_msg: err_message
            },
        )
    }

    match validate_mtl_tx(&tx.tx_body) {
        Ok(mtx) => {
            match store_transaction_update(&conn, mtx.clone()).await {
                Ok(_) => {
                    let txid = mtx.txid();
                    cache.unblock(&txid);
                    let url = uri!(view_transaction(tid = Some(hex::encode(txid))));
                    Ok(Redirect::to(url))
                },
                Err(e) => Err(render_error(&format!("{}", e))),
            }
            
        }
        Err(e) => Err(render_error(&format!("{}", e))),
    }
}

#[derive(Debug, Error)]
pub enum CheckError {
    #[error("Failed to decode transaction ID")]
    TransactionId(#[from] hex::FromHexError),
    #[error("Failed to load transaction: {0}")]
    TransactionLoad(#[from] database::TxLoadError),
    #[error("{0}")]
    MtlError(#[from] MtlError),
}

#[derive(Serialize)]
pub struct CheckResult {
    pub updated: bool,
    pub is_error: bool,
    pub error_msg: Option<String>,
}

#[get("/check/update/<txid>?<updates>&<block>&<published>")]
async fn check_update_transaction(conn: TransactionsDb, cache: &State<Cache>, txid: String, updates: u32, block: bool, published: bool) -> Json<CheckResult> {
    
    async fn check(conn: TransactionsDb, cache: &State<Cache>, txid: String, updates: u32, block: bool, published: bool) -> Result<bool, CheckError> {
        let txid = hex::decode(&txid)?;
        let meta = get_transaction(&conn, txid.clone()).await?;
        let is_blocked = cache.is_blocked(&txid);
        let is_published = meta.current().0.is_published().unwrap_or(false);
        Ok(block != is_blocked || updates != meta.history.len() as u32 || published != is_published)
    }

    match check(conn, cache, txid, updates, block, published).await {
        Err(e) => Json(CheckResult {
            updated: false,
            is_error: true, 
            error_msg: Some(format!("{}", e)),
        }),
        Ok(updated) => Json(CheckResult {
            updated,
            is_error: false, 
            error_msg: None,
        }),
    }
}

async fn run_migrations(rocket: Rocket<Build>) -> Rocket<Build> {
    embed_migrations!();

    let conn = TransactionsDb::get_one(&rocket)
        .await
        .expect("database connection");
    conn.run(|c| embedded_migrations::run(c))
        .await
        .expect("can run migrations");

    rocket
}

#[launch]
fn rocket() -> _ {
    rocket::build()
        .mount("/", FileServer::from(relative!("static")))
        .mount(
            "/",
            routes![
                index,
                create_transaction,
                post_transaction,
                view_transaction,
                block_transaction,
                unblock_transaction,
                update_transaction,
                check_update_transaction,
            ],
        )
        .manage(Cache::new())
        .attach(Template::fairing())
        .attach(TransactionsDb::fairing())
        .attach(AdHoc::on_ignite("Run Migrations", run_migrations))
}
