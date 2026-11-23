use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool, Postgres, Transaction};
use uuid::Uuid;

// every function here takes &mut tx, so the service decides where the tx begins and ends, and this file has no rules, only sql
// the one exception is list_active_bank_accounts, it only reads, so it takes the pool

// the columns of a saved bank, status comes as text so we do not copy the enum
const BANK_ACCOUNT_COLUMNS: &str = "bank_account_id, user_id, bank_name, bank_code, \
    account_number, account_name, recipient_code, status::text AS status, \
    created_at, removed_at";

// a saved bank as the db holds it
#[derive(Debug, FromRow)]
pub struct BankAccount {
    pub bank_account_id: Uuid,
    pub user_id: Uuid,
    pub bank_name: String,
    pub bank_code: String,
    pub account_number: String,
    pub account_name: String,
    pub recipient_code: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub removed_at: Option<DateTime<Utc>>,
}

// what the service gives us to save, after paystack resolved the name and made the recipient
// the service makes the id, status and created_at come from the db defaults
#[derive(Debug)]
pub struct NewBankAccount {
    pub bank_account_id: Uuid,
    pub user_id: Uuid,
    pub bank_name: String,
    pub bank_code: String,
    pub account_number: String,
    pub account_name: String,
    pub recipient_code: String,
}

// locks this user's row until the tx ends, so two save-bank requests of the same user wait for each other
// without this, two requests can both count 4 banks and both insert, going over the limit
// FOR NO KEY UPDATE blocks other bank changes but not inserts that only point at the user
// gives back false when the user does not exist
pub async fn lock_user_for_bank_change(
    tx: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let locked = sqlx::query_scalar::<_, Uuid>(
        r#"
        SELECT user_id
        FROM users
        WHERE user_id = $1
        FOR NO KEY UPDATE
        "#,
    )
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await?;

    Ok(locked.is_some()) // no semicolon
}

// how many active saved banks this user has, the service compares it with the limit
// call it after lock_user_for_bank_change in the same tx
pub async fn count_active_bank_accounts(
    tx: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
        FROM bank_accounts
        WHERE user_id = $1 AND status = 'active'
        "#,
    )
    .bind(user_id)
    .fetch_one(&mut **tx)
    .await // no semicolon
}

// saves a bank and gives back the saved row
// a duplicate active account fails on the unique index, the service turns that error into a clear message
pub async fn insert_bank_account(
    tx: &mut Transaction<'_, Postgres>,
    new_bank: &NewBankAccount,
) -> Result<BankAccount, sqlx::Error> {
    let sql = format!(
        r#"
        INSERT INTO bank_accounts
            (bank_account_id, user_id, bank_name, bank_code,
             account_number, account_name, recipient_code)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        RETURNING {BANK_ACCOUNT_COLUMNS}
        "#
    );

    sqlx::query_as::<_, BankAccount>(&sql)
        .bind(new_bank.bank_account_id)
        .bind(new_bank.user_id)
        .bind(&new_bank.bank_name)
        .bind(&new_bank.bank_code)
        .bind(&new_bank.account_number)
        .bind(&new_bank.account_name)
        .bind(&new_bank.recipient_code)
        .fetch_one(&mut **tx)
        .await // no semicolon
}

// all active saved banks of this user, oldest first, for the banks page and the payout picker
pub async fn list_active_bank_accounts(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Vec<BankAccount>, sqlx::Error> {
    let sql = format!(
        r#"
        SELECT {BANK_ACCOUNT_COLUMNS}
        FROM bank_accounts
        WHERE user_id = $1 AND status = 'active'
        ORDER BY created_at
        "#
    );

    sqlx::query_as::<_, BankAccount>(&sql)
        .bind(user_id)
        .fetch_all(pool)
        .await // no semicolon
}

// one active saved bank of this user, payouts use it to load the bank the user picked
// gives back None when it does not exist, was removed, or belongs to someone else
pub async fn find_active_bank_account(
    tx: &mut Transaction<'_, Postgres>,
    bank_account_id: Uuid,
    user_id: Uuid,
) -> Result<Option<BankAccount>, sqlx::Error> {
    let sql = format!(
        r#"
        SELECT {BANK_ACCOUNT_COLUMNS}
        FROM bank_accounts
        WHERE bank_account_id = $1 AND user_id = $2 AND status = 'active'
        "#
    );

    sqlx::query_as::<_, BankAccount>(&sql)
        .bind(bank_account_id)
        .bind(user_id)
        .fetch_optional(&mut **tx)
        .await // no semicolon
}

// soft delete: marks the bank removed and stamps the time, the row stays so payout history keeps its link
// gives back false when nothing changed (not found, already removed, or someone else's)
pub async fn mark_bank_account_removed(
    tx: &mut Transaction<'_, Postgres>,
    bank_account_id: Uuid,
    user_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        r#"
        UPDATE bank_accounts
        SET status = 'removed', removed_at = now()
        WHERE bank_account_id = $1 AND user_id = $2 AND status = 'active'
        "#,
    )
    .bind(bank_account_id)
    .bind(user_id)
    .execute(&mut **tx)
    .await?;

    Ok(result.rows_affected() == 1) // no semicolon
}
