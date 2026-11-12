use argon2::password_hash::{rand_core::OsRng, SaltString};
use argon2::{Argon2, PasswordHasher};
use sqlx::PgPool;
use uuid::Uuid;

use super::repo::{create_user, create_wallet, NewUser, NewWallet};

const PASSWORD_MIN_LENGTH: usize = 8;
const PASSWORD_MAX_LENGTH: usize = 128;
const NAME_MAX_LENGTH: usize = 100;

// the raw values from the signup form, no debug derive so a password can never be printed
pub struct SignupInput {
    pub name: String,
    pub email: String,
    pub phone: String,
    pub password: String,
}

#[derive(Debug)]
pub enum SignupError {
    InvalidName,
    InvalidEmail,
    InvalidPhone,
    WeakPassword,
    AccountExist,
    HashingFailed,
    DatabaseError,
}

// logs the real database error, then gives back our own error
fn database_error(err: sqlx::Error) -> SignupError {
    eprintln!("Signup database error: {:?}", err);
    SignupError::DatabaseError // no semicolon
}

// trims the name and rejects empty or too long names
pub fn clean_name(name: &str) -> Result<String, SignupError> {
    let cleaned = name.trim();

    if cleaned.is_empty() || cleaned.len() > NAME_MAX_LENGTH {
        return Err(SignupError::InvalidName);
    }

    Ok(cleaned.to_string()) // no semicolon
}

// trims and lowercases the email, then checks its basic shape
pub fn clean_email(email: &str) -> Result<String, SignupError> {
    let email = email.trim().to_lowercase();

    // split at the first @, no @ means invalid
    let Some((local, domain)) = email.split_once('@') else {
        return Err(SignupError::InvalidEmail);
    };

    let bad_local = local.is_empty();
    let bad_domain =
        !domain.contains('.') || domain.starts_with('.') || domain.ends_with('.');

    if bad_local || bad_domain {
        return Err(SignupError::InvalidEmail);
    }

    Ok(email) // no semicolon
}

// turns 0803..., 234803... or +234803... into +234803...
pub fn normalise_phone(phone: &str) -> Result<String, SignupError> {
    // 1. remove spaces and dashes
    let cleaned: String = phone
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
    let has_ten_digits = national.len() == 10 && national.chars().all(|c| c.is_ascii_digit());
    let starts_correctly = matches!(national.as_bytes().first(), Some(b'7') | Some(b'8') | Some(b'9'));

    if !has_ten_digits || !starts_correctly {
        return Err(SignupError::InvalidPhone);
    }

    // 4. return the e164 format
    Ok(format!("+234{}", national)) // no semicolon
}

// checks length, upper, lower, digit and symbol rules
pub fn check_password(password: &str) -> Result<(), SignupError> {
    let length = password.chars().count();
    let long_enough = length >= PASSWORD_MIN_LENGTH;
    let short_enough = length <= PASSWORD_MAX_LENGTH;
    let has_upper = password.chars().any(|c| c.is_uppercase());
    let has_lower = password.chars().any(|c| c.is_lowercase());
    let has_digit = password.chars().any(|c| c.is_ascii_digit());
    let has_symbol = password
        .chars()
        .any(|c| !c.is_alphanumeric() && !c.is_whitespace());

    let all_rules_pass =
        long_enough && short_enough && has_upper && has_lower && has_digit && has_symbol;

    if !all_rules_pass {
        return Err(SignupError::WeakPassword);
    }

    Ok(()) // no semicolon
}

// hashes the password with argon2, the random salt is stored inside the hash
pub fn hash_password(password: &str) -> Result<String, SignupError> {
    let salt = SaltString::generate(&mut OsRng);

    let hash = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|_| SignupError::HashingFailed)?
        .to_string();

    Ok(hash) // no semicolon
}

// validates the input, then creates the user and the wallet in one transaction
pub async fn signup_user(pool: &PgPool, input: SignupInput) -> Result<(), SignupError> {
    // validate everything before touching the database
    let name = clean_name(&input.name)?;
    let email = clean_email(&input.email)?;
    let phone = normalise_phone(&input.phone)?;
    check_password(&input.password)?;
    let hashed_password = hash_password(&input.password)?;

    // the same user_id goes into both rows, that is what links them
    let user_id = Uuid::new_v4();
    let wallet_id = Uuid::new_v4();

    // open the transaction, an early return drops it and rolls everything back
    let mut tx = pool.begin().await.map_err(database_error)?;

    let new_user = NewUser {
        user_id,
        name,
        email,
        phone,
        hashed_password,
    };

    // a unique violation means the email or phone is taken, same message for both
    create_user(&mut tx, new_user).await.map_err(|err| match err {
        sqlx::Error::Database(db_err) if db_err.is_unique_violation() => {
            SignupError::AccountExist
        }
        other => database_error(other),
    })?;

    let new_wallet = NewWallet { wallet_id, user_id };

    create_wallet(&mut tx, new_wallet)
        .await
        .map_err(database_error)?;

    // only now are both rows saved for real
    tx.commit().await.map_err(database_error)?;

    Ok(()) // no semicolon
}
