use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use chrono::{DateTime, Duration, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use super::repo;
use crate::domains::auth::services::{password_matches, PASSWORD_MAX_LENGTH};

// the rules of the payout pin live here, the repo only runs sql
// the service decides where each db transaction begins and ends

// a pin is always exactly this many digits
const PIN_LENGTH: usize = 4;

// wrong tries allowed before the pin locks, this is the only place the number lives
pub const MAX_WRONG_PIN_TRIES: i32 = 5;

// how long a lock lasts, in minutes
pub const PIN_LOCK_MINUTES: i64 = 30;

// wrong passwords allowed on the forgot-pin page before it locks
pub const MAX_WRONG_RESET_TRIES: i32 = 5;

// how long the forgot-pin lock lasts, in minutes
pub const RESET_LOCK_MINUTES: i64 = 30;

// everything that can go wrong with a pin
#[derive(Debug)]
pub enum PinError {
    // not exactly 4 digits, this does not count as a wrong try
    InvalidPinFormat,
    // set_pin was called but the user already has a pin
    PinAlreadySet,
    // the user has no pin yet
    PinNotSet,
    // the current pin typed in change_pin was wrong
    WrongPin { tries_left: i32 },
    // too many wrong tries, the pin is locked until this time
    PinLocked { until: DateTime<Utc> },
    // the password typed on the forgot-pin page was wrong
    WrongPassword { tries_left: i32 },
    // too many wrong passwords on the forgot-pin page, locked until this time
    ResetLocked { until: DateTime<Utc> },
    // the logged in user has no row in users
    UserNotFound,
    // the hash could not be made or read, the text is only for our logs
    Hashing(String),
    // anything unexpected from the database
    Database(sqlx::Error),
}

// lets us use ? on repo calls, a sqlx error becomes PinError::Database
impl From<sqlx::Error> for PinError {
    fn from(err: sqlx::Error) -> Self {
        PinError::Database(err) // no semicolon
    }
}

// the answer of check_pin, payouts match on it
// wrong and locked are normal answers here, not errors, because the try count is already saved by then
#[derive(Debug)]
pub enum PinCheck {
    Correct,
    Wrong { tries_left: i32 },
    Locked { until: DateTime<Utc> },
}

// checks the pin is exactly 4 digits
fn check_pin_format(pin: &str) -> Result<(), PinError> {
    let all_digits = pin.chars().all(|character| character.is_ascii_digit());

    if pin.len() != PIN_LENGTH || !all_digits {
        return Err(PinError::InvalidPinFormat);
    }

    Ok(()) // no semicolon
}

// turns a pin into an argon2 hash with a fresh random salt
fn hash_pin(pin: &str) -> Result<String, PinError> {
    let salt = SaltString::generate(&mut OsRng);

    let hash = Argon2::default()
        .hash_password(pin.as_bytes(), &salt)
        .map_err(|err| PinError::Hashing(err.to_string()))?;

    Ok(hash.to_string()) // no semicolon
}

// compares a typed pin with a stored hash, true when they match
// this takes a few milliseconds of cpu, which is fine here
fn pin_matches(pin: &str, stored_hash: &str) -> Result<bool, PinError> {
    let parsed_hash =
        PasswordHash::new(stored_hash).map_err(|err| PinError::Hashing(err.to_string()))?;

    let matches = Argon2::default()
        .verify_password(pin.as_bytes(), &parsed_hash)
        .is_ok();

    Ok(matches) // no semicolon
}

// tells whether the user has a pin, the pages use it to show set or change
pub async fn pin_is_set(pool: &PgPool, user_id: Uuid) -> Result<bool, PinError> {
    let is_set = repo::pin_exists(pool, user_id).await?;

    Ok(is_set) // no semicolon
}

// saves the first pin, only works when the user has none yet
pub async fn set_pin(pool: &PgPool, user_id: Uuid, pin: &str) -> Result<(), PinError> {
    check_pin_format(pin)?;
    let pin_hash = hash_pin(pin)?;

    let mut tx = pool.begin().await?;

    let inserted = repo::insert_pin(&mut tx, user_id, &pin_hash).await?;
    if !inserted {
        return Err(PinError::PinAlreadySet);
    }

    tx.commit().await?;

    Ok(()) // no semicolon
}

// part 2 of every reset: saves a new pin and clears the lock and every counter
// it never asks how the user proved who they are, so change_pin and the forgot-pin flow both end here
pub async fn save_new_pin(pool: &PgPool, user_id: Uuid, new_pin: &str) -> Result<(), PinError> {
    check_pin_format(new_pin)?;
    let pin_hash = hash_pin(new_pin)?;

    let mut tx = pool.begin().await?;

    let found = repo::replace_pin_hash(&mut tx, user_id, &pin_hash).await?;
    if !found {
        return Err(PinError::PinNotSet);
    }

    tx.commit().await?;

    Ok(()) // no semicolon
}

// checks a pin, used by payouts and by change_pin
// it runs in its own transaction and commits the try count before it answers,
// so a failed payout afterwards cannot undo a wrong try
pub async fn check_pin(pool: &PgPool, user_id: Uuid, pin: &str) -> Result<PinCheck, PinError> {
    check_pin_format(pin)?;

    let mut tx = pool.begin().await?;

    // the row is locked, so two checks at the same moment take turns
    let record = repo::lock_pin_row(&mut tx, user_id)
        .await?
        .ok_or(PinError::PinNotSet)?;

    let now = Utc::now();

    // still locked: answer without touching anything, the tx rolls back when it is dropped
    if let Some(until) = record.locked_until {
        if until > now {
            return Ok(PinCheck::Locked { until });
        }
    }

    // correct pin: wipe the wrong tries and any ended lock, only write if there is something to wipe
    if pin_matches(pin, &record.pin_hash)? {
        if record.failed_attempts > 0 || record.locked_until.is_some() {
            repo::clear_wrong_pins(&mut tx, user_id).await?;
        }
        tx.commit().await?;
        return Ok(PinCheck::Correct);
    }

    // wrong pin: if an old lock has ended, the user starts again from zero tries
    let tries_before = if record.locked_until.is_some() {
        0
    } else {
        record.failed_attempts
    };
    let tries_now = tries_before + 1;

    // this was the last allowed try: lock the pin
    if tries_now >= MAX_WRONG_PIN_TRIES {
        let until = now + Duration::minutes(PIN_LOCK_MINUTES);
        repo::record_wrong_pin(&mut tx, user_id, tries_now, Some(until)).await?;
        tx.commit().await?;
        return Ok(PinCheck::Locked { until });
    }

    repo::record_wrong_pin(&mut tx, user_id, tries_now, None).await?;
    tx.commit().await?;

    Ok(PinCheck::Wrong {
        tries_left: MAX_WRONG_PIN_TRIES - tries_now,
    }) // no semicolon
}

// changes the pin, the current pin must be right first and a wrong guess counts toward the lock
pub async fn change_pin(
    pool: &PgPool,
    user_id: Uuid,
    current_pin: &str,
    new_pin: &str,
) -> Result<(), PinError> {
    // check the new pin first, so a typo in it does not touch the try count
    check_pin_format(new_pin)?;

    match check_pin(pool, user_id, current_pin).await? {
        PinCheck::Correct => {}
        PinCheck::Wrong { tries_left } => return Err(PinError::WrongPin { tries_left }),
        PinCheck::Locked { until } => return Err(PinError::PinLocked { until }),
    }

    save_new_pin(pool, user_id, new_pin).await // no semicolon
}

// forgot pin: part 1 proves who the user is with the login password, part 2 is save_new_pin
// wrong passwords have their own counter and lock, so this page cannot be used to guess a password
// when email or phone confirmation comes, only part 1 is replaced
pub async fn reset_pin_with_password(
    pool: &PgPool,
    user_id: Uuid,
    password: &str,
    new_pin: &str,
) -> Result<(), PinError> {
    // a typo in the new pin must not cost a password try
    check_pin_format(new_pin)?;

    // a plain read, so it happens before the transaction
    let stored_hash = repo::find_password_hash(pool, user_id)
        .await?
        .ok_or(PinError::UserNotFound)?;

    let mut tx = pool.begin().await?;

    // the row is locked, so two resets at the same moment take turns
    // no row means no pin, and there is nothing to reset
    let record = repo::lock_pin_row(&mut tx, user_id)
        .await?
        .ok_or(PinError::PinNotSet)?;

    let now = Utc::now();

    // still locked: answer without touching anything
    if let Some(until) = record.reset_locked_until {
        if until > now {
            return Err(PinError::ResetLocked { until });
        }
    }

    // an empty or huge password is never worth hashing, it counts as wrong like at login
    let password_is_usable =
        !password.is_empty() && password.chars().count() <= PASSWORD_MAX_LENGTH;

    if password_is_usable && password_matches(password, &stored_hash) {
        // nothing was written here, we end the tx first so its row lock is gone
        // save_new_pin opens its own tx and would wait on this lock forever
        tx.commit().await?;
        return save_new_pin(pool, user_id, new_pin).await;
    }

    // wrong password: if an old lock has ended, the user starts again from zero tries
    let tries_before = if record.reset_locked_until.is_some() {
        0
    } else {
        record.reset_failed_attempts
    };
    let tries_now = tries_before + 1;

    // this was the last allowed try: lock the page
    if tries_now >= MAX_WRONG_RESET_TRIES {
        let until = now + Duration::minutes(RESET_LOCK_MINUTES);
        repo::record_wrong_reset(&mut tx, user_id, tries_now, Some(until)).await?;
        tx.commit().await?;
        return Err(PinError::ResetLocked { until });
    }

    repo::record_wrong_reset(&mut tx, user_id, tries_now, None).await?;
    tx.commit().await?;

    Err(PinError::WrongPassword {
        tries_left: MAX_WRONG_RESET_TRIES - tries_now,
    }) // no semicolon
}
