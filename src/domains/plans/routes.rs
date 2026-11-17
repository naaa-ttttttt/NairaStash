use axum::{routing::get, Router};

use super::handlers::{
    create_plan_handler, new_plan_page_handler, plan_detail_handler, plans_page_handler,
};
use crate::AppState;

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/plans", get(plans_page_handler))
        .route("/plans/new", get(new_plan_page_handler).post(create_plan_handler))
        .route("/plans/:plan_id", get(plan_detail_handler)) // axum 0.7 uses :name
        .with_state(state)
}
