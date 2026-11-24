use askama::Template;
use axum::{
    extract::{Form, Query, State},
    http::StatusCode,
    response::{IntoResponse, Redirect, Response},
};
use chrono::Utc;
use serde::Deserialize;
use uuid::Uuid;

use super::repo::PayoutListItem;
use super::services::{
    self, PayoutError, PayoutStart, DAILY_PAYOUT_LIMIT_NAIRA, MIN_PAYOUT_NAIRA,
};
use crate::domains::auth::handlers::AuthUser;
use crate::domains::bank_accounts::repo as bank_repo;
use crate::domains::payout_pin::services::pin_is_set;
use crate::shared::money::{format_kobo_as_naira, parse_naira_to_kobo};
use crate::shared::render::render_page;
use crate::AppState;

// handlers only move data between the browser and the service, the rules live in the service
// the user_id always comes from the session (AuthUser), never from a form field

// ---------------------------------------------------------------- what the pages receive

// one saved bank in the dropdown
pub struct BankChoice {
    pub bank_account_id: String,
    pub bank_name: String,
    pub masked_number: String,
    pub account_name: String,
}

// one payout in the recent list, already formatted for display
pub struct PayoutRowView {
    pub bank_name: String,
    pub masked_number: String,
    pub amount: String,
    pub fee: String,
    // pending, success or failed, the page uses it to pick the colour
    pub status_key: &'static str,
    pub status_label: &'static str,
    pub date: String,
}

// the payouts page: the form to start a payout, and the recent payouts
#[derive(Template)]
#[template(path = "payouts.html")]
pub struct PayoutsTemplate {
    pub active: &'static str,
    // false shows "set a PIN first" instead of the form
    pub has_pin: bool,
    pub banks: Vec<BankChoice>,
    pub recent: Vec<PayoutRowView>,
    pub message: Option<String>,
    pub message_is_error: bool,
    // what the user picked and typed, so the form can be filled again after an error
    pub selected_bank_id: String,
    pub amount: String,
    // the limits as text, they come from the service constants
    pub min_amount: String,
    pub daily_limit: String,
}

// the review page: the summary and the pin box
#[derive(Template)]
#[template(path = "payout_review.html")]
pub struct PayoutReviewTemplate {
    pub active: &'static str,
    pub error: Option<String>,
    pub bank_account_id: String,
    pub bank_name: String,
    pub account_name: String,
    pub account_number: String,
    // the amount as the user typed it, it rides along in a hidden field
    pub amount_text: String,
    pub amount: String,
    pub fee: String,
    pub total: String,
}

// the form of the first step
#[derive(Deserialize)]
pub struct ReviewForm {
    bank_account_id: String,
    amount: String,
}

// the form of the second step
// no Debug derive on purpose, this struct holds a pin
#[derive(Deserialize)]
pub struct PayoutForm {
    bank_account_id: String,
    amount: String,
    pin: String,
}

// the short code the payouts page can get in its url after an action
#[derive(Deserialize)]
pub struct PayoutsQuery {
    notice: Option<String>,
}

// ---------------------------------------------------------------- helpers

// hides all but the last 4 digits of an account number
fn mask_account_number(number: &str) -> String {
    let last_four = number.get(number.len().saturating_sub(4)..).unwrap_or("");
    format!("******{last_four}") // no semicolon
}

// turns a payout line into display strings
fn payout_row_view(item: &PayoutListItem) -> PayoutRowView {
    let (status_key, status_label) = match item.status.as_str() {
        "success" => ("success", "Sent"),
        "failed" => ("failed", "Failed, money returned"),
        _ => ("pending", "Processing"),
    };

    PayoutRowView {
        bank_name: item.bank_name.clone(),
        masked_number: mask_account_number(&item.account_number),
        amount: format_kobo_as_naira(item.amount),
        fee: format_kobo_as_naira(item.fee),
        status_key,
        status_label,
        date: item.created_at.format("%b %d, %Y").to_string(),
    } // no semicolon
}

// builds the banner text from the code in the url, an unknown code shows nothing
fn notice_message(code: &str) -> Option<(String, bool)> {
    match code {
        "started" => Some((
            "Payout started. It shows below while it is processed.".to_string(),
            false,
        )),
        "confirming" => Some((
            "We sent your payout and are waiting for confirmation. It shows as processing below. If it fails, your money comes back automatically.".to_string(),
            false,
        )),
        _ => None,
    } // no semicolon
}

