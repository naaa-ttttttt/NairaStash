use chrono::{DateTime, NaiveDate, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "plan_frequency", rename_all = "lowercase")]
pub enum PlanFrequency {
    Daily,
    Weekly,
    Monthly,
}

// locked plans pay nothing out before the end date, flexible plans allow early withdrawal with a fee
#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "lock_mode", rename_all = "lowercase")]
pub enum LockMode {
    Locked,
    Flexible,
}

// active plans can be saved into, closed plans cannot
#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "plan_status", rename_all = "lowercase")]
pub enum PlanStatus {
    Active,
    Closed,
}

// one row of the savings_plans table, money is in kobo
// every plan has an end date and a target, the target is always amount x number of periods
#[derive(Debug, Clone, FromRow)]
pub struct SavingsPlan {
    pub plan_id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub frequency: PlanFrequency,
    pub amount_per_frequency: i64,
    pub target_amount: i64,
    pub lock_mode: LockMode,
    pub end_date: NaiveDate,
    pub balance: i64,
    pub status: PlanStatus,
    pub created_at: DateTime<Utc>,
}

// what we need to create a plan
// balance, status and created_at are set by the database defaults
#[derive(Debug, Clone)]
pub struct NewPlan {
    pub user_id: Uuid,
    pub name: String,
    pub frequency: PlanFrequency,
    pub amount_per_frequency: i64,
    pub target_amount: i64,
    pub lock_mode: LockMode,
    pub end_date: NaiveDate,
}

// create a plan and give back the saved row
pub async fn create_plan(pool: &PgPool, new_plan: &NewPlan) -> Result<SavingsPlan, sqlx::Error> {
    // a fresh id for the plan
    let plan_id = Uuid::new_v4();
    let query = sqlx::query_as::<_, SavingsPlan>(INSERT_PLAN_SQL)
        .bind(plan_id)
        .bind(new_plan.user_id)
        .bind(&new_plan.name)
        .bind(new_plan.frequency)
        .bind(new_plan.amount_per_frequency)
        .bind(new_plan.target_amount)
        .bind(new_plan.lock_mode)
        .bind(new_plan.end_date);
    let created_plan = query.fetch_one(pool).await?;
    Ok(created_plan) // no semicolon
}

// every plan the user owns, newest first
pub async fn list_plans_for_user(pool: &PgPool, user_id: Uuid) -> Result<Vec<SavingsPlan>, sqlx::Error> {
    let query = sqlx::query_as::<_, SavingsPlan>(LIST_PLANS_SQL).bind(user_id);
    let plans = query.fetch_all(pool).await?;
    Ok(plans) // no semicolon
}

// only the active plans, used by the dashboard
pub async fn list_active_plans_for_user(pool: &PgPool, user_id: Uuid) -> Result<Vec<SavingsPlan>, sqlx::Error> {
    let query = sqlx::query_as::<_, SavingsPlan>(LIST_ACTIVE_PLANS_SQL).bind(user_id);
    let active_plans = query.fetch_all(pool).await?;
    Ok(active_plans) // no semicolon
}

// one plan, but only if it belongs to this user
pub async fn find_plan_for_user(
    pool: &PgPool,
    plan_id: Uuid,
    user_id: Uuid,
) -> Result<Option<SavingsPlan>, sqlx::Error> {
    let query = sqlx::query_as::<_, SavingsPlan>(FIND_PLAN_SQL)
        .bind(plan_id)
        .bind(user_id);
    let found_plan = query.fetch_optional(pool).await?;
    Ok(found_plan) // no semicolon
}

const INSERT_PLAN_SQL: &str = "
    INSERT INTO savings_plans
        (plan_id, user_id, name, frequency, amount_per_frequency, target_amount, lock_mode, end_date)
    VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
    RETURNING *";

const LIST_PLANS_SQL: &str = "
    SELECT * FROM savings_plans
    WHERE user_id = $1 AND status = 'active'
    ORDER BY created_at DESC";

const LIST_ACTIVE_PLANS_SQL: &str = "
    SELECT * FROM savings_plans
    WHERE user_id = $1 AND status = 'active'
    ORDER BY created_at DESC";

const FIND_PLAN_SQL: &str = "
    SELECT * FROM savings_plans
    WHERE plan_id = $1 AND user_id = $2";
