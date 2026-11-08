use axum::{routing::get, response::{Html, Redirect}, Router};
use askama::Template;
use crate::AppState;

#[derive(Template)]
#[template(path = "login.html")]
pub struct LoginTemplate { pub active: &'static str }

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/login", get(login_handler).post(login_submit_handler))
        .route("/signup", get(signup_handler).post(signup_submit_handler))
        .with_state(state)
}

pub async fn login_handler() -> Html<String> {
    Html(LoginTemplate { active: "" }.render().unwrap())
}

pub async fn login_submit_handler() -> Redirect { Redirect::to("/") }

pub async fn signup_handler() -> Html<String> {
    Html(LoginTemplate { active: "" }.render().unwrap())
}

pub async fn signup_submit_handler() -> Redirect { Redirect::to("/") }

