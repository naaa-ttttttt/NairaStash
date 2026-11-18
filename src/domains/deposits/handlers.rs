use axum::{
    body::Bytes,
    extract::{Form, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use serde::Deserialize;

use super::pay_stack::verify_paystack_signature;
use super::services::{
    process_deposit_webhook, start_deposit, DepositError, StartDepositError, WebhookFacts,
};
use crate::domains::auth::handlers::AuthUser;
use crate::AppState;

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

// only the amount is read from the form, who is paying comes from the session
#[derive(Deserialize)]
pub struct DepositFormInput {
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
        DepositError::DatabaseError(_) | DepositError::WalletNotFound | DepositError::Ledger(_) => {
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

pub async fn handle_paystack_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    raw_bytes: Bytes,
) -> Response {
    // 1. INPUTS
    // syntax to extract the header from the webhook
    let header_signature = match headers.get("x-paystack-signature") {
        Some(header_value) => header_value.to_str().unwrap_or(""),
        None => return StatusCode::BAD_REQUEST.into_response(),
    };

    // 2. GUARDS
    if !verify_paystack_signature(&raw_bytes, header_signature, &state.paystack.secret_key) {
        return StatusCode::UNAUTHORIZED.into_response();
    }

    // extracting the body of the webhook.
    let payload: PaystackWebhook = match serde_json::from_slice(&raw_bytes) {
        Ok(data) => data,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };

    // 3. TRANSFORMATIONS: hand the service plain values, not http types
    let facts = WebhookFacts {
        event: payload.event,
        status: payload.data.status,
        reference: payload.data.reference,
        amount: payload.data.amount,
        currency: payload.data.currency,
    };

    if let Err(err) = process_deposit_webhook(&state.db, facts).await {
        eprintln!("Webhook processing failed: {:?}", err);
        return webhook_status_for(&err).into_response();
    }

    // 4. OUTPUTS
    StatusCode::OK.into_response()
}

// picks the status and the message for each way starting a deposit can fail
fn start_error_response(err: &StartDepositError) -> Response {
    match err {
        StartDepositError::AmountTooSmall => {
            (StatusCode::BAD_REQUEST, "Amount must be at least 1 naira").into_response()
        }
        StartDepositError::AmountTooLarge => {
            (StatusCode::BAD_REQUEST, "That amount is too large").into_response()
        }
        StartDepositError::UserNotFound => {
            (StatusCode::UNAUTHORIZED, "User not found").into_response()
        }
        StartDepositError::Database(e) => {
            eprintln!("Deposit database error: {:?}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response()
        }
        StartDepositError::Paystack(e) => {
            eprintln!("Paystack error: {:?}", e);

            if e.status() == Some(reqwest::StatusCode::UNAUTHORIZED) {
                eprintln!("WARNING: Paystack rejected PAYSTACK_SECRET_KEY, check .env for typos");
            }

            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to connect to Paystack gateway",
            )
                .into_response()
        }
    }
}

// starts a deposit for the logged in user and gives back the paystack checkout link
pub async fn initialize_deposit_handler(
    auth: AuthUser,
    State(state): State<AppState>,
    Form(input): Form<DepositFormInput>,
) -> Response {
    match start_deposit(
        &state.db,
        &state.http_client,
        &state.paystack,
        auth.user_id,
        input.amount,
    )
    .await
    {
        Ok(checkout_url) => (StatusCode::OK, checkout_url).into_response(),
        Err(err) => start_error_response(&err),
    }
}
