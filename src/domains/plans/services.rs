use chrono::{NaiveDate, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use super::repo::{self, LockMode, NewPlan, PlanFrequency, SavingsPlan};

// everything that can go wrong in the plans feature
#[derive(Debug)]
pub enum PlanError {
    EmptyName,
    NameTooLong,
    InvalidAmount,
    InvalidTarget,
    MissingEndDate,
    EndDateNotInFuture,
    NotFound,
    Database(sqlx::Error),
}

// lets us use ? on repo calls, a sqlx error becomes PlanError::Database
impl From<sqlx::Error> for PlanError {
    fn from(error: sqlx::Error) -> Self {
        PlanError::Database(error) // no semicolon
    }
}


// what the handler gives the service, money is in kobo
pub struct CreatePlanInput {
    pub user_id: Uuid,
    pub name: String,
    pub frequency: PlanFrequency,
    pub amount_per_frequency: i64,
    pub target_amount: Option<i64>,
    pub lock_mode: LockMode,
    pub end_date: Option<NaiveDate>,
}
