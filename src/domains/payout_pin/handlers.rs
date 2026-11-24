use askama::Template;
use axum::{
    extract::{Form, Query, State},
    http::StatusCode,
    response::{IntoResponse, Redirect, Response},
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use uuid::Uuid;

use super::services::{self, PinError, PIN_LOCK_MINUTES};
use crate::domains::auth::handlers::AuthUser;
use crate::shared::render::render_page;
use crate::AppState;

// ---------------------------------------------------------------- what the pages receive

// the pin page, it shows the set form when the user has no pin and the change form when they do
#[derive(Template)]
#[template(path = "pin.html")]
pub struct PinTemplate {
    pub active: &'static str,
    pub has_pin: bool,
    pub error: Option<String>,
    pub success: Option<String>,
}

// the forgot pin page
#[derive(Template)]
#[template(path = "forgot_pin.html")]
pub struct ForgotPinTemplate {
    pub active: &'static str,
    pub error: Option<String>,
}

// the forms as the browser sends them, no Debug derive and nothing is ever shown again,
// so a pin or a password can never be printed or sent back to the page
#[derive(Deserialize)]
pub struct SetPinForm {
    pin: String,
    confirm_pin: String,
}

#[derive(Deserialize)]
pub struct ChangePinForm {
    current_pin: String,
    new_pin: String,
    confirm_pin: String,
}

#[derive(Deserialize)]
pub struct ForgotPinForm {
    password: String,
    new_pin: String,
    confirm_pin: String,
}

// the short code the pin page can get in its url after a success
#[derive(Deserialize)]
pub struct PinQuery {
    done: Option<String>,
}

// ---------------------------------------------------------------- helpers

// the text when the two typed pins are not the same
const PINS_DO_NOT_MATCH: &str = "The two PINs are not the same.";

// whole minutes until a lock ends, never less than 1
fn minutes_left(until: DateTime<Utc>) -> i64 {
    let seconds = (until - Utc::now()).num_seconds();
    ((seconds + 59) / 60).max(1) // no semicolon
}

// the text for a success code in the url, an unknown code shows nothing
fn done_message(code: &str) -> Option<String> {
    let text = match code {
        "set" => "Your PIN is set.",
        "changed" => "Your PIN was changed.",
        "reset" => "Your PIN was reset.",
        _ => return None,
    };

    Some(text.to_string()) // no semicolon
}

// picks the status code and the message the user sees for each pin error
// the errors the user cannot fix are logged here and shown as one generic message
fn pin_error_message(err: &PinError) -> (StatusCode, String) {
    match err {
        PinError::InvalidPinFormat => (
            StatusCode::BAD_REQUEST,
            "A PIN must be exactly 4 digits.".to_string(),
        ),
        PinError::PinAlreadySet => (
            StatusCode::CONFLICT,
            "You already have a PIN. Use change PIN instead.".to_string(),
        ),
        PinError::PinNotSet => (
            StatusCode::BAD_REQUEST,
            "You have not set a PIN yet.".to_string(),
        ),
        PinError::WrongPin { tries_left } => (
            StatusCode::BAD_REQUEST,
            format!(
                "That PIN is wrong. {} tries left before it locks for {} minutes.",
                tries_left, PIN_LOCK_MINUTES
            ),
        ),
        PinError::PinLocked { until } => (
            StatusCode::LOCKED,
            format!(
                "Too many wrong tries. Your PIN is locked, try again in about {} minutes, or use Forgot PIN.",
                minutes_left(*until)
            ),
        ),
        PinError::WrongPassword { tries_left } => (
            StatusCode::BAD_REQUEST,
            format!("That password is wrong. {} tries left.", tries_left),
        ),
        PinError::ResetLocked { until } => (
            StatusCode::LOCKED,
            format!(
                "Too many wrong passwords. Try again in about {} minutes.",
                minutes_left(*until)
            ),
        ),
        PinError::UserNotFound => {
            eprintln!("Payout pin: logged in user has no row in users");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Something went wrong. Please try again later.".to_string(),
            )
        }
        PinError::Hashing(text) => {
            eprintln!("Payout pin hashing error: {}", text);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Something went wrong. Please try again later.".to_string(),
            )
        }
        PinError::Database(e) => {
            eprintln!("Payout pin database error: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Something went wrong. Please try again later.".to_string(),
            )
        }
    } // no semicolon
}

