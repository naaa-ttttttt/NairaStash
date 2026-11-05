use serde::Deserialize;
use axum::{body::Bytes, http::HeaderMap, response::IntoResponse, http::StatusCode};

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
    raw_bytes: axum::body::Bytes,
    headers: axum::http::HeaderMap,
) -> impl axum::response::IntoResponse {
    
    //syntax for extracting the header signature from the captured request
    let header_signature = match headers.get("x-paystack-signature") {
        Some(header_value) => {
            header_value.to_str().unwrap_or("")
        }
        None => {
            return axum::http::StatusCode::BAD_REQUEST.into_response();
        }
    };

    //we get the secret key from the .env file.
    let secret_key = std::env::var("PAYSTACK_SECRET_KEY").unwrap_or_else(|_| "".to_string());

    let is_valid = verify_paystack_signature(
        &raw_bytes,
        header_signature,
        &secret_key,
    );

    if !is_valid {
        return StatusCode::UNAUTHORIZED.into_response();
    }


    let payload: super::dto::PaystackWebhook = match serde_json::from_slice(&raw_bytes) {
        Ok(data) => data,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    }


    StatusCode::OK.into_response()

}
