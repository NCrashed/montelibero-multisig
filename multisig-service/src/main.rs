#[macro_use]extern crate rocket;
#[macro_use] extern crate rocket_sync_db_pools;
#[macro_use] extern crate diesel_migrations;

use rocket_dyn_templates::{context, Template};

use rocket::{Rocket, Build};
use rocket::fairing::AdHoc;
use rocket::form::Form;
use rocket::fs::{relative, FileServer};
use rocket::response::Redirect;
use rocket_sync_db_pools::diesel;

use montelibero_transactions::transaction::validate_mtl_tx;

#[database("transactions")]
struct TransactionsDb(diesel::SqliteConnection);

#[get("/")]
pub fn index() -> Redirect {
    Redirect::to(uri!("/", create_transaction()))
}

#[get("/view?<tid>")]
fn view_transaction(conn: TransactionsDb, tid: Option<String>) -> Template {
    Template::render(
        "view-tx",
        &context! {
            title: "Montelibero multisignature service",
            parent: "base",
            menu_view_tx: true,
        },
    )
}

#[derive(FromForm)]
struct CreateTx {
    tx_body: String,
}

#[get("/create")]
fn create_transaction(conn: TransactionsDb) -> Template {
    Template::render(
        "create-tx",
        &context! {
            title: "Montelibero multisignature service",
            parent: "base",
            menu_create_tx: true,
        },
    )
}

#[post("/create", data = "<tx>")]
async fn post_transaction(conn: TransactionsDb, tx: Form<CreateTx>) -> Template {
    if tx.tx_body.len() == 0 {
        Template::render(
            "create-tx-response",
            &context! {
                title: "Montelibero multisignature service",
                parent: "base",
                menu_create_tx: true,
                is_error: true,
                error_msg: "Transaction body is empty"
            },
        )
    } else {
        match validate_mtl_tx(&tx.tx_body) {
            Ok(mtx) => Template::render(
                "create-tx-response",
                &context! {
                    title: "Montelibero multisignature service",
                    parent: "base",
                    menu_create_tx: true,
                    txid: hex::encode(mtx.txid()),
                    is_error: false,
                },
            ),
            Err(e) => Template::render(
                "create-tx-response",
                &context! {
                    title: "Montelibero multisignature service",
                    parent: "base",
                    menu_create_tx: true,
                    is_error: true,
                    error_msg: format!("{}", e)
                },
            ),
        }
    }
}

async fn run_migrations(rocket: Rocket<Build>) -> Rocket<Build> {
    embed_migrations!();

    let conn = TransactionsDb::get_one(&rocket).await.expect("database connection");
    conn.run(|c| embedded_migrations::run(c)).await.expect("can run migrations");

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
                view_transaction
            ],
        )
        .attach(Template::fairing())
        .attach(TransactionsDb::fairing())
        .attach(AdHoc::on_ignite("Run Migrations", run_migrations))
}
