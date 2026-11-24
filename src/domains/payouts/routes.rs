use axum::{
    routing::{get, post},
    Router,
};

use super::handlers::{create_payout_handler, payouts_page_handler, review_payout_handler};
use crate::AppState;

// every handler takes AuthUser, so every route here needs a logged in user
pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/payouts", get(payouts_page_handler).post(create_payout_handler))
        .route("/payouts/review", post(review_payout_handler))
        .with_state(state)
}
