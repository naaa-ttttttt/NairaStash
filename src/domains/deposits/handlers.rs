use axum::{
    body::Bytes,
    extract::{Form, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::AppState;
use super::pay_stack::verify_paystack_signature;
use super::services::DepositError;

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

#[derive(serde::Deserialize)]
pub struct DepositFormInput {
    pub email: String,
    pub amount: i64,
}

// decides what to tell paystack
// 200 means stop retrying, 500 means try again later
fn webhook_status_for(error: &DepositError) -> StatusCode {
    match error {
        DepositError::DatabaseError(_) | DepositError::WalletNotFound => {
            StatusCode::INTERNAL_SERVER_ERROR
        }
        DepositError::NotFound
        | DepositError::AmountMismatch
        | DepositError::InvalidCurrency
        | DepositError::InvalidStatus => StatusCode::OK,
    }
}

pub async fn handle_paystack_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    raw_bytes: Bytes,
) -> impl IntoResponse {
    // 1. INPUTS
    let header_signature = match headers.get("x-paystack-signature") {
        Some(header_value) => header_value.to_str().unwrap_or(""),
        None => return StatusCode::BAD_REQUEST.into_response(),
    };

    let secret_key = &state.paystack.secret_key;

    // 2. GUARDS
    let is_valid = verify_paystack_signature(&raw_bytes, header_signature, secret_key);
    if !is_valid {
        return StatusCode::UNAUTHORIZED.into_response();
    }

    let payload: PaystackWebhook = match serde_json::from_slice(&raw_bytes) {
        Ok(data) => data,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };

    // 3. TRANSFORMATIONS
    if let Err(err) = super::services::process_deposit_webhook(&state.db, payload).await {
        eprintln!("Webhook processing failed: {:?}", err);
        return webhook_status_for(&err).into_response();
    }

    // 4. OUTPUTS
    StatusCode::OK.into_response()
}

pub async fn initialize_deposit_handler(
    State(state): State<AppState>,
    Form(input): Form<DepositFormInput>,
) -> impl axum::response::IntoResponse {
    // 1. Convert Naira to Kobo
    let amount_in_kobo = input.amount * 100;
    let temp_user_id = Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap();
    let temp_plan_id = Uuid::nil();

    // 2. Generate unique tracking reference
    let tracking_reference = format!("NS-DEP-{}", &Uuid::new_v4().to_string()[..8]);

    println!(
        "Attempting Paystack Handshake for Email: {}, Amount: {} Kobo",
        input.email, amount_in_kobo
    );

    if let Err(e) = super::repo::create_pending_deposits(
        &state.db,
        temp_user_id,
        temp_plan_id,
        &tracking_reference,
        amount_in_kobo,
        super::repo::DepositType::Manual,
    )
    .await
    {
        println!("DB Error creating pending deposit: {:?}", e);
        return (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response();
    }

    // 3. Fire outbound call
    match super::pay_stack::initialize_payment(
        &input.email,
        amount_in_kobo,
        &tracking_reference,
        &state.paystack,
    )
    .await
    {
        Ok(checkout_url) => {
            println!("SUCCESS: Paystack Link Generated: {}", checkout_url);
            (axum::http::StatusCode::OK, checkout_url).into_response()
        }
        Err(err) => {
            eprintln!("CRITICAL PAYSTACK ERROR LOG: {:?}", err);

            if let Some(status) = err.status() {
                if status == reqwest::StatusCode::UNAUTHORIZED {
                    eprintln!("WARNING: Your PAYSTACK_SECRET_KEY in your .env file is being rejected by Paystack's servers! Double check that it has no typos and starts with sk_test_");
                }
            }

            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to connect to Paystack gateway",
            )
                .into_response()
        }
    }
}