// picks the status code and the message the user sees for each payout error
fn payout_error_message(err: &PayoutError) -> (StatusCode, String) {
    match err {
        PayoutError::AmountTooSmall => (
            StatusCode::BAD_REQUEST,
            format!(
                "The smallest payout is ₦{}.",
                format_kobo_as_naira(MIN_PAYOUT_NAIRA * 100)
            ),
        ),
        PayoutError::AmountTooLarge => (
            StatusCode::BAD_REQUEST,
            "That amount is too large.".to_string(),
        ),
        PayoutError::InvalidPin => (
            StatusCode::BAD_REQUEST,
            "A PIN must be exactly 4 digits.".to_string(),
        ),
        PayoutError::PinNotSet => (
            StatusCode::BAD_REQUEST,
            "Set a PIN first, it is asked before every payout.".to_string(),
        ),
        PayoutError::WrongPin { tries_left } => (
            StatusCode::BAD_REQUEST,
            format!("Wrong PIN. You have {tries_left} tries left."),
        ),
        PayoutError::PinLocked { until } => {
            // minutes left, rounded up, so the user never sees 0 minutes while still locked
            let minutes = (*until - Utc::now()).num_minutes() + 1;
            (
                StatusCode::TOO_MANY_REQUESTS,
                format!(
                    "Too many wrong tries. Try again in about {} minutes.",
                    minutes.max(1)
                ),
            )
        }
        PayoutError::BankNotFound => (
            StatusCode::BAD_REQUEST,
            "Please choose one of your saved banks.".to_string(),
        ),
        PayoutError::NotEnoughMoney => (
            StatusCode::BAD_REQUEST,
            "Your wallet does not have enough money for this payout and its fee.".to_string(),
        ),
        PayoutError::DailyLimitReached => (
            StatusCode::BAD_REQUEST,
            format!(
                "This would go over your daily payout limit of ₦{}.",
                format_kobo_as_naira(DAILY_PAYOUT_LIMIT_NAIRA * 100)
            ),
        ),
        PayoutError::TransferRefused => (
            StatusCode::SERVICE_UNAVAILABLE,
            "We could not send this payout right now. Your money was returned to your wallet. Please try again later.".to_string(),
        ),
        other => {
            eprintln!("Payout error: {:?}", other);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Something went wrong. Please try again later.".to_string(),
            )
        }
    }
}

