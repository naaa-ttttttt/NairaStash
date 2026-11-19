use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool, Postgres, Transaction};
use uuid::Uuid;

// the four kinds of settled money movement, matches the ledger_entry_type enum in the database
#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "ledger_entry_type", rename_all = "snake_case")]
pub enum LedgerEntryType {
    Deposit,
    PlanContribution,
    PlanWithdrawal,
    Payout,
}

// one row of the ledger_entries table, money is in kobo
// amount always matches the wallet movement, fee is extra information (0 when there is none)
#[derive(Debug, Clone, FromRow)]
pub struct LedgerEntry {
    pub entry_id: Uuid,
    pub user_id: Uuid,
    pub entry_type: LedgerEntryType,
    pub amount: i64,
    pub fee: i64,
    pub plan_id: Option<Uuid>,
    pub plan_name: Option<String>,
    pub remark: Option<String>,
    pub reference: String,
    pub created_at: DateTime<Utc>,
    pub wallet_balance_after: i64,
}

// what we need to write one new row, the id and the time are set when it is saved
#[derive(Debug, Clone)]
pub struct NewLedgerEntry {
    pub user_id: Uuid,
    pub entry_type: LedgerEntryType,
    pub amount: i64,
    pub fee: i64,
    pub plan_id: Option<Uuid>,
    pub remark: Option<String>,
    pub reference: String,
    pub wallet_balance_after: i64,
}

// saves one transaction on the database transaction it is given (tx)
// it is kept only if the whole database transaction is committed
pub async fn save_transaction(
    tx: &mut Transaction<'_, Postgres>,
    new_entry: &NewLedgerEntry,
) -> Result<(), sqlx::Error> {
    // a fresh id for the row
    let entry_id = Uuid::new_v4();
    sqlx::query(SAVE_TRANSACTION_SQL)
        .bind(entry_id)
        .bind(new_entry.user_id)
        .bind(new_entry.entry_type)
        .bind(new_entry.amount)
        .bind(new_entry.fee)
        .bind(new_entry.plan_id)
        .bind(&new_entry.remark)
        .bind(&new_entry.reference)
        .bind(new_entry.wallet_balance_after)
        .execute(&mut **tx)
        .await?;
    Ok(()) // no semicolon
}

// one transaction, but only if it belongs to this user (for the detail page)
pub async fn get_transaction_details(
    pool: &PgPool,
    entry_id: Uuid,
    user_id: Uuid,
) -> Result<Option<LedgerEntry>, sqlx::Error> {
    let query = sqlx::query_as::<_, LedgerEntry>(GET_TRANSACTION_SQL)
        .bind(entry_id)
        .bind(user_id);
    let found_transaction = query.fetch_optional(pool).await?;
    Ok(found_transaction) // no semicolon
}

// the latest transactions of one user, newest first (activity page and dashboard)
pub async fn list_user_transactions(
    pool: &PgPool,
    user_id: Uuid,
    limit: i64,
) -> Result<Vec<LedgerEntry>, sqlx::Error> {
    let query = sqlx::query_as::<_, LedgerEntry>(LIST_USER_TRANSACTIONS_SQL)
        .bind(user_id)
        .bind(limit);
    let transactions = query.fetch_all(pool).await?;
    Ok(transactions) // no semicolon
}

// the latest transactions of one plan, only if the plan belongs to this user
pub async fn list_plan_transactions(
    pool: &PgPool,
    plan_id: Uuid,
    user_id: Uuid,
    limit: i64,
) -> Result<Vec<LedgerEntry>, sqlx::Error> {
    let query = sqlx::query_as::<_, LedgerEntry>(LIST_PLAN_TRANSACTIONS_SQL)
        .bind(plan_id)
        .bind(user_id)
        .bind(limit);
    let transactions = query.fetch_all(pool).await?;
    Ok(transactions) // no semicolon
}

// how many contributions a plan has, each one fills one cell of the streak grid
pub async fn count_plan_contributions(pool: &PgPool, plan_id: Uuid) -> Result<i64, sqlx::Error> {
    let count = sqlx::query_scalar::<_, i64>(COUNT_PLAN_CONTRIBUTIONS_SQL)
        .bind(plan_id)
        .fetch_one(pool)
        .await?;
    Ok(count) // no semicolon
}

const SAVE_TRANSACTION_SQL: &str = "
    INSERT INTO ledger_entries
        (entry_id, user_id, entry_type, amount, fee, plan_id, remark, reference, wallet_balance_after)
    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)";

const GET_TRANSACTION_SQL: &str = "
    SELECT e.entry_id, e.user_id, e.entry_type, e.amount, e.fee, e.plan_id, p.name AS plan_name,
           e.remark, e.reference, e.wallet_balance_after, e.created_at
    FROM ledger_entries e
    LEFT JOIN savings_plans p ON p.plan_id = e.plan_id
    WHERE e.entry_id = $1 AND e.user_id = $2";

const LIST_USER_TRANSACTIONS_SQL: &str = "
    SELECT e.entry_id, e.user_id, e.entry_type, e.amount, e.fee, e.plan_id, p.name AS plan_name,
           e.remark, e.reference, e.wallet_balance_after, e.created_at
    FROM ledger_entries e
    LEFT JOIN savings_plans p ON p.plan_id = e.plan_id
    WHERE e.user_id = $1
    ORDER BY e.created_at DESC
    LIMIT $2";

const LIST_PLAN_TRANSACTIONS_SQL: &str = "
    SELECT e.entry_id, e.user_id, e.entry_type, e.amount, e.fee, e.plan_id, p.name AS plan_name,
           e.remark, e.reference, e.wallet_balance_after, e.created_at
    FROM ledger_entries e
    LEFT JOIN savings_plans p ON p.plan_id = e.plan_id
    WHERE e.plan_id = $1 AND e.user_id = $2
    ORDER BY e.created_at DESC
    LIMIT $3";

const COUNT_PLAN_CONTRIBUTIONS_SQL: &str = "
    SELECT COUNT(*) FROM ledger_entries
    WHERE plan_id = $1 AND entry_type = 'plan_contribution'";