// the pin page again with a message above the form
// whether it shows set or change is looked up again, if that read fails it shows set
async fn pin_page_with_error(
    state: &AppState,
    user_id: Uuid,
    status: StatusCode,
    message: String,
) -> Response {
    let has_pin = services::pin_is_set(&state.db, user_id)
        .await
        .unwrap_or(false);

    let page = PinTemplate {
        active: "none",
        has_pin,
        error: Some(message),
        success: None,
    };
    render_page(page, status) // no semicolon
}

// the forgot pin page again with a message above the form
fn forgot_page_with_error(status: StatusCode, message: String) -> Response {
    let page = ForgotPinTemplate {
        active: "none",
        error: Some(message),
    };
    render_page(page, status) // no semicolon
}

// ---------------------------------------------------------------- handlers

// GET /pin: the set form or the change form, the url may carry a success code
pub async fn pin_page_handler(
    auth: AuthUser,
    State(state): State<AppState>,
    Query(query): Query<PinQuery>,
) -> Response {
    match services::pin_is_set(&state.db, auth.user_id).await {
        Ok(has_pin) => {
            let page = PinTemplate {
                active: "none",
                has_pin,
                error: None,
                success: query.done.as_deref().and_then(done_message),
            };
            render_page(page, StatusCode::OK)
        }
        Err(err) => pin_error_message(&err).0.into_response(),
    }
}

// POST /pin/set: saves the first pin, then redirects so a refresh cannot resubmit
pub async fn set_pin_handler(
    auth: AuthUser,
    State(state): State<AppState>,
    Form(form): Form<SetPinForm>,
) -> Response {
    if form.pin != form.confirm_pin {
        return pin_page_with_error(
            &state,
            auth.user_id,
            StatusCode::BAD_REQUEST,
            PINS_DO_NOT_MATCH.to_string(),
        )
        .await;
    }

    match services::set_pin(&state.db, auth.user_id, &form.pin).await {
        Ok(()) => Redirect::to("/pin?done=set").into_response(), // 303 see other
        Err(err) => {
            let (status, message) = pin_error_message(&err);
            pin_page_with_error(&state, auth.user_id, status, message).await
        }
    }
}

// POST /pin/change: needs the current pin, a wrong one counts toward the lock
pub async fn change_pin_handler(
    auth: AuthUser,
    State(state): State<AppState>,
    Form(form): Form<ChangePinForm>,
) -> Response {
    if form.new_pin != form.confirm_pin {
        return pin_page_with_error(
            &state,
            auth.user_id,
            StatusCode::BAD_REQUEST,
            PINS_DO_NOT_MATCH.to_string(),
        )
        .await;
    }

    match services::change_pin(&state.db, auth.user_id, &form.current_pin, &form.new_pin).await {
        Ok(()) => Redirect::to("/pin?done=changed").into_response(),
        Err(err) => {
            let (status, message) = pin_error_message(&err);
            pin_page_with_error(&state, auth.user_id, status, message).await
        }
    }
}

// GET /pin/forgot: the forgot pin form
pub async fn forgot_pin_page_handler(_auth: AuthUser) -> Response {
    let page = ForgotPinTemplate {
        active: "none",
        error: None,
    };
    render_page(page, StatusCode::OK)
}

// POST /pin/forgot: the login password proves who the user is, then the new pin is saved
pub async fn forgot_pin_handler(
    auth: AuthUser,
    State(state): State<AppState>,
    Form(form): Form<ForgotPinForm>,
) -> Response {
    if form.new_pin != form.confirm_pin {
        return forgot_page_with_error(StatusCode::BAD_REQUEST, PINS_DO_NOT_MATCH.to_string());
    }

    match services::reset_pin_with_password(
        &state.db,
        auth.user_id,
        &form.password,
        &form.new_pin,
    )
    .await
    {
        Ok(()) => Redirect::to("/pin?done=reset").into_response(),
        Err(err) => {
            let (status, message) = pin_error_message(&err);
            forgot_page_with_error(status, message)
        }
    }
}
