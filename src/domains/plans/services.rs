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

// flexible plans never keep an end date, locked plans need one in the future
fn check_end_date(lock_mode: LockMode, end_date: Option<NaiveDate>) -> Result<Option<NaiveDate>, PlanError> {
    match lock_mode {
        LockMode::Flexible => Ok(None),
        LockMode::Locked => {
            let date = end_date.ok_or(PlanError::MissingEndDate)?;
            let today = Utc::now().date_naive();
            if date <= today {
                return Err(PlanError::EndDateNotInFuture);
            }
            Ok(Some(date)) // no semicolon
        }
    }
}

// check everything, then save the plan through the repo
pub async fn create_plan(pool: &PgPool, input: CreatePlanInput) -> Result<SavingsPlan, PlanError> {
    let name = clean_name(&input.name)?;
    check_amount(input.amount_per_frequency)?;
    check_target(input.target_amount)?;
    let end_date = check_end_date(input.lock_mode, input.end_date)?;
    let new_plan = NewPlan {
        user_id: input.user_id,
        name,
        frequency: input.frequency,
        amount_per_frequency: input.amount_per_frequency,
        target_amount: input.target_amount,
        lock_mode: input.lock_mode,
        end_date,
    };
    let created_plan = repo::create_plan(pool, &new_plan).await?;
    Ok(created_plan) // no semicolon
}

// all plans of the user, for the plans page
pub async fn list_plans(pool: &PgPool, user_id: Uuid) -> Result<Vec<SavingsPlan>, PlanError> {
    let plans = repo::list_plans_for_user(pool, user_id).await?;
    Ok(plans) // no semicolon
}

// only active plans, for the dashboard
pub async fn list_active_plans(pool: &PgPool, user_id: Uuid) -> Result<Vec<SavingsPlan>, PlanError> {
    let active_plans = repo::list_active_plans_for_user(pool, user_id).await?;
    Ok(active_plans) // no semicolon
}


// one plan of the user, or NotFound if it is missing or not theirs
pub async fn get_plan(pool: &PgPool, plan_id: Uuid, user_id: Uuid) -> Result<SavingsPlan, PlanError> {
    let found_plan = repo::find_plan_for_user(pool, plan_id, user_id).await?;
    let plan = found_plan.ok_or(PlanError::NotFound)?;
    Ok(plan) // no semicolon
}
