use axum::{
    routing::{get, post},
    Router,
};

use super::handlers::{
    login_handler, login_submit_handler, logout_handler, signup_handler, signup_submit_handler,
};
use crate::AppState;

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/login", get(login_handler).post(login_submit_handler))
        .route("/signup", get(signup_handler).post(signup_submit_handler))
        .route("/logout", post(logout_handler))
        .with_state(state)
}
