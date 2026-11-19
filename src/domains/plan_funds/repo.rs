use chrono::{DateTime, NaiveDate, Utc};
use sqlx::{FromRow, Postgres, Transaction};
use uuid::Uuid;


// every function here takes &mut tx, so the service decides where the tx begins and ends, money is in kobo, and this file has no rules, only sql

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
