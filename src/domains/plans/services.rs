use chrono::{Datelike, FixedOffset, Months, NaiveDate, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use super::repo::{self, LockMode, NewPlan, PlanFrequency, SavingsPlan};

const MAX_NAME_LENGTH: usize = 60;

// everything that can go wrong in the plans feature
#[derive(Debug)]
pub enum PlanError {
    EmptyName,
    NameTooLong,
    InvalidAmount,
    InvalidTarget,
    InvalidFrequency,
    InvalidLockMode,
    InvalidEndDate,
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

// the user either types the amount per period, or types a target and we work the amount out
#[derive(Debug, Clone, Copy)]
pub enum PlanAmountInput {
    PerPeriod(i64),
    FromTarget(i64),
}

// what the handler gives the service, money is in kobo
pub struct CreatePlanInput {
    pub user_id: Uuid,
    pub name: String,
    pub frequency: PlanFrequency,
    pub lock_mode: LockMode,
    pub end_date: NaiveDate,
    pub amount: PlanAmountInput,
}

// today's date in Lagos, Nigeria is UTC+1 all year with no daylight saving
pub fn today_in_lagos() -> NaiveDate {
    match FixedOffset::east_opt(60 * 60) {
        Some(lagos) => Utc::now().with_timezone(&lagos).date_naive(),
        None => Utc::now().date_naive(),
    }
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

// the amount per period must be more than zero
fn check_amount(amount: i64) -> Result<(), PlanError> {
    if amount <= 0 {
        return Err(PlanError::InvalidAmount);
    }
    Ok(()) // no semicolon
}

// the target must be more than zero
fn check_target(target: i64) -> Result<(), PlanError> {
    if target <= 0 {
        return Err(PlanError::InvalidTarget);
    }
    Ok(()) // no semicolon
}

// every plan, flexible or locked, needs an end date after today
fn check_end_date(end_date: NaiveDate, today: NaiveDate) -> Result<(), PlanError> {
    if end_date <= today {
        return Err(PlanError::EndDateNotInFuture);
    }
    Ok(()) // no semicolon
}

// how many cells the plan has: cell n starts at start + (n - 1) periods
// the last cell may start on the end date itself
pub fn count_periods(start: NaiveDate, end: NaiveDate, frequency: PlanFrequency) -> i64 {
    if end < start {
        return 0;
    }
    let days = (end - start).num_days();

    match frequency {
        PlanFrequency::Daily => days + 1,
        PlanFrequency::Weekly => days / 7 + 1,
        PlanFrequency::Monthly => {
            let mut months = (end.year() - start.year()) as i64 * 12
                + end.month() as i64
                - start.month() as i64;
            // the last cell must not start after the end date
            if months > 0 {
                let last_start = start.checked_add_months(Months::new(months as u32));
                if last_start.map_or(true, |date| date > end) {
                    months -= 1;
                }
            }
            months + 1 // no semicolon
        }
    }
}

// the amount to save each period to reach the target, rounded UP to a whole naira
// None when the numbers are too big
pub fn calculate_amount_per_period(target: i64, periods: i64) -> Option<i64> {
    if periods <= 0 {
        return None;
    }
    let kobo_per_period = target.checked_add(periods - 1)? / periods;
    let whole_naira = kobo_per_period.checked_add(99)? / 100;
    whole_naira.checked_mul(100) // no semicolon
}

// the target we store is always amount x periods
pub fn calculate_stored_target(amount: i64, periods: i64) -> Option<i64> {
    amount.checked_mul(periods) // no semicolon
}

// works out the amount per period and the stored target from what the user typed
fn work_out_amount_and_target(
    amount_input: PlanAmountInput,
    periods: i64,
) -> Result<(i64, i64), PlanError> {
    let amount = match amount_input {
        PlanAmountInput::PerPeriod(amount) => {
            check_amount(amount)?;
            amount
        }
        PlanAmountInput::FromTarget(target) => {
            check_target(target)?;
            calculate_amount_per_period(target, periods).ok_or(PlanError::InvalidTarget)?
        }
    };
    let target = calculate_stored_target(amount, periods).ok_or(PlanError::InvalidAmount)?;
    Ok((amount, target)) // no semicolon
}

// check everything, work out the amount and target, then save the plan through the repo
pub async fn create_plan(pool: &PgPool, input: CreatePlanInput) -> Result<SavingsPlan, PlanError> {
    let name = clean_name(&input.name)?;
    let today = today_in_lagos();
    check_end_date(input.end_date, today)?;

    let periods = count_periods(today, input.end_date, input.frequency);
    let (amount_per_frequency, target_amount) = work_out_amount_and_target(input.amount, periods)?;

    let new_plan = NewPlan {
        user_id: input.user_id,
        name,
        frequency: input.frequency,
        amount_per_frequency,
        target_amount,
        lock_mode: input.lock_mode,
        end_date: input.end_date,
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
