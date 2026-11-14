use axum::{
    body::Bytes,
    extract::{Form, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::AppState;
use crate::domains::auth::handlers::AuthUser;
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

// only the amount is read from the form, who is paying comes from the session
#[derive(serde::Deserialize)]
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

// starts a deposit for the logged in user and gives back the paystack checkout link
pub async fn initialize_deposit_handler(
    auth: AuthUser,
    State(state): State<AppState>,
    Form(input): Form<DepositFormInput>,
) -> impl IntoResponse {
    // 1. INPUTS
    let user_id = auth.user_id;

    // 2. GUARDS
    if input.amount < 1 {
        return (StatusCode::BAD_REQUEST, "Amount must be at least 1 naira").into_response();
    }

    let user_email = match super::repo::get_user_email(&state.db, user_id).await {
        Ok(Some(email)) => email,
        Ok(None) => return (StatusCode::UNAUTHORIZED, "User not found").into_response(),
        Err(e) => {
            println!("DB Error loading user email: {:?}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response();
        }
    };

    // 3. TRANSFORMATIONS
    let amount_in_kobo = input.amount * 100;
    let temp_plan_id = Uuid::nil();
    let tracking_reference = format!("NS-DEP-{}", &Uuid::new_v4().to_string()[..8]);

    println!(
        "Attempting Paystack Handshake for Email: {}, Amount: {} Kobo",
        user_email, amount_in_kobo
    );

    if let Err(e) = super::repo::create_pending_deposits(
        &state.db,
        user_id,
        temp_plan_id,
        &tracking_reference,
        amount_in_kobo,
        super::repo::DepositType::Manual,
    )
    .await
    {
        println!("DB Error creating pending deposit: {:?}", e);
        return (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response();
    }

    // 4. OUTPUTS
    match super::pay_stack::initialize_payment(
        &user_email,
        amount_in_kobo,
        &tracking_reference,
        &state.paystack,
    )
    .await
    {
        Ok(checkout_url) => {
            println!("SUCCESS: Paystack Link Generated: {}", checkout_url);
            (StatusCode::OK, checkout_url).into_response()
        }
        Err(err) => {
            eprintln!("CRITICAL PAYSTACK ERROR LOG: {:?}", err);

            if let Some(status) = err.status() {
                if status == reqwest::StatusCode::UNAUTHORIZED {
                    eprintln!("WARNING: Your PAYSTACK_SECRET_KEY in your .env file is being rejected by Paystack's servers! Double check that it has no typos and starts with sk_test_");
                }
            }

            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to connect to Paystack gateway",
            )
                .into_response()
        }
    }
}
