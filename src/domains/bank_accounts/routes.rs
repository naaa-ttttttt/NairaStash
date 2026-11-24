use axum::{
    routing::{get, post},
    Router,
};

use super::handlers::{
    banks_page_handler, lookup_bank_handler, new_bank_page_handler, remove_bank_handler,
    resolve_bank_handler, save_bank_handler,
};
use crate::AppState;

// every route here takes AuthUser in its handler, so all of them need a login
pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/banks", get(banks_page_handler).post(save_bank_handler))
        .route("/banks/new", get(new_bank_page_handler))
        .route("/banks/resolve", post(resolve_bank_handler))
        .route("/banks/lookup", post(lookup_bank_handler))
        .route("/banks/:bank_account_id/remove", post(remove_bank_handler)) // axum 0.7 uses :name
        .with_state(state)
}
