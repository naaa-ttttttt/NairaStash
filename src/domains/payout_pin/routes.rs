use axum::{
    routing::{get, post},
    Router,
};

use super::handlers::{
    change_pin_handler, forgot_pin_handler, forgot_pin_page_handler, pin_page_handler,
    set_pin_handler,
};
use crate::AppState;

// every handler here takes AuthUser, so all of these routes need a login
pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/pin", get(pin_page_handler))
        .route("/pin/set", post(set_pin_handler))
        .route("/pin/change", post(change_pin_handler))
        .route("/pin/forgot", get(forgot_pin_page_handler).post(forgot_pin_handler))
        .with_state(state)
}
