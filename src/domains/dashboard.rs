use axum::{routing::get, http::StatusCode, response::Html, Router};
use askama::Template;
use crate::AppState;

#[derive(Template)]
#[template(path = "dashboard.html")]
pub struct DashboardTemplate { pub active: &'static str, pub total_balance: String }

#[derive(Template)]
#[template(path = "deposit.html")]
pub struct DepositTemplate { pub active: &'static str }

#[derive(Template)]
#[template(path = "profile.html")]
pub struct ProfileTemplate { pub active: &'static str }

#[derive(Template)]
#[template(path = "notifications.html")]
pub struct NotificationsTemplate { pub active: &'static str }

#[derive(Template)]
#[template(path = "activity.html")]
pub struct ActivityTemplate { pub active: &'static str }

#[derive(Template)]
#[template(path = "link_account.html")]
pub struct LinkAccountTemplate { pub active: &'static str }

#[derive(Template)]
#[template(path = "not_found.html")]
pub struct NotFoundTemplate { pub active: &'static str }

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/", get(dashboard_handler))
        .route("/deposit", get(deposit_page_handler))
        .route("/activity", get(activity_handler))
        .route("/accounts/link", get(link_account_handler))
        .route("/profile", get(profile_handler))
        .route("/notifications", get(notifications_handler))
        .with_state(state)
}

pub async fn dashboard_handler() -> Html<String> {
    Html(DashboardTemplate { active: "dashboard", total_balance: "0".to_string() }.render().unwrap())
}

pub async fn deposit_page_handler() -> Html<String> {
    Html(DepositTemplate { active: "deposit" }.render().unwrap())
}

pub async fn activity_handler() -> Html<String> {
    Html(ActivityTemplate { active: "activity" }.render().unwrap())
}

pub async fn link_account_handler() -> Html<String> {
    Html(LinkAccountTemplate { active: "profile" }.render().unwrap())
}

pub async fn profile_handler() -> Html<String> {
    Html(ProfileTemplate { active: "profile" }.render().unwrap())
}

pub async fn notifications_handler() -> Html<String> {
    Html(NotificationsTemplate { active: "profile" }.render().unwrap())
}

pub async fn not_found_handler() -> (StatusCode, Html<String>) {
    (StatusCode::NOT_FOUND, Html(NotFoundTemplate { active: "" }.render().unwrap()))
}

