use axum::{
    async_trait,
    extract::{Form, FromRequestParts, State},
    http::{header, request::Parts, HeaderMap, StatusCode},
    response::{IntoResponse, Redirect, Response},
};
use serde::Deserialize;
use uuid::Uuid;

use super::services::{
    authenticate_session, login_user, logout_user, signup_user, LoginError, LoginInput,
    SignupError, SignupInput, SESSION_DAYS,
};
use super::{render, LoginTemplate};
use crate::AppState;

const SESSION_COOKIE: &str = "session";

// the signup form as the browser sends it, no debug derive so a password can never be printed
#[derive(Deserialize)]
pub struct SignupForm {
    name: String,
    email: String,
    phone: String,
    password: String,
    confirm: String,
    terms: Option<String>, // a checked checkbox sends "on", an unchecked one sends nothing
}

// the login form as the browser sends it, no debug derive so a password can never be printed
#[derive(Deserialize)]
pub struct LoginForm {
    email: String,
    password: String,
}


// reads the session token out of the cookie header, nothing if there is none
fn session_token_from_headers(headers: &HeaderMap) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|raw| raw.split(';'))
        .find_map(|pair| {
            let (name, value) = pair.trim().split_once('=')?;
            if name == SESSION_COOKIE && !value.is_empty() {
                Some(value.to_string())
            } else {
                None
            }
        })
}

// the cookie that carries the session, HttpOnly so javascript can never read it
fn session_cookie(token: &str) -> String {
    let max_age = SESSION_DAYS * 24 * 60 * 60;
    format!("{SESSION_COOKIE}={token}; HttpOnly; SameSite=Lax; Path=/; Max-Age={max_age}")
}

// a cookie with no value and no lifetime, the browser deletes it
fn cleared_cookie() -> String {
    format!("{SESSION_COOKIE}=; HttpOnly; SameSite=Lax; Path=/; Max-Age=0")
}

// the guard: put AuthUser in a handler's arguments and that page needs a valid session
// no session or an expired one sends the visitor to /login, the user_id comes from the session only
pub struct AuthUser {
    pub user_id: Uuid,
}

#[async_trait]
impl FromRequestParts<AppState> for AuthUser {
    type Rejection = Redirect;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let token =
            session_token_from_headers(&parts.headers).ok_or_else(|| Redirect::to("/login"))?;

        match authenticate_session(&state.db, &token).await {
            Ok(user_id) => Ok(AuthUser { user_id }),
            Err(_) => Err(Redirect::to("/login")),
        }
    }
}

pub async fn login_handler() -> Response {
    render(LoginTemplate::new("login"), StatusCode::OK)
}

pub async fn login_submit_handler(
    State(state): State<AppState>,
    Form(form): Form<LoginForm>,
) -> Response {
    // keep what the user typed (never the password) so we can refill the form
    let identifier = form.email.clone();

    let input = LoginInput {
        email: form.email,
        password: form.password,
    };

        match login_user(&state.db, input).await {
        // set the cookie, then send the user to the dashboard
        Ok(token) => (
            [(header::SET_COOKIE, session_cookie(&token))],
            Redirect::to("/"),
        )
            .into_response(),
        Err(err) => {
            let (status, message) = login_error_status_and_message(&err);

            let page = LoginTemplate {
                active: "login",
                error: Some(message.to_string()),
                identifier,
                name: String::new(),
                email: String::new(),
                phone: String::new(),
            };

            render(page, status)
        }
    }
}

// deletes the session and the cookie, safe to call when nobody is logged in
pub async fn logout_handler(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if let Some(token) = session_token_from_headers(&headers) {
        if let Err(err) = logout_user(&state.db, &token).await {
            eprintln!("logout failed: {:?}", err);
        }
    }

    (
        [(header::SET_COOKIE, cleared_cookie())],
        Redirect::to("/login"),
    )
        .into_response()
}

pub async fn signup_handler() -> Response {
    render(LoginTemplate::new("signup"), StatusCode::OK)
}

pub async fn signup_submit_handler(
    State(state): State<AppState>,
    Form(form): Form<SignupForm>,
) -> Response {
    // keep what the user typed (never the password) so we can refill the form
    let name = form.name.clone();
    let email = form.email.clone();
    let phone = form.phone.clone();

    let input = SignupInput {
        name: form.name,
        email: form.email,
        phone: form.phone,
        password: form.password,
        confirm_password: form.confirm,
        terms_accepted: form.terms.is_some(),
    };

    match signup_user(&state.db, input).await {
        Ok(()) => Redirect::to("/login").into_response(), // 303 see other
        Err(err) => {
            let (status, message) = error_status_and_message(&err);

            let page = LoginTemplate {
                active: "signup",
                error: Some(message.to_string()),
                identifier: String::new(),
                name,
                email,
                phone,
            };

            render(page, status)
        }
    }
}

// picks the status code and the message the user sees for each signup error
fn error_status_and_message(err: &SignupError) -> (StatusCode, &'static str) {
    match err {
        SignupError::InvalidName => (
            StatusCode::BAD_REQUEST,
            "Please enter your full legal name.",
        ),
        SignupError::InvalidEmail => (
            StatusCode::BAD_REQUEST,
            "Please enter a valid email address.",
        ),
        SignupError::InvalidPhone => (
            StatusCode::BAD_REQUEST,
            "Please enter a valid Nigerian phone number.",
        ),
        SignupError::WeakPassword => (
            StatusCode::BAD_REQUEST,
            "Password must be 8-128 characters with an uppercase letter, a lowercase letter, a number and a symbol.",
        ),
        SignupError::PasswordMismatch => (StatusCode::BAD_REQUEST, "Passwords do not match."),
        SignupError::TermsNotAccepted => (
            StatusCode::BAD_REQUEST,
            "You must accept the Terms of Service and Privacy Policy.",
        ),
        SignupError::AccountExist => (
            StatusCode::CONFLICT,
            "An account with these details already exists. Please log in.",
        ),
        SignupError::HashingFailed | SignupError::DatabaseError => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Something went wrong. Please try again later.",
        ),
    }
}

// picks the status code and the message the user sees for each login error
fn login_error_status_and_message(err: &LoginError) -> (StatusCode, &'static str) {
    match err {
        LoginError::InvalidCredentials => (
            StatusCode::UNAUTHORIZED,
            "Invalid email, phone or password.",
        ),
        LoginError::NotLoggedIn => (StatusCode::UNAUTHORIZED, "Please log in to continue."),
        LoginError::DatabaseError => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Something went wrong. Please try again later.",
        ),
    }
}
