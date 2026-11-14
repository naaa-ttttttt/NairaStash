use sqlx::{Pool, Postgres, FromRow};
use uuid::Uuid;
use chrono::{DateTime, Utc};

#[derive(Debug, sqlx::Type, serde::Serialize, serde::Deserialize, PartialEq)]
#[sqlx(type_name = "deposit_type", rename_all = "lowercase")]
pub enum DepositType {
    AutoDebit,
    Manual,
}

#[derive(Debug, sqlx::Type, serde::Serialize, serde::Deserialize, PartialEq)]
#[sqlx(type_name = "transaction_status", rename_all = "lowercase")]
pub enum TransactionStatus {
    Successful,
    Pending,
    Failed,
}

#[derive(Debug, FromRow)]
pub struct DepositRecord {
    pub deposit_id: Uuid,
    pub user_id: Uuid,
    pub plan_id: Uuid,
    pub reference: String,
    pub amount: i64,
    pub status: TransactionStatus,
    pub created_at: DateTime<Utc>,
    pub deposit_type: DepositType,
}

// finds the email of a user, paystack needs it to start a payment
pub async fn get_user_email(
    pool: &Pool<Postgres>,
    user_id: Uuid,
) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar::<_, String>(
        r#"
        SELECT email
        FROM users
        WHERE user_id = $1
        "#
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await // no semicolon
}

pub async fn create_pending_deposits(
    pool: &Pool<Postgres>,
    user_id: Uuid,
    plan_id: Uuid,
    reference: &str,
    amount: i64,
    deposit_type: DepositType,
) -> Result<DepositRecord, sqlx::Error> {
    // the table has no default for deposit_id, so we make it here
    let deposit_id = Uuid::new_v4();

    sqlx::query_as::<_, DepositRecord>(
        r#"
        INSERT INTO deposits (deposit_id, user_id, plan_id, reference, amount, deposit_type, status)
        VALUES ($1, $2, $3, $4, $5, $6, 'pending')
        RETURNING deposit_id, user_id, plan_id, reference, amount, status, created_at, deposit_type
        "#
    )
    .bind(deposit_id)
    .bind(user_id)
    .bind(plan_id)
    .bind(reference)
    .bind(amount)
    .bind(deposit_type)
    .fetch_one(pool)
    .await // no semicolon
}

pub async fn get_deposit_by_reference(
    pool: &Pool<Postgres>,
    reference: &str,
) -> Result<Option<DepositRecord>, sqlx::Error> {
    sqlx::query_as::<_, DepositRecord>(
        r#"
        SELECT deposit_id, user_id, plan_id, reference, amount, status, created_at, deposit_type
        FROM deposits
        WHERE reference = $1
        "#
    )
    .bind(reference)
    .fetch_optional(pool)
    .await // no semicolon
}

// moves a deposit out of pending, only if it is still pending
// gives back None when someone else already settled it
pub async fn claim_pending_deposit(
    transaction: &mut sqlx::Transaction<'_, Postgres>,
    reference: &str,
    new_status: TransactionStatus,
) -> Result<Option<DepositRecord>, sqlx::Error> {
    sqlx::query_as::<_, DepositRecord>(
        r#"
        UPDATE deposits
        SET status = $1
        WHERE reference = $2 AND status = 'pending'
        RETURNING deposit_id, user_id, plan_id, reference, amount, status, created_at, deposit_type
        "#
    )
    .bind(new_status)
    .bind(reference)
    .fetch_optional(&mut **transaction)
    .await // no semicolon
}

// adds money to the wallet, gives back true only when exactly one wallet changed
pub async fn update_wallet_balance(
    transaction: &mut sqlx::Transaction<'_, Postgres>,
    user_id: Uuid,
    amount_in_kobo: i64,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        r#"
        UPDATE wallets
        SET balance = balance + $1
        WHERE user_id = $2
        "#
    )
    .bind(amount_in_kobo)
    .bind(user_id)
    .execute(&mut **transaction)
    .await?;

    let wallet_was_credited = result.rows_affected() == 1;

    Ok(wallet_was_credited) // no semicolon
}
