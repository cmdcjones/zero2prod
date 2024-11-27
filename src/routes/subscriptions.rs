use actix_web::web;

#[derive(serde::Deserialize)]
pub struct FormData {
    name: String,
    email: String,
}

pub async fn subscribe(_form: web::Form<FormData>) -> String {
    format!("{}", _form.name)
}
