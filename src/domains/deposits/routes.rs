use axum::{routing::post, Router};
use crate::AppState;

// Pull both the initialization handler and the webhook handler from handlers.rs
use super::handlers::{handle_paystack_webhook, initialize_deposit_handler};

pub fn deposit_routes(state: AppState) -> Router {
    Router::new()
        // Room 1: Binds the incoming user trigger to the correct controller handler
        .route("/initialize", post(initialize_deposit_handler))
        
        // Room 2: Binds the Paystack transaction notification drop box
        .route("/webhook", post(handle_paystack_webhook))
        
        // Injects your global state/database tools safely
        .with_state(state)
}

