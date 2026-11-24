use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool, Postgres, Transaction};
use uuid::Uuid;

// every function here takes &mut tx, so the service decides where the tx begins and ends, and this file has no rules, only sql
// the exceptions are the three reads (get_payout_by_reference, list_user_payouts, list_stale_pending_references), they take the pool

// the columns of a payout, status comes as text so we do not copy the enum
const PAYOUT_COLUMNS: &str = "payout_id, user_id, bank_account_id, amount, fee, reference, \
    transfer_code, status::text AS status, failure_reason, wallet_balance_after, \
    created_at, settled_at";

// a payout as the db holds it, money is in kobo
#[derive(Debug, Clone, FromRow)]
pub struct Payout {
    pub payout_id: Uuid,
    pub user_id: Uuid,
    pub bank_account_id: Uuid,
    // what the bank receives
    pub amount: i64,
    // what the user paid on top
    pub fee: i64,
    // our own reference, also sent to paystack
    pub reference: String,
    pub transfer_code: Option<String>,
    // pending, success or failed
    pub status: String,
    pub failure_reason: Option<String>,
    // the wallet balance right after the hold
    pub wallet_balance_after: i64,
    pub created_at: DateTime<Utc>,
    pub settled_at: Option<DateTime<Utc>>,
}

// what the service gives us to save, the status, the time and the empty fields come from the db defaults
#[derive(Debug)]
pub struct NewPayout {
    pub payout_id: Uuid,
    pub user_id: Uuid,
    pub bank_account_id: Uuid,
    pub amount: i64,
    pub fee: i64,
    pub reference: String,
    pub wallet_balance_after: i64,
}

// one line of the payouts page: the payout and the bank it went to
#[derive(Debug, Clone, FromRow)]
pub struct PayoutListItem {
    pub payout_id: Uuid,
    pub amount: i64,
    pub fee: i64,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub bank_name: String,
    pub account_number: String,
}

// saves a new payout as pending and gives back the saved row
// call it in the same tx as the wallet debit, so the money is only held if the row exists
pub async fn insert_pending_payout(
    tx: &mut Transaction<'_, Postgres>,
    new_payout: &NewPayout,
) -> Result<Payout, sqlx::Error> {
    let sql = format!(
        r#"
        INSERT INTO payouts
            (payout_id, user_id, bank_account_id, amount, fee, reference, wallet_balance_after)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        RETURNING {PAYOUT_COLUMNS}
        "#
    );

    sqlx::query_as::<_, Payout>(&sql)
        .bind(new_payout.payout_id)
        .bind(new_payout.user_id)
        .bind(new_payout.bank_account_id)
        .bind(new_payout.amount)
        .bind(new_payout.fee)
        .bind(&new_payout.reference)
        .bind(new_payout.wallet_balance_after)
        .fetch_one(&mut **tx)
        .await // no semicolon
}

// stores paystack's transfer code on the payout once paystack accepted it
pub async fn attach_transfer_code(
    tx: &mut Transaction<'_, Postgres>,
    payout_id: Uuid,
    transfer_code: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE payouts
        SET transfer_code = $1
        WHERE payout_id = $2
        "#,
    )
    .bind(transfer_code)
    .bind(payout_id)
    .execute(&mut **tx)
    .await?;

    Ok(()) // no semicolon
}

// moves a payout from pending to success, only if it is still pending
// gives back None when someone else already settled it (a repeated webhook, or the job and a webhook at the same time)
pub async fn claim_payout_as_success(
    tx: &mut Transaction<'_, Postgres>,
    reference: &str,
) -> Result<Option<Payout>, sqlx::Error> {
    let sql = format!(
        r#"
        UPDATE payouts
        SET status = 'success', settled_at = now()
        WHERE reference = $1 AND status = 'pending'
        RETURNING {PAYOUT_COLUMNS}
        "#
    );

    sqlx::query_as::<_, Payout>(&sql)
        .bind(reference)
        .fetch_optional(&mut **tx)
        .await // no semicolon
}

// moves a payout from pending to failed and records why, only if it is still pending
// gives back None when someone else already settled it, so the refund can never happen twice
pub async fn claim_payout_as_failed(
    tx: &mut Transaction<'_, Postgres>,
    reference: &str,
    failure_reason: &str,
) -> Result<Option<Payout>, sqlx::Error> {
    let sql = format!(
        r#"
        UPDATE payouts
        SET status = 'failed', failure_reason = $2, settled_at = now()
        WHERE reference = $1 AND status = 'pending'
        RETURNING {PAYOUT_COLUMNS}
        "#
    );

    sqlx::query_as::<_, Payout>(&sql)
        .bind(reference)
        .bind(failure_reason)
        .fetch_optional(&mut **tx)
        .await // no semicolon
}

// finds a payout by our reference, the webhook uses it to find the payout an event is about
pub async fn get_payout_by_reference(
    pool: &PgPool,
    reference: &str,
) -> Result<Option<Payout>, sqlx::Error> {
    let sql = format!(
        r#"
        SELECT {PAYOUT_COLUMNS}
        FROM payouts
        WHERE reference = $1
        "#
    );

    sqlx::query_as::<_, Payout>(&sql)
        .bind(reference)
        .fetch_optional(pool)
        .await // no semicolon
}

// what this user has asked to send today (Lagos time), in kobo
// pending counts because that money is on its way out, failed does not because it went back
// call it after the wallet debit in the same tx, the debit locks the wallet row so two requests take turns
pub async fn sum_payouts_today(
    tx: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COALESCE(SUM(amount), 0)::BIGINT
        FROM payouts
        WHERE user_id = $1
          AND status IN ('pending', 'success')
          AND (created_at AT TIME ZONE 'Africa/Lagos')::date
              = (now() AT TIME ZONE 'Africa/Lagos')::date
        "#,
    )
    .bind(user_id)
    .fetch_one(&mut **tx)
    .await // no semicolon
}

// the latest payouts of one user with the bank each went to, newest first, for the payouts page
pub async fn list_user_payouts(
    pool: &PgPool,
    user_id: Uuid,
    limit: i64,
) -> Result<Vec<PayoutListItem>, sqlx::Error> {
    sqlx::query_as::<_, PayoutListItem>(
        r#"
        SELECT p.payout_id, p.amount, p.fee, p.status::text AS status, p.created_at,
               b.bank_name, b.account_number
        FROM payouts p
        JOIN bank_accounts b ON b.bank_account_id = p.bank_account_id
        WHERE p.user_id = $1
        ORDER BY p.created_at DESC
        LIMIT $2
        "#,
    )
    .bind(user_id)
    .bind(limit)
    .fetch_all(pool)
    .await // no semicolon
}

// the references of payouts that have been pending longer than this many minutes, oldest first
// the stuck-payout job asks paystack about each one
pub async fn list_stale_pending_references(
    pool: &PgPool,
    older_than_minutes: i32,
    limit: i64,
) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar::<_, String>(
        r#"
        SELECT reference
        FROM payouts
        WHERE status = 'pending'
          AND created_at < now() - ($1::int * interval '1 minute')
        ORDER BY created_at
        LIMIT $2
        "#,
    )
    .bind(older_than_minutes)
    .bind(limit)
    .fetch_all(pool)
    .await // no semicolon
}
