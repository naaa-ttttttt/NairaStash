use askama::Template;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use uuid::Uuid;

use super::services::{self, LedgerError, TransactionView};
use crate::domains::auth::handlers::AuthUser;
use crate::domains::dashboard::not_found_handler;
use crate::shared::render::render_page;
use crate::AppState;

// how many transactions the activity page shows
const ACTIVITY_LIMIT: i64 = 50;

#[derive(Template)]
#[template(path = "activity.html")]
pub struct ActivityTemplate {
    pub active: &'static str,
    pub transactions: Vec<TransactionView>,
}

#[derive(Template)]
#[template(path = "transaction_detail.html")]
pub struct TransactionDetailTemplate {
    pub active: &'static str,
    pub transaction: TransactionView,
}

// a missing transaction gets the 404 page, anything else is logged and becomes a 500
async fn ledger_error_response(err: LedgerError) -> Response {
    match err {
        LedgerError::NotFound => not_found_handler().await,
        other => {
            eprintln!("Ledger error: {:?}", other);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}


// GET /activity: the latest transactions of the logged in user
pub async fn activity_page_handler(auth: AuthUser, State(state): State<AppState>) -> Response {
    match services::get_user_activity(&state.db, auth.user_id, ACTIVITY_LIMIT).await {
        Ok(transactions) => render_page(
            ActivityTemplate { active: "activity", transactions },
            StatusCode::OK,
        ),
        Err(err) => ledger_error_response(err).await,
    }
}

// GET /activity/:entry_id: one transaction, a bad id or someone else's is a 404
pub async fn transaction_detail_handler(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(entry_id): Path<String>,
) -> Response {
    // uuid has no serde feature here, so the id arrives as text and is parsed by hand
    let Ok(entry_id) = Uuid::parse_str(&entry_id) else {
        return not_found_handler().await;
    };

    match services::get_transaction_details(&state.db, entry_id, auth.user_id).await {
        Ok(transaction) => render_page(
            TransactionDetailTemplate { active: "activity", transaction },
            StatusCode::OK,
        ),
        Err(err) => ledger_error_response(err).await,
    }
}
