use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool, Postgres, Transaction};
use uuid::Uuid;

// every function here takes &mut tx, so the service decides where the tx begins and ends, and this file has no rules, only sql
// the one exception is pin_exists, it only reads, so it takes the pool

// one row of payout_pins
// no Debug derive on purpose, this struct holds a pin hash
#[derive(FromRow)]
pub struct PinRecord {
    pub user_id: Uuid,
    pub pin_hash: String,
    // wrong pin tries since the last correct one
    pub failed_attempts: i32,
    // none means not locked, a time in the past means the lock has ended
    pub locked_until: Option<DateTime<Utc>>,
    // the same two values for wrong passwords on the forgot-pin page
    pub reset_failed_attempts: i32,
    pub reset_locked_until: Option<DateTime<Utc>>,
}

// tells whether this user has set a pin yet, the pages use it to show set or change
pub async fn pin_exists(pool: &PgPool, user_id: Uuid) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar::<_, bool>(
        r#"
        SELECT EXISTS (SELECT 1 FROM payout_pins WHERE user_id = $1)
        "#,
    )
    .bind(user_id)
    .fetch_one(pool)
    .await // no semicolon
}

// reads this user's pin row and locks it until the tx ends
// two checks of the same pin at the same moment now wait for each other, so no wrong try is lost
// gives back None when the user has no pin
pub async fn lock_pin_row(
    tx: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
) -> Result<Option<PinRecord>, sqlx::Error> {
    sqlx::query_as::<_, PinRecord>(
        r#"
        SELECT user_id, pin_hash, failed_attempts, locked_until,
               reset_failed_attempts, reset_locked_until
        FROM payout_pins
        WHERE user_id = $1
        FOR UPDATE
        "#,
    )
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await // no semicolon
}

// saves the first pin of a user
// gives back false when the user already has one, ON CONFLICT makes two quick set requests safe
pub async fn insert_pin(
    tx: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    pin_hash: &str,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        r#"
        INSERT INTO payout_pins (user_id, pin_hash)
        VALUES ($1, $2)
        ON CONFLICT (user_id) DO NOTHING
        "#,
    )
    .bind(user_id)
    .bind(pin_hash)
    .execute(&mut **tx)
    .await?;

    Ok(result.rows_affected() == 1) // no semicolon
}

// swaps in a new pin hash and clears every lock and counter, change and reset both end here
// gives back false when the user has no pin row
pub async fn replace_pin_hash(
    tx: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    pin_hash: &str,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        r#"
        UPDATE payout_pins
        SET pin_hash = $1,
            failed_attempts = 0,
            locked_until = NULL,
            reset_failed_attempts = 0,
            reset_locked_until = NULL,
            updated_at = now()
        WHERE user_id = $2
        "#,
    )
    .bind(pin_hash)
    .bind(user_id)
    .execute(&mut **tx)
    .await?;

    Ok(result.rows_affected() == 1) // no semicolon
}

// stores the new count of wrong pin tries, and the lock end when this try caused a lock
pub async fn record_wrong_pin(
    tx: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    failed_attempts: i32,
    locked_until: Option<DateTime<Utc>>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE payout_pins
        SET failed_attempts = $1, locked_until = $2, updated_at = now()
        WHERE user_id = $3
        "#,
    )
    .bind(failed_attempts)
    .bind(locked_until)
    .bind(user_id)
    .execute(&mut **tx)
    .await?;

    Ok(()) // no semicolon
}

// a correct pin wipes the wrong tries and any ended lock
pub async fn clear_wrong_pins(
    tx: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE payout_pins
        SET failed_attempts = 0, locked_until = NULL, updated_at = now()
        WHERE user_id = $1
        "#,
    )
    .bind(user_id)
    .execute(&mut **tx)
    .await?;

    Ok(()) // no semicolon
}

// same as record_wrong_pin, but for wrong passwords on the forgot-pin page
pub async fn record_wrong_reset(
    tx: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    reset_failed_attempts: i32,
    reset_locked_until: Option<DateTime<Utc>>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE payout_pins
        SET reset_failed_attempts = $1, reset_locked_until = $2, updated_at = now()
        WHERE user_id = $3
        "#,
    )
    .bind(reset_failed_attempts)
    .bind(reset_locked_until)
    .bind(user_id)
    .execute(&mut **tx)
    .await?;

    Ok(()) // no semicolon
}

// a correct password on the forgot-pin page wipes its wrong tries and any ended lock
pub async fn clear_wrong_resets(
    tx: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE payout_pins
        SET reset_failed_attempts = 0, reset_locked_until = NULL, updated_at = now()
        WHERE user_id = $1
        "#,
    )
    .bind(user_id)
    .execute(&mut **tx)
    .await?;

    Ok(()) // no semicolon
}


// gives back the stored password hash of one user, none when the user does not exist
// the pin reset uses it, so the password is checked the same way as at login
pub async fn find_password_hash(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar::<_, String>(
        r#"
        SELECT hashed_password FROM users WHERE user_id = $1
        "#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await // no semicolon
}
