use serde::Deserialize;
use axum::{body::Bytes, http::{HeaderMap, StatusCode}, response::IntoResponse};
use super::pay_stack::verify_paystack_signature;

#[derive(Debug, Deserialize)]
pub struct PaystackWebhook {
    pub event: String,
    pub data: WebhookData,
}

#[derive(Debug, Deserialize)]
pub struct WebhookData {
    pub id: u64,
    pub status: String,
    pub reference: String,
    pub amount: i64,
    pub currency: String,
    pub customer: WebhookCustomer,
}

#[derive(Debug, Deserialize)]
pub struct WebhookCustomer {
    pub email: String,
}

pub async fn handle_paystack_webhook(
    headers: HeaderMap,
    raw_bytes: Bytes,
) -> impl IntoResponse {
    // 1. Extract the header signature string slice from the captured request
    let header_signature = match headers.get("x-paystack-signature") {
        Some(header_value) => {
            header_value.to_str().unwrap_or("")
        }
        None => {
            return StatusCode::BAD_REQUEST.into_response();
        }
    };

    // 2. Fetch the cryptographic secret key from RAM environment variables
    let secret_key = std::env::var("PAYSTACK_SECRET_KEY")
        .unwrap_or_else(|_| "".to_string());

    // 3. Trigger our constant-time cryptographic signature verification engine
    let is_valid = verify_paystack_signature(
        &raw_bytes,
        header_signature,
        &secret_key,
    );

    // 4. Guard against malicious or invalid request payloads
    if !is_valid {
        return StatusCode::UNAUTHORIZED.into_response();
    }

    // 5. Transform raw binary bytes into our strongly-typed data package structure
    let _payload: PaystackWebhook = match serde_json::from_slice(&raw_bytes) {
        Ok(data) => data,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };

    // Note: Once we finish validating the whole pipeline, we will orchestrate 
    // service layer policy rules right here before answering Paystack!

    StatusCode::OK.into_response()
}

pub async fn initialize_deposit_handler() -> impl axum::response::IntoResponse {
    // This acts as the placeholder endpoint for initializing our deposits
    axum::http::StatusCode::OK
}

