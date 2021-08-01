#[macro_use]
extern crate rocket;

use rocket_dyn_templates::{context, Template};

use rocket::form::Form;
use rocket::fs::{relative, FileServer};
use rocket::response::Redirect;

use montelibero_transactions::transaction::validate_mtl_tx;

#[get("/")]
pub fn index() -> Redirect {
    Redirect::to(uri!("/", create_transaction()))
}

#[get("/view?<tid>")]
fn view_transaction(tid: Option<String>) -> Template {
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

#[post("/create", data = "<tx>")]
async fn post_transaction(tx: Form<CreateTx>) -> Template {
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
}
