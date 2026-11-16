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

// trim the name and make sure it is not empty or too long
fn clean_name(name: &str) -> Result<String, PlanError> {
    let trimmed_name = name.trim();
    if trimmed_name.is_empty() {
        return Err(PlanError::EmptyName);
    }
    if trimmed_name.chars().count() > MAX_NAME_LENGTH {
        return Err(PlanError::NameTooLong);
    }
    let cleaned_name = trimmed_name.to_string();
    Ok(cleaned_name) // no semicolon
}

const MAX_NAME_LENGTH: usize = 60;

// the amount per frequency must be more than zero
fn check_amount(amount: i64) -> Result<(), PlanError> {
    if amount <= 0 {
        return Err(PlanError::InvalidAmount);
    }
    Ok(()) // no semicolon
}

// the target is optional, but if it is there it must be more than zero
fn check_target(target: Option<i64>) -> Result<(), PlanError> {
    match target {
        Some(value) if value <= 0 => Err(PlanError::InvalidTarget),
        _ => Ok(()), // no semicolon
    }
}
