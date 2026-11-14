use argon2::password_hash::{rand_core::OsRng, PasswordHash, SaltString};
use argon2::{Argon2, PasswordHasher, PasswordVerifier};
use chrono::{Duration, Utc};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

use super::repo::{
    create_session, create_user, create_wallet, delete_session, find_active_session_user,
    find_user_by_email, NewSession, NewUser, NewWallet,
};

const PASSWORD_MIN_LENGTH: usize = 8;
const PASSWORD_MAX_LENGTH: usize = 128;
const NAME_MAX_LENGTH: usize = 100;

// how long a login lasts before the user has to log in again
pub const SESSION_DAYS: i64 = 7;

// the raw values from the signup form, no debug derive so a password can never be printed
pub struct SignupInput {
    pub name: String,
    pub email: String,
    pub phone: String,
    pub password: String,
    pub confirm_password: String,
    pub terms_accepted: bool,
}

#[derive(Debug)]
pub enum SignupError {
    InvalidName,
    InvalidEmail,
    InvalidPhone,
    WeakPassword,
    PasswordMismatch,
    TermsNotAccepted,
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
    let starts_correctly = matches!(
        national.as_bytes().first(),
        Some(b'7') | Some(b'8') | Some(b'9')
    );

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

    if !input.terms_accepted {
        return Err(SignupError::TermsNotAccepted);
    }

    if input.password != input.confirm_password {
        return Err(SignupError::PasswordMismatch);
    }

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

// ---------------------------------------------------------------- login

// the raw values from the login form, no debug derive so a password can never be printed
pub struct LoginInput {
    pub email: String,
    pub password: String,
}

#[derive(Debug)]
pub enum LoginError {
    InvalidCredentials,
    NotLoggedIn,
    DatabaseError,
}

// logs the real database error, then gives back our own error
fn login_database_error(err: sqlx::Error) -> LoginError {
    eprintln!("Login database error: {:?}", err);
    LoginError::DatabaseError // no semicolon
}


fn generate_token() -> String {
    format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

// the database only ever sees this hash, never the token the browser holds
fn hash_token(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

// compares the typed password to the stored argon2 hash
fn password_matches(password: &str, stored_hash: &str) -> bool {
    match PasswordHash::new(stored_hash) {
        Ok(parsed) => Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok(),
        Err(_) => false,
    }
}

// checks who the user is, then opens a session and gives back the raw token for the cookie
pub async fn login_user(pool: &PgPool, input: LoginInput) -> Result<String, LoginError> {
    // a badly shaped email cannot belong to anyone, so it gets the same error
    let email = clean_email(&input.email).map_err(|_| LoginError::InvalidCredentials)?;

    // an empty or huge password is never worth hashing
    if input.password.is_empty() || input.password.chars().count() > PASSWORD_MAX_LENGTH {
        return Err(LoginError::InvalidCredentials);
    }

    // no user and wrong password give the same error, so nobody can probe which accounts exist
    let user = find_user_by_email(pool, &email)
        .await
        .map_err(login_database_error)?
        .ok_or(LoginError::InvalidCredentials)?;

    if !password_matches(&input.password, &user.hashed_password) {
        return Err(LoginError::InvalidCredentials);
    }

    let token = generate_token();

    let new_session = NewSession {
        session_id: Uuid::new_v4(),
        user_id: user.user_id,
        token_hash: hash_token(&token),
        expires_at: Utc::now() + Duration::days(SESSION_DAYS),
    };

    create_session(pool, new_session)
        .await
        .map_err(login_database_error)?;

    Ok(token) // no semicolon
}

// ends the session for this token
pub async fn logout_user(pool: &PgPool, token: &str) -> Result<(), LoginError> {
    delete_session(pool, &hash_token(token))
        .await
        .map_err(login_database_error)?;

    Ok(()) // no semicolon
}

// turns a cookie token into the user it belongs to, this is the guard for protected pages
pub async fn authenticate_session(pool: &PgPool, token: &str) -> Result<Uuid, LoginError> {
    find_active_session_user(pool, &hash_token(token))
        .await
        .map_err(login_database_error)?
        .ok_or(LoginError::NotLoggedIn)
}
