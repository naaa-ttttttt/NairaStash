use axum::{routing::get, Router};

use super::handlers::{activity_page_handler, transaction_detail_handler};
use crate::AppState;

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/activity", get(activity_page_handler))
        .route("/activity/:entry_id", get(transaction_detail_handler)) // axum 0.7 uses :name
        .with_state(state)
}
