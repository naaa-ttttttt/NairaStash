use askama::Template;
use axum::{
    http::StatusCode,
    response::{Html, IntoResponse, Response},
};

pub mod handlers;
pub mod repo;
pub mod routes;
pub mod services;

pub use routes::routes;

// one page for both tabs, active is "login" or "signup"
#[derive(Template)]
#[template(path = "login.html")]
pub struct LoginTemplate {
    pub active: &'static str,
    pub error: Option<String>,
    pub identifier: String,
    pub name: String,
    pub email: String,
    pub phone: String,
}

impl LoginTemplate {
    // a blank page with no error and no refilled values
    fn new(active: &'static str) -> Self {
        Self {
            active,
            error: None,
            identifier: String::new(),
            name: String::new(),
            email: String::new(),
            phone: String::new(),
        }
    }
}

// renders a template with a status code, logs and returns 500 if rendering fails
fn render(template: LoginTemplate, status: StatusCode) -> Response {
    match template.render() {
        Ok(html) => (status, Html(html)).into_response(),
        Err(e) => {
            eprintln!("template render failed: {e:?}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}
