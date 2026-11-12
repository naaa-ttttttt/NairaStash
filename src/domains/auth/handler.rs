use askama::Template;
use axum::{
    extract::{Form, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
};
use serde::Deserialize;

use crate::AppState;
use super::services::{signup_user, SignupError, SignupInput};

pub struct SignupForm {
    name: String, 
    email: String, 
    phone: String, 
    password: String,
}


