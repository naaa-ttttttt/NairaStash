use chrono::{DateTime, Utc};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

// the values needed to write one new user row, no debug derive so the hash is never printed
pub struct NewUser {
    pub user_id: Uuid,
    pub name: String,
    pub email: String,
    pub hashed_password: String,
    pub phone: String,
}

// the values needed to write one new wallet row, balance and currency come from table defaults
pub struct NewWallet {
    pub wallet_id: Uuid,
    pub user_id: Uuid,
}

// writes one user row on the transaction it is given
pub async fn create_user(
    transaction: &mut Transaction<'_, Postgres>,
    new_user: NewUser,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO users (user_id, name, email, phone, hashed_password)
        VALUES ($1, $2, $3, $4, $5)
        "#,
    )
    .bind(new_user.user_id)
    .bind(new_user.name)
    .bind(new_user.email)
    .bind(new_user.phone)
    .bind(new_user.hashed_password)
    .execute(&mut **transaction)
    .await?;

    Ok(()) // no semicolon
}

// writes one wallet row on the transaction it is given
pub async fn create_wallet(
    transaction: &mut Transaction<'_, Postgres>,
    new_wallet: NewWallet,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO wallets (wallet_id, user_id)
        VALUES ($1, $2)
        "#,
    )
    .bind(new_wallet.wallet_id)
    .bind(new_wallet.user_id)
    .execute(&mut **transaction)
    .await?;

    Ok(()) // no semicolon
}

// what login needs to know about an account, no debug derive so the hash is never printed
#[derive(sqlx::FromRow)]
pub struct LoginUser {
    pub user_id: Uuid,
    pub hashed_password: String,
}

// finds one user by a cleaned email or a normalised phone, nothing if there is none
// finds one user by a cleaned email, nothing if there is none
pub async fn find_user_by_email(
    pool: &PgPool,
    email: &str,
) -> Result<Option<LoginUser>, sqlx::Error> {
    let user = sqlx::query_as::<_, LoginUser>(
        r#"
        SELECT user_id, hashed_password
        FROM users
        WHERE email = $1
        "#,
    )
    .bind(email)
    .fetch_optional(pool)
    .await?;

    Ok(user) // no semicolon
}

pub struct NewSession {
    pub session_id: Uuid,
    pub user_id: Uuid,
    pub token_hash: String,
    pub expires_at: DateTime<Utc>,
}

// writes one session row
pub async fn create_session(pool: &PgPool, new_session: NewSession) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO sessions (session_id, user_id, token_hash, expires_at)
        VALUES ($1, $2, $3, $4)
        "#,
    )
    .bind(new_session.session_id)
    .bind(new_session.user_id)
    .bind(new_session.token_hash)
    .bind(new_session.expires_at)
    .execute(pool)
    .await?;

    Ok(()) // no semicolon
}

// gives back the owner of a session, but only if the session has not expired
pub async fn find_active_session_user(
    pool: &PgPool,
    token_hash: &str,
) -> Result<Option<Uuid>, sqlx::Error> {
    let user_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        SELECT user_id
        FROM sessions
        WHERE token_hash = $1 AND expires_at > now()
        "#,
    )
    .bind(token_hash)
    .fetch_optional(pool)
    .await?;

    Ok(user_id) // no semicolon
}

// removes one session, deleting a session that does not exist is fine
pub async fn delete_session(pool: &PgPool, token_hash: &str) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM sessions WHERE token_hash = $1")
        .bind(token_hash)
        .execute(pool)
        .await?;

    Ok(()) // no semicolon
}