// the payouts page, with an optional banner (text and whether it is an error) and the typed values
async fn payouts_page(
    state: &AppState,
    user_id: Uuid,
    message: Option<(String, bool)>,
    status: StatusCode,
    selected_bank_id: String,
    amount: String,
) -> Response {
    let has_pin = match pin_is_set(&state.db, user_id).await {
        Ok(is_set) => is_set,
        Err(err) => {
            eprintln!("Payouts page: pin check failed: {:?}", err);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let banks = match bank_repo::list_active_bank_accounts(&state.db, user_id).await {
        Ok(banks) => banks,
        Err(err) => {
            eprintln!("Payouts page: banks failed: {:?}", err);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let recent = match services::list_recent_payouts(&state.db, user_id).await {
        Ok(recent) => recent,
        Err(err) => {
            eprintln!("Payouts page: recent payouts failed: {:?}", err);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let (message, message_is_error) = match message {
        Some((text, is_error)) => (Some(text), is_error),
        None => (None, false),
    };

    let page = PayoutsTemplate {
        active: "profile",
        has_pin,
        banks: banks
            .iter()
            .map(|bank| BankChoice {
                bank_account_id: bank.bank_account_id.to_string(),
                bank_name: bank.bank_name.clone(),
                masked_number: mask_account_number(&bank.account_number),
                account_name: bank.account_name.clone(),
            })
            .collect(),
        recent: recent.iter().map(payout_row_view).collect(),
        message,
        message_is_error,
        selected_bank_id,
        amount,
        min_amount: format_kobo_as_naira(MIN_PAYOUT_NAIRA * 100),
        daily_limit: format_kobo_as_naira(DAILY_PAYOUT_LIMIT_NAIRA * 100),
    };
    render_page(page, status) // no semicolon
}

// the review page for a payout, if the review itself fails (bank removed, amount too small) we go back to the payouts page
async fn review_response(
    state: &AppState,
    user_id: Uuid,
    bank_account_id: Uuid,
    amount_text: String,
    amount_in_kobo: i64,
    error: Option<String>,
    status: StatusCode,
) -> Response {
    match services::review_payout(&state.db, user_id, bank_account_id, amount_in_kobo).await {
        Ok(review) => {
            let page = PayoutReviewTemplate {
                active: "profile",
                error,
                bank_account_id: review.bank_account_id.to_string(),
                bank_name: review.bank_name,
                account_name: review.account_name,
                account_number: review.account_number,
                amount_text,
                amount: format_kobo_as_naira(review.amount),
                fee: format_kobo_as_naira(review.fee),
                total: format_kobo_as_naira(review.total),
            };
            render_page(page, status)
        }
        Err(err) => {
            let (status, message) = payout_error_message(&err);
            payouts_page(
                state,
                user_id,
                Some((message, true)),
                status,
                bank_account_id.to_string(),
                amount_text,
            )
            .await
        }
    }
}

// ---------------------------------------------------------------- handlers

// GET /payouts: the form to start a payout and the recent payouts
pub async fn payouts_page_handler(
    auth: AuthUser,
    State(state): State<AppState>,
    Query(query): Query<PayoutsQuery>,
) -> Response {
    let message = query.notice.as_deref().and_then(notice_message);

    payouts_page(
        &state,
        auth.user_id,
        message,
        StatusCode::OK,
        String::new(),
        String::new(),
    )
    .await // no semicolon
}

// POST /payouts/review: step 1, shows the bank, the amount and the fee, and asks for the pin
// nothing is saved here
pub async fn review_payout_handler(
    auth: AuthUser,
    State(state): State<AppState>,
    Form(form): Form<ReviewForm>,
) -> Response {
    let Some(amount_in_kobo) = parse_naira_to_kobo(&form.amount) else {
        return payouts_page(
            &state,
            auth.user_id,
            Some(("Enter a valid amount.".to_string(), true)),
            StatusCode::BAD_REQUEST,
            form.bank_account_id,
            form.amount,
        )
        .await;
    };

    let Ok(bank_account_id) = Uuid::parse_str(&form.bank_account_id) else {
        return payouts_page(
            &state,
            auth.user_id,
            Some(("Please choose one of your saved banks.".to_string(), true)),
            StatusCode::BAD_REQUEST,
            String::new(),
            form.amount,
        )
        .await;
    };

    review_response(
        &state,
        auth.user_id,
        bank_account_id,
        form.amount,
        amount_in_kobo,
        None,
        StatusCode::OK,
    )
    .await // no semicolon
}

// POST /payouts: step 2, checks the pin, holds the money and starts the transfer
// a redirect on success, so a refresh cannot send it twice
pub async fn create_payout_handler(
    auth: AuthUser,
    State(state): State<AppState>,
    Form(form): Form<PayoutForm>,
) -> Response {
    let (Some(amount_in_kobo), Ok(bank_account_id)) = (
        parse_naira_to_kobo(&form.amount),
        Uuid::parse_str(&form.bank_account_id),
    ) else {
        return payouts_page(
            &state,
            auth.user_id,
            Some(("Something was wrong with the form. Please start again.".to_string(), true)),
            StatusCode::BAD_REQUEST,
            String::new(),
            String::new(),
        )
        .await;
    };

    let result = services::request_payout(
        &state.db,
        &state.http_client,
        &state.paystack,
        auth.user_id,
        bank_account_id,
        amount_in_kobo,
        &form.pin,
    )
    .await;

    match result {
        Ok(PayoutStart::Started) => Redirect::to("/payouts?notice=started").into_response(),
        Ok(PayoutStart::Confirming) => Redirect::to("/payouts?notice=confirming").into_response(),
        Err(err) => {
            let (status, message) = payout_error_message(&err);

            // a pin problem keeps the user on the review page, so they only retype the pin
            let is_pin_problem = matches!(
                err,
                PayoutError::InvalidPin | PayoutError::WrongPin { .. } | PayoutError::PinLocked { .. }
            );

            if is_pin_problem {
                review_response(
                    &state,
                    auth.user_id,
                    bank_account_id,
                    form.amount,
                    amount_in_kobo,
                    Some(message),
                    status,
                )
                .await
            } else {
                payouts_page(
                    &state,
                    auth.user_id,
                    Some((message, true)),
                    status,
                    form.bank_account_id,
                    form.amount,
                )
                .await
            }
        }
    }
}
