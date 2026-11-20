use axum::{routing::post, Router};

use super::handlers::{add_money_handler, liquidate_handler, withdraw_handler};
use crate::AppState;

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/plans/:plan_id/add", post(add_money_handler)) // axum 0.7 uses :name
        .route("/plans/:plan_id/withdraw", post(withdraw_handler))
        .route("/plans/:plan_id/liquidate", post(liquidate_handler))
        .with_state(state)
}
