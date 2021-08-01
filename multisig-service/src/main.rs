#[macro_use]
extern crate rocket;

use rocket_dyn_templates::{Template, context};

use rocket::fs::{FileServer, relative};
use rocket::response::Redirect;
use rocket::form::Form;

#[get("/")]
pub fn index() -> Redirect {
    Redirect::to(uri!("/", create_transaction()))
}

#[get("/view?<tid>")]
fn view_transaction(tid: Option<String>) -> Template {
    Template::render("view-tx", &context!{
        title: "Montelibero multisignature service",
        parent: "base",
        menu_view_tx: true,
    })
}

#[derive(FromForm)]
struct CreateTx {
    tx_body: String,
}

#[get("/create")]
fn create_transaction() -> Template {
    Template::render("create-tx", &context!{
        title: "Montelibero multisignature service",
        parent: "base",
        menu_create_tx: true,
    })
}

#[post("/create", data = "<tx>")]
async fn post_transaction(tx: Form<CreateTx>) -> Template { 
    Template::render("create-tx-response", &context!{
        title: "Montelibero multisignature service",
        parent: "base",
        menu_create_tx: true,
        txid: "sdfjlksdfjlsdkjfsdlkfj32423456",
        is_error: true,
        error_msg: "Transaction doesn't belong to MTL foundation"
    })
}

#[launch]
fn rocket() -> _ {
    rocket::build()
        .mount("/", FileServer::from(relative!("static")))
        .mount("/", routes![index, create_transaction, post_transaction, view_transaction])
        .attach(Template::fairing())
}
