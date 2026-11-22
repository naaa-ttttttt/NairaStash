use chrono::{DateTime, NaiveDate, Utc};
use sqlx::{FromRow, PgPool, Postgres, Transaction};
use uuid::Uuid;

// every function here takes &mut tx, so the service decides where the tx begins and ends, money is in kobo, and this file has no rules, only sql
// the one exception is sum_plan_money_taken_out, it only reads, so it takes the pool

// the plan row as the money code needs it, read while locked
// status and lock_mode come as text so we do not copy enums we have not seen yet
#[derive(Debug, FromRow)]
pub struct LockedPlan {
    pub plan_id: Uuid,
    pub user_id: Uuid,
    pub amount_per_frequency: i64,
    pub target_amount: i64,
    pub balance: i64,
    pub status: String,
    pub lock_mode: String,
    pub end_date: NaiveDate,
    pub created_at: DateTime<Utc>,
}

// reads one plan of this user and locks the row until the tx ends
// two add-money requests on the same plan now wait for each other
// gives back None when the plan does not exist or belongs to someone else
pub async fn lock_plan_for_update(
    tx: &mut Transaction<'_, Postgres>,
    plan_id: Uuid,
    user_id: Uuid,
) -> Result<Option<LockedPlan>, sqlx::Error> {
    sqlx::query_as::<_, LockedPlan>(
        r#"
        SELECT plan_id, user_id, amount_per_frequency, target_amount, balance,
               status::text AS status, lock_mode::text AS lock_mode,
               end_date, created_at
        FROM savings_plans
        WHERE plan_id = $1 AND user_id = $2
        FOR UPDATE
        "#,
    )
    .bind(plan_id)
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await // no semicolon
}


// takes money out of the wallet only if the wallet has enough
// gives back the new wallet balance, or None when the wallet is too low
pub async fn debit_wallet_if_enough(
    tx: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    amount_in_kobo: i64,
) -> Result<Option<i64>, sqlx::Error> {
    sqlx::query_scalar::<_, i64>(
        r#"
        UPDATE wallets
        SET balance = balance - $1
        WHERE user_id = $2 AND balance >= $1
        RETURNING balance
        "#,
    )
    .bind(amount_in_kobo)
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await // no semicolon
}


// puts money into the wallet, gives back the new balance
// None means the user has no wallet
pub async fn credit_wallet(
    tx: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    amount_in_kobo: i64,
) -> Result<Option<i64>, sqlx::Error> {
    sqlx::query_scalar::<_, i64>(
        r#"
        UPDATE wallets
        SET balance = balance + $1
        WHERE user_id = $2
        RETURNING balance
        "#,
    )
    .bind(amount_in_kobo)
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await // no semicolon
}


// adds money to the plan balance, gives back the new plan balance
// call it only after lock_plan_for_update in the same tx
pub async fn add_to_plan_balance(
    tx: &mut Transaction<'_, Postgres>,
    plan_id: Uuid,
    amount_in_kobo: i64,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar::<_, i64>(
        r#"
        UPDATE savings_plans
        SET balance = balance + $1
        WHERE plan_id = $2
        RETURNING balance
        "#,
    )
    .bind(amount_in_kobo)
    .bind(plan_id)
    .fetch_one(&mut **tx)
    .await // no semicolon
}


// takes money out of the plan balance only if the plan has enough
// gives back the new plan balance, or None when the plan is too low
pub async fn subtract_from_plan_balance(
    tx: &mut Transaction<'_, Postgres>,
    plan_id: Uuid,
    amount_in_kobo: i64,
) -> Result<Option<i64>, sqlx::Error> {
    sqlx::query_scalar::<_, i64>(
        r#"
        UPDATE savings_plans
        SET balance = balance - $1
        WHERE plan_id = $2 AND balance >= $1
        RETURNING balance
        "#,
    )
    .bind(amount_in_kobo)
    .bind(plan_id)
    .fetch_optional(&mut **tx)
    .await // no semicolon
}

// marks the plan as closed, used by liquidate and by the payout job later
pub async fn close_plan(
    tx: &mut Transaction<'_, Postgres>,
    plan_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE savings_plans
        SET status = 'closed'
        WHERE plan_id = $1
        "#,
    )
    .bind(plan_id)
    .execute(&mut **tx)
    .await?;

    Ok(()) // no semicolon
}

// counts the filled cells of a plan, one plan_contribution row is one cell
// read it inside the tx, after the lock, so the count cannot go stale
pub async fn count_plan_contributions_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    plan_id: Uuid,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
        FROM ledger_entries
        WHERE plan_id = $1 AND entry_type = 'plan_contribution'
        "#,
    )
    .bind(plan_id)
    .fetch_one(&mut **tx)
    .await // no semicolon
}

// all the money that has left a plan so far: amount + fee of every plan_withdrawal row
// the grid uses it to know how many cells are struck, it only reads, so it takes the pool
// COALESCE gives 0 when there are no withdrawals, SUM of bigint is numeric so we cast it back
pub async fn sum_plan_money_taken_out(pool: &PgPool, plan_id: Uuid) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COALESCE(SUM(amount + fee), 0)::BIGINT
        FROM ledger_entries
        WHERE plan_id = $1 AND entry_type = 'plan_withdrawal'
        "#,
    )
    .bind(plan_id)
    .fetch_one(pool)
    .await // no semicolon
}
