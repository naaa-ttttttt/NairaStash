use axum::{routing::post, Router};

use super::handlers::add_money_handler;
use crate::AppState;

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/plans/:plan_id/add", post(add_money_handler)) // axum 0.7 uses :name
        .with_state(state)
}
