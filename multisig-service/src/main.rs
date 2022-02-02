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

use chrono::{Duration, NaiveDateTime, Utc};
use rocket::fairing::AdHoc;
use rocket::form::Form;
use rocket::fs::{relative, FileServer};
use rocket::http::{Cookie, CookieJar};
use rocket::response::Redirect;
use rocket::serde::{json::Json, Deserialize, Serialize};
use rocket::{Build, Rocket, State};
use rocket_dyn_templates::{context, Template};
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::Mutex;

use montelibero_transactions::account::*;
use montelibero_transactions::error::MtlError;
use montelibero_transactions::transaction::*;

#[derive(Clone)]
struct Cache {
    blocks: Arc<Mutex<HashMap<Vec<u8>, NaiveDateTime>>>,
    users: UsersMapping,
    signs: Arc<Mutex<SignsMapping>>,
}

impl Cache {
    fn new(users: UsersMapping) -> Self {
        Cache {
            blocks: Arc::new(Mutex::new(HashMap::new())),
            users,
            signs: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    async fn is_blocked(&self, tid: &[u8]) -> bool {
        if let Some(t) = self.blocks.lock().await.get(tid) {
            *t > Utc::now().naive_utc()
        } else {
            false
        }
    }

    async fn block(&self, tid: &[u8], delay: Duration) {
        self.blocks
            .lock()
            .await
            .insert(tid.to_owned(), Utc::now().naive_utc() + delay);
    }

    async fn unblock(&self, tid: &[u8]) {
        self.blocks.lock().await.remove(tid);
    }

    async fn update_signs(&self, conn: &TransactionsDb) -> Result<(), SignsMappingError> {
        let mut signs_mut = self.signs.lock().await;
        *signs_mut = read_user_recent_signs(conn).await?;
        Ok(())
    }
}

pub type SignsMapping = HashMap<substrate_stellar_sdk::PublicKey, u32>;

#[derive(Debug, Error)]
pub enum SignsMappingError {
    #[error("{0}")]
    Mtl(#[from] MtlError),
    #[error("{0}")]
    DatabaseError(#[from] TxLoadError),
}

pub async fn read_user_recent_signs(
    conn: &TransactionsDb,
) -> Result<SignsMapping, SignsMappingError> {
    let month_ago = chrono::Utc::now() - chrono::Duration::days(30);
    let txs = get_transactions(conn, month_ago.naive_utc()).await?;
    let mut result = HashMap::new();
    let mut acc_cache = HashMap::new();
    for mtx in txs {
        for (tx, _) in mtx.history.iter().take(1) {
            let account_id = tx.source_account()?;
            let account = match acc_cache.get(&account_id) {
                Some(acc) => acc,
                None => {
                    let account = tx.fetch_source_account()?;
                    acc_cache.insert(account_id.clone(), account);
                    acc_cache.get(&account_id).unwrap()
                }
            };
            let signs = tx.get_signed_keys(account)?;
            for (s, _) in signs {
                *result.entry(s).or_insert(1) += 1;
            }
        }
    }

    Ok(result)
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
    pub short_key: String,
    pub weight: i32,
    pub signed: bool,
    pub telegram: Option<String>,
    pub singed_monthly: u32,
    pub is_few_signs: bool,
    pub is_moderate_signs: bool,
}

impl ViewSigner {
    pub fn collect(
        telegram_map: &UsersMapping,
        signs_map: &SignsMapping,
        account: &AccountResponse,
        signs: &[SignatureHint],
    ) -> Result<Vec<Self>, MtlError> {
        let mut res = Vec::new();
        for s in get_mtl_signers(account)? {
            let signer_weight = s.1;
            let signer_key = s.0;
            if signer_weight > 0 {
                let singed_monthly = signs_map.get(&signer_key).copied().unwrap_or(0);
                let key = std::str::from_utf8(&signer_key.to_encoding())
                    .unwrap()
                    .to_owned();
                let short_key = format!("{}...{}", &key[0 .. 15], &key[key.len()-15 ..]);
                res.push(ViewSigner {
                    key,
                    short_key,
                    weight: signer_weight,
                    signed: signs.contains(&signer_key.get_signature_hint()),
                    telegram: telegram_map.get(&signer_key).cloned(),
                    singed_monthly,
                    is_few_signs: singed_monthly < 4,
                    is_moderate_signs: singed_monthly < 10,
                });
            }
        }
        res.sort_by(|a, b| b.weight.cmp(&a.weight));
        Ok(res)
    }
}

#[derive(Serialize)]
pub struct TxHistoryItem {
    pub number: u32,
    pub date: String,
    pub tx: String,
}

impl TxHistoryItem {
    pub fn collect(tx: &MtlTxMeta) -> Vec<Self> {
        let mut res = Vec::new();
        let n = tx.history.len();
        for (i, (tx, t)) in tx.history.iter().enumerate() {
            res.push(TxHistoryItem {
                number: (n - i) as u32,
                date: t.format("%Y-%m-%d %H:%M:%S").to_string(),
                tx: tx.into_encoding(),
            })
        }
        res.sort_by(|a, b| a.number.cmp(&b.number));
        res
    }
}

#[get("/view?<tid>")]
async fn view_transaction(
    conn: TransactionsDb,
    cache: &State<Cache>,
    cookies: &CookieJar<'_>,
    tid: Option<String>,
) -> Template {
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

    async fn view(
        conn: TransactionsDb,
        cache: &State<Cache>,
        cookies: &CookieJar<'_>,
        mtid: Option<String>,
    ) -> Result<Template, ViewError> {
        let txid = match mtid {
            None => return Err(ViewError::NotImplemented),
            Some(v) => hex::decode(&v)?,
        };
        let tx = get_transaction(&conn, txid.clone()).await?;
        let curr_tx = tx.current().0;

        async fn render_tx(
            cache: &State<Cache>,
            cookies: &CookieJar<'_>,
            txid: &[u8],
            tx: &MtlTxMeta,
            published: bool,
            invalid: Option<String>,
        ) -> Result<Template, ViewError> {
            let users = &cache.users;
            let curr_tx = tx.current().0;
            let is_blocked = cache.is_blocked(txid).await;
            let mut is_blocker = cookies.get("is_blocker").is_some();
            if !is_blocked && is_blocker {
                cookies.remove(Cookie::new("is_blocker", ""));
                is_blocker = false;
            }
            let account = curr_tx.fetch_source_account()?;
            let signs = curr_tx.get_signed_keys(&account)?;
            let hints: Vec<SignatureHint> =
                signs.iter().map(|s| s.0.get_signature_hint()).collect();
            let tx_collected: i32 = signs.iter().map(|s| s.1).sum();
            let tx_signers = {
                let signs_map = cache.signs.lock().await;
                ViewSigner::collect(users, &signs_map, &account, &hints)?
            };
            let tx_ignorants: Vec<String> = tx_signers
                .iter()
                .filter(|s| !s.signed && s.telegram.is_some())
                .map(|s| s.telegram.clone().unwrap())
                .collect();
            let tx_history = TxHistoryItem::collect(tx);
            Ok(Template::render(
                "view-tx",
                &context! {
                    title: "Montelibero multisignature service",
                    parent: "base",
                    menu_view_tx: true,
                    is_error: false,
                    tx_id: hex::encode(txid),
                    tx_title: tx.title.clone(),
                    tx_description: tx.description.clone(),
                    tx_last: curr_tx.into_encoding(),
                    tx_required: get_required_weight(&account),
                    tx_collected,
                    is_blocked,
                    is_blocker,
                    tx_signers,
                    tx_ignorants,
                    tx_published: published,
                    tx_updates: tx.history.len(),
                    tx_invalid: invalid.is_some(),
                    tx_invalid_msg: invalid,
                    tx_history,
                },
            ))
        }

        match curr_tx.is_published() {
            Ok(true) => render_tx(cache, cookies, &txid, &tx, true, None).await,
            _ => match curr_tx.validate_create() {
                Ok(_) => render_tx(cache, cookies, &txid, &tx, false, None).await,
                Err(e) => {
                    render_tx(cache, cookies, &txid, &tx, false, Some(format!("{}", e))).await
                }
            },
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
async fn block_transaction(
    conn: TransactionsDb,
    cache: &State<Cache>,
    cookies: &CookieJar<'_>,
    txid: String,
) -> Json<BlockResp> {
    async fn block(
        conn: TransactionsDb,
        cache: &State<Cache>,
        tid: &str,
    ) -> Result<(), BlockError> {
        let txid = hex::decode(tid)?;
        let _ = get_transaction(&conn, txid.clone()).await?;
        if cache.is_blocked(&txid).await {
            return Err(BlockError::AlreadyBlocked);
        }
        cache.block(&txid, Duration::minutes(5)).await;
        Ok(())
    }

    match block(conn, cache, &txid).await {
        Ok(_) => {
            cookies.add(Cookie::new("is_blocker", ""));
            Json(BlockResp { error: None })
        }
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
async fn unblock_transaction(
    conn: TransactionsDb,
    cache: &State<Cache>,
    cookies: &CookieJar<'_>,
    txid: String,
) -> Json<UnBlockResp> {
    async fn unblock(
        conn: TransactionsDb,
        cache: &State<Cache>,
        tid: &str,
    ) -> Result<(), UnBlockError> {
        let txid = hex::decode(tid)?;
        let _ = get_transaction(&conn, txid.clone()).await?;
        if !cache.is_blocked(&txid).await {
            return Err(UnBlockError::NotBlocked);
        }
        cache.unblock(&txid).await;
        Ok(())
    }

    match unblock(conn, cache, &txid).await {
        Ok(_) => {
            cookies.remove(Cookie::new("is_blocker", ""));
            Json(UnBlockResp { error: None })
        }
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

    if tx.tx_body.is_empty() {
        render_error("Transaction body is empty")
    } else if tx.tx_title.is_empty() {
        render_error("Transaction title is empty")
    } else {
        match validate_mtl_tx(&tx.tx_body) {
            Ok(mtx) => {
                match store_transaction(
                    &conn,
                    mtx.clone(),
                    tx.tx_title.clone(),
                    tx.tx_description.clone(),
                )
                .await
                {
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

#[derive(Debug, Error)]
pub enum UpdateError {
    #[error("Failed to decode transaction ID")]
    TransactionId(#[from] hex::FromHexError),
    #[error("Failed to load transaction: {0}")]
    TransactionLoad(#[from] database::TxLoadError),
    #[error("Update transaction is empty")]
    TransactionEmpty,
    #[error("Update contains no new signatures")]
    TransactionNotChanged,
    #[error("{0}")]
    MtlError(#[from] MtlError),
    #[error("Database error: {0}")]
    DatabaseError(#[from] diesel::result::Error),
    #[error("Failed to update signs mapping: {0}")]
    SignsError(#[from] SignsMappingError),
}

#[post("/update", data = "<tx>")]
async fn update_transaction(
    conn: TransactionsDb,
    cache: &State<Cache>,
    tx: Form<UpdateTx>,
) -> Result<Redirect, Template> {
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

    async fn update(
        conn: TransactionsDb,
        cache: &State<Cache>,
        tx: Form<UpdateTx>,
    ) -> Result<MtlTransaction, UpdateError> {
        if tx.tx_body.is_empty() {
            return Err(UpdateError::TransactionEmpty);
        }
        let mtx = validate_mtl_tx(&tx.tx_body)?;
        let txid = mtx.txid();
        let old_tx = get_transaction(&conn, txid.clone()).await?;
        old_tx.current().0.validate_update(&mtx)?;
        if mtx.into_bytes() == old_tx.current().0.into_bytes() {
            return Err(UpdateError::TransactionNotChanged);
        }
        store_transaction_update(&conn, mtx.clone()).await?;
        cache.unblock(&txid).await;
        cache.update_signs(&conn).await?;
        Ok(mtx)
    }

    match update(conn, cache, tx).await {
        Err(e) => Err(render_error(&format!("{}", e))),
        Ok(tx) => {
            let url = uri!(view_transaction(tid = Some(hex::encode(tx.txid()))));
            Ok(Redirect::to(url))
        }
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
async fn check_update_transaction(
    conn: TransactionsDb,
    cache: &State<Cache>,
    txid: String,
    updates: u32,
    block: bool,
    published: bool,
) -> Json<CheckResult> {
    async fn check(
        conn: TransactionsDb,
        cache: &State<Cache>,
        txid: String,
        updates: u32,
        block: bool,
        published: bool,
    ) -> Result<bool, CheckError> {
        let txid = hex::decode(&txid)?;
        let meta = get_transaction(&conn, txid.clone()).await?;
        let is_blocked = cache.is_blocked(&txid).await;
        let is_published = meta.current().0.is_published().unwrap_or(false);
        Ok(
            block != is_blocked
                || updates != meta.history.len() as u32
                || published != is_published,
        )
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

async fn load_signs(rocket: Rocket<Build>, signs_mux: Arc<Mutex<SignsMapping>>) -> Rocket<Build> {
    let conn = TransactionsDb::get_one(&rocket)
        .await
        .expect("database connection");

    let signs = read_user_recent_signs(&conn)
        .await
        .expect("loaded signatures");

    {
        let mut mut_signs = signs_mux.lock().await;
        *mut_signs = signs;
    }
    rocket
}

#[derive(Deserialize)]
struct Config {
    statics: Option<String>,
    users: String,
}

#[launch]
fn rocket() -> _ {
    let builder = rocket::build();
    let figment = builder.figment();
    let config: Config = figment.extract().expect("config");

    let statics = config
        .statics
        .unwrap_or_else(|| relative!("static").to_owned());
    let users = get_telegram_mapping(&config.users).unwrap();
    let cache = Cache::new(users);
    builder
        .mount("/", FileServer::from(&statics))
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
        .manage(cache.clone())
        .attach(Template::fairing())
        .attach(TransactionsDb::fairing())
        .attach(AdHoc::on_ignite("Run Migrations", run_migrations))
        .attach(AdHoc::on_ignite("Load initial signs", move |rocket| {
            load_signs(rocket, cache.signs)
        }))
}
