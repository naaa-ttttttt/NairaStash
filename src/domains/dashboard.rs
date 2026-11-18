use askama::Template;
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use sqlx::PgPool;
use uuid::Uuid;

use crate::domains::auth::handlers::AuthUser;
use crate::domains::ledger::services::{get_user_activity, TransactionView};
use crate::shared::money::format_kobo_as_naira;
use crate::shared::render::render_page;
use crate::AppState;
use crate::domains::plans::handlers::{plan_view, PlanView};
use crate::domains::plans::services::list_active_plans;

#[derive(Template)]
#[template(path = "dashboard.html")]
pub struct DashboardTemplate {
    pub active: &'static str,
    pub total_balance: String,
    pub recent_transactions: Vec<TransactionView>,
}

#[derive(Template)]
#[template(path = "deposit.html")]
pub struct DepositTemplate {
    pub active: &'static str,
    pub wallet_balance: String,
}

#[derive(Template)]
#[template(path = "notifications.html")]
pub struct NotificationsTemplate {
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

// the profile page lives in profile.rs and the activity pages live in the ledger feature
pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/", get(dashboard_handler))
        .route("/deposit", get(deposit_page_handler))
        .route("/accounts/link", get(link_account_handler))
        .route("/notifications", get(notifications_handler))
        .with_state(state)
}

// reads the wallet balance in kobo, 0 only when the user has no wallet row
async fn fetch_wallet_balance_in_kobo(db: &PgPool, user_id: Uuid) -> Result<i64, sqlx::Error> {
    let balance = sqlx::query_scalar::<_, i64>("SELECT balance FROM wallets WHERE user_id = $1")
        .bind(user_id)
        .fetch_optional(db)
        .await?;

    Ok(balance.unwrap_or(0)) // no semicolon
}

// how many transactions the dashboard shows
const RECENT_ACTIVITY_LIMIT: i64 = 5;

// AuthUser is the guard: no valid session sends the visitor to /login
pub async fn dashboard_handler(auth: AuthUser, State(state): State<AppState>) -> Response {
    // a failed read is an error page, never a fake zero balance
    let balance_in_kobo = match fetch_wallet_balance_in_kobo(&state.db, auth.user_id).await {
        Ok(balance) => balance,
        Err(err) => {
            eprintln!("Could not read wallet balance: {:?}", err);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

        // a failed read is an error page, never a fake empty list
    let recent_transactions =
        match get_user_activity(&state.db, auth.user_id, RECENT_ACTIVITY_LIMIT).await {
            Ok(transactions) => transactions,
            Err(err) => {
                eprintln!("Could not read recent activity: {:?}", err);
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        };

    let page = DashboardTemplate {
        active: "home", // the nav bar calls this tab "home"
        total_balance: format_kobo_as_naira(balance_in_kobo),
        recent_transactions,
    };

    render_page(page, StatusCode::OK)
}
    
// the page that adds money to the wallet, it shows the current wallet balance
pub async fn deposit_page_handler(auth: AuthUser, State(state): State<AppState>) -> Response {
    let balance_in_kobo = match fetch_wallet_balance_in_kobo(&state.db, auth.user_id).await {
        Ok(balance) => balance,
        Err(err) => {
            eprintln!("Could not read wallet balance: {:?}", err);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let page = DepositTemplate {
        active: "deposit",
        wallet_balance: format_kobo_as_naira(balance_in_kobo),
    };

    render_page(page, StatusCode::OK)
}

pub async fn link_account_handler(_auth: AuthUser) -> Response {
    render_page(LinkAccountTemplate { active: "profile" }, StatusCode::OK)
}

pub async fn notifications_handler(_auth: AuthUser) -> Response {
    render_page(NotificationsTemplate { active: "profile" }, StatusCode::OK)
}

pub async fn not_found_handler() -> Response {
    render_page(NotFoundTemplate { active: "" }, StatusCode::NOT_FOUND)
}
