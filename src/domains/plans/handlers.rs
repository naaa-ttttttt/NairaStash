use askama::Template;
use axum::{
    extract::{Form, Path, State},
    http::StatusCode,
    response::{IntoResponse, Redirect, Response},
};
use chrono::NaiveDate;
use serde::Deserialize;
use uuid::Uuid;

use super::repo::{LockMode, PlanFrequency, PlanStatus, SavingsPlan};
use super::services::{self, CreatePlanInput, PlanError};
use crate::domains::auth::handlers::AuthUser;
use crate::domains::dashboard::not_found_handler;
use crate::shared::money::{format_kobo_as_naira, parse_naira_to_kobo};
use crate::shared::render::render_page;
use crate::AppState;

// ---------------------------------------------------------------- what the pages receive

// a plan already formatted for display, templates never touch kobo
pub struct PlanView {
    pub plan_id: String,
    pub name: String,
    pub frequency_label: &'static str,
    pub unit: &'static str, // "day", "week" or "month"
    pub amount: String,
    pub balance: String,
    pub target: Option<String>,
    pub progress_percent: i64,
    pub lock_label: &'static str,
    pub end_date: Option<String>,
    pub created_on: String,
    pub is_active: bool,
}

#[derive(Template)]
#[template(path = "plans.html")]
pub struct PlansTemplate {
    pub active: &'static str,
    pub active_count: usize,
    pub total_saved: String,
    pub plans: Vec<PlanView>,
}

#[derive(Template)]
#[template(path = "new_plan.html")]
pub struct NewPlanTemplate {
    pub active: &'static str,
    pub error: Option<String>,
}

#[derive(Template)]
#[template(path = "plan_detail.html")]
pub struct PlanDetailTemplate {
    pub active: &'static str,
    pub plan: PlanView,
}

// the new plan form as the browser sends it
#[derive(Deserialize)]
pub struct NewPlanForm {
    name: String,
    frequency: String,           // "daily", "weekly" or "monthly"
    amount: String,              // naira text like "2,000.50"
    lock_mode: String,           // "flexible" or "locked"
    end_date: Option<String>,    // "2027-03-01", only used when locked
    has_target: Option<String>,  // a checked checkbox sends "on", unchecked sends nothing
    target: Option<String>,      // naira text, only used when has_target is on
}

// ---------------------------------------------------------------- helpers

// turns a saved plan into display strings
pub fn plan_view(plan: &SavingsPlan) -> PlanView {
    // i128 so balance * 100 can never overflow, capped at 100 percent
    let progress_percent = match plan.target_amount {
        Some(target) if target > 0 => {
            let percent = (plan.balance as i128) * 100 / (target as i128);
            percent.min(100) as i64
        }
        _ => 0,
    };

    let frequency_label = match plan.frequency {
        PlanFrequency::Daily => "Daily",
        PlanFrequency::Weekly => "Weekly",
        PlanFrequency::Monthly => "Monthly",
    };

    let unit = match plan.frequency {
        PlanFrequency::Daily => "day",
        PlanFrequency::Weekly => "week",
        PlanFrequency::Monthly => "month",
    };

    let lock_label = match plan.lock_mode {
        LockMode::Locked => "Locked",
        LockMode::Flexible => "Flexible",
    };

    PlanView {
        plan_id: plan.plan_id.to_string(),
        name: plan.name.clone(),
        frequency_label,
        unit,
        amount: format_kobo_as_naira(plan.amount_per_frequency),
        balance: format_kobo_as_naira(plan.balance),
        target: plan.target_amount.map(format_kobo_as_naira),
        progress_percent,
        lock_label,
        end_date: plan.end_date.map(|date| date.format("%b %d, %Y").to_string()),
        created_on: plan.created_at.format("%b %d, %Y").to_string(),
        is_active: plan.status == PlanStatus::Active,
    }
}

// turns the form text into what the service expects, naira text becomes kobo
fn build_create_input(user_id: Uuid, form: NewPlanForm) -> Result<CreatePlanInput, PlanError> {
    let frequency = match form.frequency.as_str() {
        "daily" => PlanFrequency::Daily,
        "weekly" => PlanFrequency::Weekly,
        "monthly" => PlanFrequency::Monthly,
        _ => return Err(PlanError::InvalidFrequency),
    };

    let lock_mode = match form.lock_mode.as_str() {
        "flexible" => LockMode::Flexible,
        "locked" => LockMode::Locked,
        _ => return Err(PlanError::InvalidLockMode),
    };

    let amount_per_frequency =
        parse_naira_to_kobo(&form.amount).ok_or(PlanError::InvalidAmount)?;

    // only a locked plan has an end date, the service checks it is in the future
    let end_date = match lock_mode {
        LockMode::Flexible => None,
        LockMode::Locked => {
            let text = form.end_date.unwrap_or_default();
            if text.trim().is_empty() {
                return Err(PlanError::MissingEndDate);
            }
            let date = NaiveDate::parse_from_str(text.trim(), "%Y-%m-%d")
                .map_err(|_| PlanError::InvalidEndDate)?;
            Some(date)
        }
    };

    // the target only counts when the box is checked and something is typed
    let target_text = form.target.unwrap_or_default();
    let wants_target = form.has_target.is_some() && !target_text.trim().is_empty();

    let target_amount = if wants_target {
        Some(parse_naira_to_kobo(&target_text).ok_or(PlanError::InvalidTarget)?)
    } else {
        None
    };

    Ok(CreatePlanInput {
        user_id,
        name: form.name,
        frequency,
        amount_per_frequency,
        target_amount,
        lock_mode,
        end_date,
    }) // no semicolon
}

