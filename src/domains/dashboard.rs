use askama::Template;
use axum::{extract::State, http::StatusCode, response::Html, routing::get, Router};
use sqlx::PgPool;
use uuid::Uuid;

use crate::domains::auth::handlers::AuthUser;
use crate::AppState;

#[derive(Template)]
#[template(path = "dashboard.html")]
pub struct DashboardTemplate {
    pub active: &'static str,
    pub total_balance: String,
}

#[derive(Template)]
#[template(path = "deposit.html")]
pub struct DepositTemplate {
    pub active: &'static str,
}

#[derive(Template)]
#[template(path = "notifications.html")]
pub struct NotificationsTemplate {
    pub active: &'static str,
}

#[derive(Template)]
#[template(path = "activity.html")]
pub struct ActivityTemplate {
    pub active: &'static str,
}

#[derive(Template)]
#[template(path = "link_account.html")]
pub struct LinkAccountTemplate {
    pub active: &'static str,
}

#[derive(Template)]
#[template(path = "not_found.html")]
pub struct NotFoundTemplate {
    pub active: &'static str,
}

// the profile page now lives in profile.rs, so there is no profile route here
pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/", get(dashboard_handler))
        .route("/deposit", get(deposit_page_handler))
        .route("/activity", get(activity_handler))
        .route("/accounts/link", get(link_account_handler))
        .route("/notifications", get(notifications_handler))
        .with_state(state)
}

// reads the wallet balance in kobo, gives 0 if there is no wallet or the query fails
async fn fetch_wallet_balance_in_kobo(db: &PgPool, user_id: Uuid) -> i64 {
    let query_result =
        sqlx::query_scalar::<_, i64>("SELECT balance FROM wallets WHERE user_id = $1")
            .bind(user_id)
            .fetch_optional(db)
            .await;

    match query_result {
        Ok(Some(balance)) => balance,
        Ok(None) => 0,
        Err(err) => {
            eprintln!("Could not read wallet balance: {:?}", err);
            0
        }
    }
}

// 19000000 kobo becomes "190,000.00", 0 becomes "0.00"
fn format_kobo_as_naira(kobo: i64) -> String {
    let naira = kobo / 100;
    let remainder = kobo % 100;

    let digits = naira.to_string();
    let mut with_commas = String::new();

    for (position, digit) in digits.chars().enumerate() {
        let digits_left = digits.len() - position;
        if position > 0 && digits_left % 3 == 0 {
            with_commas.push(',');
        }
        with_commas.push(digit);
    }

    format!("{}.{:02}", with_commas, remainder) // no semicolon
}

// AuthUser is the guard: no valid session sends the visitor to /login
pub async fn dashboard_handler(auth: AuthUser, State(state): State<AppState>) -> Html<String> {
    let balance_in_kobo = fetch_wallet_balance_in_kobo(&state.db, auth.user_id).await;
    let total_balance = format_kobo_as_naira(balance_in_kobo);

    Html(
        DashboardTemplate {
            active: "dashboard",
            total_balance,
        }
        .render()
        .unwrap(),
    )
}

pub async fn deposit_page_handler(_auth: AuthUser) -> Html<String> {
    Html(DepositTemplate { active: "deposit" }.render().unwrap())
}

pub async fn activity_handler(_auth: AuthUser) -> Html<String> {
    Html(ActivityTemplate { active: "activity" }.render().unwrap())
}

pub async fn link_account_handler(_auth: AuthUser) -> Html<String> {
    Html(LinkAccountTemplate { active: "profile" }.render().unwrap())
}

pub async fn notifications_handler(_auth: AuthUser) -> Html<String> {
    Html(NotificationsTemplate { active: "profile" }.render().unwrap())
}

pub async fn not_found_handler() -> (StatusCode, Html<String>) {
    (
        StatusCode::NOT_FOUND,
        Html(NotFoundTemplate { active: "" }.render().unwrap()),
    )
}
