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
    
}
