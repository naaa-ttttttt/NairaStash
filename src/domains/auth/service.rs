use argon2::password_hash::{rand_core::OsRng, SaltString};
use argon2::{Argon2, PasswordHasher};
use sqlx::PgPool;
use uuid::Uuid;

use super::repo::{create_user, create_wallet, NewUser, NewWallet};

pub struct SignupInput {
    name: String,
    email: String,
    phone: String,
    password: String,
}

pub enum SignupError {
    InvalidName,
    InvalidEmail,
    InvalidPhone,
    WeakPassword,
    AccountExist,
}

// function that validates usernames
pub fn clean_name(name: &str) -> Result<String, SignupError> {
    let cleaned = name.trim();

    if cleaned.is_empty() || cleaned.len() > 100 {
        return Err(SignupError::InvalidName);
    }

    Ok(cleaned.to_string())
}

// function to verify email.
pub fn clean_email(email: &str) -> Result<String, SignupError> {
    let email = email.trim().to_lowercase();

    let Some((local, domain)) = email.split_once('@') else {
        return Err(SignupError::InvalidEmail);
    };

    if local.is_empty() || !domain.contains('.') || domain.starts_with('.') || domain.ends_with('.') {
        return Err(SignupError::InvalidEmail);
    }

    Ok(email)
}
