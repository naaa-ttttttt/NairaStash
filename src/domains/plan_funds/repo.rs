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
