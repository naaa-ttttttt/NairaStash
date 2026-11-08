use axum::{routing::get, response::Html, Router};
use askama::Template;
use crate::AppState;

#[derive(Template)]
#[template(path = "withdraw_plan.html")]
pub struct WithdrawPlanTemplate { pub active: &'static str }

#[derive(Template)]
#[template(path = "withdraw_amount.html")]
pub struct WithdrawAmountTemplate { pub active: &'static str }

#[derive(Template)]
#[template(path = "withdraw_destination.html")]
pub struct WithdrawDestinationTemplate { pub active: &'static str }

#[derive(Template)]
#[template(path = "withdraw_review.html")]
pub struct WithdrawReviewTemplate { pub active: &'static str }

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
        .route("/withdraw", get(withdraw_plan_handler))
        .route("/withdraw/amount", get(withdraw_amount_handler))
        .route("/withdraw/destination", get(withdraw_destination_handler))
        .route("/withdraw/review", get(withdraw_review_handler))
        .route("/loans/new", get(loan_request_handler))
        .route("/loans/review", get(loan_review_handler))
        .route("/loans/detail", get(loan_detail_handler))
        .with_state(state)
}

pub async fn withdraw_plan_handler() -> Html<String> {
    Html(WithdrawPlanTemplate { active: "none" }.render().unwrap())
}

pub async fn withdraw_amount_handler() -> Html<String> {
    Html(WithdrawAmountTemplate { active: "home" }.render().unwrap())
}

pub async fn withdraw_destination_handler() -> Html<String> {
    Html(WithdrawDestinationTemplate { active: "home" }.render().unwrap())
}

pub async fn withdraw_review_handler() -> Html<String> {
    Html(WithdrawReviewTemplate { active: "home" }.render().unwrap())
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

