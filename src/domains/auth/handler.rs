use askama::Template;
use axum::{
    extract::{Form, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
};
use serde::Deserialize;

use crate::AppState;
use super::services::{signup_user, SignupError, SignupInput};

#[derive(Deserialize)]
pub struct SignupForm {
    name: String, 
    email: String, 
    phone: String, 
    password: String,
}

fn signup_handler(
    State(state): State<AppState>,
    Form(input): Form<SignupForm>,
) -> Response {
    let input = SignupInput {
        name: input.name,
        email: input.email,
        phone: input.phone,
        password: input.password,
    };

    // funciton that signs user up
    match signup_user(&state.pool, input).await {
        Ok(()) => Redirect::to("/login").into_response(),
        Err(err) => {
            let (status, message) = error_status_and_message(&err);
            (status, message).into_response()
        }    
    }
}
