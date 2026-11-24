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
use crate::domains::payouts::services::{process_transfer_webhook, PayoutError, TransferFacts};
use crate::AppState;

// paystack sends every event to this one url, so the first job is to find out which kind of event it is
// this reads only the name, the full shape is read after we know which event it is
#[derive(Debug, Deserialize)]
struct WebhookEvent {
    event: String,
}

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

// a transfer event, it has no customer, so it has its own shape
#[derive(Debug, Deserialize)]
struct TransferWebhook {
    event: String,
    data: TransferWebhookData,
}

#[derive(Debug, Deserialize)]
struct TransferWebhookData {
    reference: String,
    amount: Option<i64>,
    currency: Option<String>,
}

// only the amount is read from the form, who is paying comes from the session
#[derive(Deserialize)]
pub struct DepositFormInput {
    pub amount: i64,
}

// decides what to tell paystack about a deposit event
// 200 means stop retrying, 500 means try again later
fn webhook_status_for(error: &DepositError) -> StatusCode {
    match error {
        DepositError::NotFound
        | DepositError::AmountMismatch
        | DepositError::InvalidCurrency
        | DepositError::InvalidStatus => StatusCode::OK,
        DepositError::DatabaseError(_) | DepositError::WalletNotFound | DepositError::Ledger(_) => {
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

// decides what to tell paystack about a transfer event
// a wrong amount or currency will not change on a retry, everything else might
fn transfer_webhook_status_for(error: &PayoutError) -> StatusCode {
    match error {
        PayoutError::AmountMismatch | PayoutError::InvalidCurrency => StatusCode::OK,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

// handles a charge.success event: reads it as a charge and settles the deposit
async fn handle_charge_event(state: &AppState, raw_bytes: &Bytes) -> Response {
    let payload: PaystackWebhook = match serde_json::from_slice(raw_bytes) {
        Ok(data) => data,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };

    // hand the service plain values, not http types
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

    StatusCode::OK.into_response()
}

// handles transfer.success, transfer.failed and transfer.reversed: reads it as a transfer and settles the payout
async fn handle_transfer_event(state: &AppState, raw_bytes: &Bytes) -> Response {
    let payload: TransferWebhook = match serde_json::from_slice(raw_bytes) {
        Ok(data) => data,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };

    let facts = TransferFacts {
        event: payload.event,
        reference: payload.data.reference,
        amount: payload.data.amount,
        currency: payload.data.currency,
    };

    if let Err(err) = process_transfer_webhook(&state.db, facts).await {
        eprintln!("Transfer webhook processing failed: {:?}", err);
        return transfer_webhook_status_for(&err).into_response();
    }

    StatusCode::OK.into_response()
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

    // 2. GUARDS: every event is checked the same way, before anything is read from it
    if !verify_paystack_signature(&raw_bytes, header_signature, &state.paystack.secret_key) {
        return StatusCode::UNAUTHORIZED.into_response();
    }

    // 3. which kind of event is this
    let event: WebhookEvent = match serde_json::from_slice(&raw_bytes) {
        Ok(event) => event,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };

    // 4. each kind goes to its own handler
    match event.event.as_str() {
        "charge.success" => handle_charge_event(&state, &raw_bytes).await,
        "transfer.success" | "transfer.failed" | "transfer.reversed" => {
            handle_transfer_event(&state, &raw_bytes).await
        }
        // an event we do not use, 200 so paystack stops sending it
        _ => StatusCode::OK.into_response(),
    }
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