// picks the status code and the message the user sees for each plan error
fn plan_error_message(err: &PlanError) -> (StatusCode, &'static str) {
    match err {
        PlanError::EmptyName => (StatusCode::BAD_REQUEST, "Please give your plan a name."),
        PlanError::NameTooLong => (
            StatusCode::BAD_REQUEST,
            "Plan names can be at most 60 characters.",
        ),
        PlanError::InvalidAmount => (StatusCode::BAD_REQUEST, "Enter an amount above zero."),
        PlanError::InvalidTarget => (
            StatusCode::BAD_REQUEST,
            "Enter a target above zero, or turn the target off.",
        ),
        PlanError::InvalidFrequency => (
            StatusCode::BAD_REQUEST,
            "Please pick daily, weekly or monthly.",
        ),
        PlanError::InvalidLockMode => (
            StatusCode::BAD_REQUEST,
            "Please pick flexible or locked.",
        ),
        PlanError::InvalidEndDate => (StatusCode::BAD_REQUEST, "Please pick a valid end date."),
        PlanError::MissingEndDate | PlanError::EndDateNotInFuture => (
            StatusCode::BAD_REQUEST,
            "Locked plans need an end date in the future.",
        ),
        PlanError::NotFound => (StatusCode::NOT_FOUND, "We could not find that plan."),
        PlanError::Database(e) => {
            eprintln!("Plans database error: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Something went wrong. Please try again later.",
            )
        }
    }
}

// the new plan page again, with the error shown above the form
fn new_plan_error_page(err: &PlanError) -> Response {
    let (status, message) = plan_error_message(err);
    let page = NewPlanTemplate {
        active: "plans",
        error: Some(message.to_string()),
    };
    render_page(page, status)
}

// errors on pages that are not a form: a missing plan gets the 404 page
async fn plan_error_response(err: PlanError) -> Response {
    match err {
        PlanError::NotFound => not_found_handler().await,
        other => plan_error_message(&other).0.into_response(),
    }
}

// ---------------------------------------------------------------- handlers

// GET /plans: every plan of the logged in user
pub async fn plans_page_handler(auth: AuthUser, State(state): State<AppState>) -> Response {
    match services::list_plans(&state.db, auth.user_id).await {
        Ok(plans) => {
            let active_count = plans
                .iter()
                .filter(|plan| plan.status == PlanStatus::Active)
                .count();

            // saturating_add so a huge sum can never overflow
            let total_saved_in_kobo = plans
                .iter()
                .fold(0i64, |sum, plan| sum.saturating_add(plan.balance));

            let page = PlansTemplate {
                active: "plans",
                active_count,
                total_saved: format_kobo_as_naira(total_saved_in_kobo),
                plans: plans.iter().map(plan_view).collect(),
            };

            render_page(page, StatusCode::OK)
        }
        Err(err) => plan_error_response(err).await,
    }
}

// GET /plans/new: the empty form
pub async fn new_plan_page_handler(_auth: AuthUser) -> Response {
    render_page(NewPlanTemplate { active: "plans", error: None }, StatusCode::OK)
}

// POST /plans/new: validate, save, then redirect so a refresh cannot resubmit
pub async fn create_plan_handler(
    auth: AuthUser,
    State(state): State<AppState>,
    Form(form): Form<NewPlanForm>,
) -> Response {
    let input = match build_create_input(auth.user_id, form) {
        Ok(input) => input,
        Err(err) => return new_plan_error_page(&err),
    };

    match services::create_plan(&state.db, input).await {
        Ok(_) => Redirect::to("/plans").into_response(), // 303 see other
        Err(err) => new_plan_error_page(&err),
    }
}

// GET /plans/:plan_id: one plan, a bad id or someone else's plan is a 404
pub async fn plan_detail_handler(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(plan_id): Path<String>,
) -> Response {
    // uuid has no serde feature here, so the id arrives as text and is parsed by hand
    let Ok(plan_id) = Uuid::parse_str(&plan_id) else {
        return not_found_handler().await;
    };

    match services::get_plan(&state.db, plan_id, auth.user_id).await {
        Ok(plan) => render_page(
            PlanDetailTemplate { active: "plans", plan: plan_view(&plan) },
            StatusCode::OK,
        ),
        Err(err) => plan_error_response(err).await,
    }
}
