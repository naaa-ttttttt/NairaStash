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

pub fn normalise_phone(phone: &str) -> Result<String, SignupError> {
    let cleaned_phone = phone
        .trim()
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .collect();
    
        // 2. strip the prefix (+234, 234, or 0) to get the national number
    let national = if let Some(rest) = cleaned.strip_prefix("+234") {
        rest
    } else if let Some(rest) = cleaned.strip_prefix("234") {
        rest
    } else if let Some(rest) = cleaned.strip_prefix('0') {
        rest
    } else {
        return Err(SignupError::InvalidPhone);
    };

    // 3. validate: exactly 10 digits, starting with 7, 8 or 9
    let valid = national.len() == 10
        && national.chars().all(|c| c.is_ascii_digit())
        && matches!(national.as_bytes()[0], b'7' | b'8' | b'9');

    if !valid {
        return Err(SignupError::InvalidPhone);
    }

    // 4. return the E.164 format
    Ok(format!("+234{}", national))
}
