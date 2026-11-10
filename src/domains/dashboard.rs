use axum::{extract::State, routing::get, http::StatusCode, response::Html, Router};
use askama::Template;
use sqlx::PgPool;
use uuid::Uuid;
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

// the one place that decides who is logged in, swap this when real login exists
fn current_user_id() -> Uuid {
    let user_id = Uuid::parse_str(SEEDED_USER_ID).unwrap();
    user_id // no semicolon
}

// reads the wallet balance in kobo, gives 0 if there is no wallet or the query fails
async fn fetch_wallet_balance_in_kobo(db: &PgPool, user_id: Uuid) -> i64 {
    let query_result = sqlx::query_scalar::<_, i64>("SELECT balance FROM wallets WHERE user_id = $1")
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

// puts a comma every three digits, 10000 becomes 10,000
fn add_thousands_commas(whole_naira: i64) -> String {
    let digits = whole_naira.to_string();
    let mut result = String::new();

    for (position, digit) in digits.chars().enumerate() {
        let digits_left = digits.len() - position;
        if position > 0 && digits_left % 3 == 0 {
            result.push(',');
        }
        result.push(digit);
    }

    result // no semicolon
}

// kobo to naira text, 1000000 kobo becomes 10,000 and 1050 kobo becomes 10.50
fn format_kobo_as_naira(kobo: i64) -> String {
    let whole_naira = kobo / 100;
    let leftover_kobo = kobo % 100;
    let whole_with_commas = add_thousands_commas(whole_naira);

    if leftover_kobo == 0 {
        return whole_with_commas;
    }

    format!("{}.{:02}", whole_with_commas, leftover_kobo) // no semicolon
}

pub async fn dashboard_handler(State(state): State<AppState>) -> Html<String> {
    let user_id = current_user_id();
    let balance_in_kobo = fetch_wallet_balance_in_kobo(&state.db, user_id).await;
    let total_balance = format_kobo_as_naira(balance_in_kobo);

    Html(DashboardTemplate { active: "dashboard", total_balance }.render().unwrap())
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

const SEEDED_USER_ID: &str = "11111111-1111-1111-1111-111111111111";
