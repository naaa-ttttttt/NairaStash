use axum::{routing::get, response::Html, Router};
use askama::Template;
use crate::AppState;

#[derive(Template)]
#[template(path = "loan_request.html")]
pub struct LoanRequestTemplate { pub active: &'static str }

#[derive(Template)]
#[template(path = "loan_review.html")]
pub struct LoanReviewTemplate { pub Security: &'static str, pub active: &'static str }

#[derive(Template)]
#[template(path = "loan_detail.html")]
pub struct LoanDetailTemplate { pub active: &'static str }

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/loans/new", get(loan_request_handler))
        .route("/loans/review", get(loan_review_handler))
        .route("/loans/detail", get(loan_detail_handler))
        .with_state(state)
}

pub async fn loan_request_handler() -> Html<String> {
    Html(LoanRequestTemplate { active: "groups" }.render().unwrap())
}

pub async fn loan_review_handler() -> Html<String> {
    Html(LoanReviewTemplate { Security: "", active: "groups" }.render().unwrap())
}

pub async fn loan_detail_handler() -> Html<String> {
    Html(LoanDetailTemplate { active: "groups" }.render().unwrap())
}
