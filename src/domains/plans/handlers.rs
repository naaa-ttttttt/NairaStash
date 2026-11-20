use askama::Template;
use axum::{
    extract::{Form, Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Redirect, Response},
};
use chrono::NaiveDate;
use serde::Deserialize;
use uuid::Uuid;

use super::repo::{LockMode, PlanFrequency, PlanStatus, SavingsPlan};
use super::services::{self, CreatePlanInput, PlanAmountInput, PlanError};
use crate::domains::auth::handlers::AuthUser;
use crate::domains::dashboard::not_found_handler;
use crate::domains::plan_funds::services::{
        early_fee_percent, get_plan_grid, GridBlock, PeriodStep, PlanGridInput,
};
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
    pub early_fee_percent: i64, // what closing early costs, in percent
    pub end_date: Option<String>,
    pub created_on: String,
    pub is_active: bool,
}

// what the form shows, so it can be filled again after an error
#[derive(Clone)]
pub struct NewPlanValues {
    pub name: String,
    pub frequency: String,
    pub amount: String,
    pub lock_mode: String,
    pub end_date: String,
    pub has_target: bool,
    pub target: String,
}

impl NewPlanValues {
    // the first visit: nothing typed, daily and flexible pre-picked
    fn empty() -> Self {
        NewPlanValues {
            name: String::new(),
            frequency: "daily".to_string(),
            amount: String::new(),
            lock_mode: "flexible".to_string(),
            end_date: String::new(),
            has_target: false,
            target: String::new(),
        }
    }

    // copies what the user typed, so we can show it again if something is wrong
    fn from_form(form: &NewPlanForm) -> Self {
        NewPlanValues {
            name: form.name.clone(),
            frequency: form.frequency.clone(),
            amount: form.amount.clone().unwrap_or_default(),
            lock_mode: form.lock_mode.clone(),
            end_date: form.end_date.clone().unwrap_or_default(),
            has_target: form.has_target.is_some(),
            target: form.target.clone().unwrap_or_default(),
        }
    }
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
    pub form: NewPlanValues,
}

#[derive(Template)]
#[template(path = "plan_detail.html")]
pub struct PlanDetailTemplate {
    pub active: &'static str,
    pub plan: PlanView,
    pub message: Option<String>,
    pub message_is_error: bool,
    pub grid: Vec<GridBlock>, // the streak grid in blocks, empty if it could not be loaded
}

// the new plan form as the browser sends it
#[derive(Deserialize)]
pub struct NewPlanForm {
    name: String,
    frequency: String,          // "daily", "weekly" or "monthly"
    amount: Option<String>,     // naira text, not sent when the target switch is on
    lock_mode: String,          // "flexible" or "locked"
    end_date: Option<String>,   // "2027-03-01", required
    has_target: Option<String>, // a checked checkbox sends "on", unchecked sends nothing
    target: Option<String>,     // naira text, used when has_target is on
}

// the short codes the plan page can get in its url after an action
// added is how many periods were saved, withdrawn and closed are kobo that reached the wallet,
// fee is the early fee in kobo, err is the reason something was refused
#[derive(Deserialize)]
pub struct DetailQuery {
    added: Option<i64>,
    withdrawn: Option<i64>,
    closed: Option<i64>,
    fee: Option<i64>,
    err: Option<String>,
}

// ---------------------------------------------------------------- helpers

// turns a saved plan into display strings
pub fn plan_view(plan: &SavingsPlan) -> PlanView {
    // i128 so balance * 100 can never overflow, capped at 100 percent
    let progress_percent = if plan.target_amount > 0 {
        let percent = (plan.balance as i128) * 100 / (plan.target_amount as i128);
        percent.min(100) as i64
    } else {
        0
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
        target: Some(format_kobo_as_naira(plan.target_amount)),
        progress_percent,
        lock_label,
        early_fee_percent: early_fee_percent(plan.lock_mode == LockMode::Locked),
        end_date: Some(plan.end_date.format("%b %d, %Y").to_string()),
        created_on: plan.created_at.format("%b %d, %Y").to_string(),
        is_active: plan.status == PlanStatus::Active,
    }
}

// the grid has its own period type, this is where the plans type is turned into it
fn step_for_frequency(frequency: PlanFrequency) -> PeriodStep {
    match frequency {
        PlanFrequency::Daily => PeriodStep::Day,
        PlanFrequency::Weekly => PeriodStep::Week,
        PlanFrequency::Monthly => PeriodStep::Month,
    } // no semicolon
}

// copies the facts of a plan that the grid needs
fn build_grid_input(plan: &SavingsPlan) -> PlanGridInput {
    PlanGridInput {
        plan_id: plan.plan_id,
        created_at: plan.created_at,
        amount_per_frequency: plan.amount_per_frequency,
        target_amount: plan.target_amount,
        step: step_for_frequency(plan.frequency),
    } // no semicolon
}

// builds the banner text from the codes in the url, an unknown code shows nothing
// gives back the text and whether it is an error
fn detail_message(query: &DetailQuery, plan: &PlanView) -> (Option<String>, bool) {
    let fee_text = format_kobo_as_naira(query.fee.unwrap_or(0).max(0));

    if let Some(received) = query.closed.filter(|received| *received >= 0) {
        let text = if received > 0 {
            format!(
                "Plan closed. ₦{} moved to your wallet. Fee ₦{}.",
                format_kobo_as_naira(received),
                fee_text
            )
        } else {
            "Plan closed. There was no money inside, so there was no fee.".to_string()
        };
        return (Some(text), false);
    }

    if let Some(received) = query.withdrawn.filter(|received| *received > 0) {
        let text = format!(
            "₦{} moved to your wallet. Fee ₦{}.",
            format_kobo_as_naira(received),
            fee_text
        );
        return (Some(text), false);
    }

    if let Some(periods) = query.added.filter(|periods| *periods > 0) {
        let word = if periods == 1 {
            plan.unit.to_string()
        } else {
            format!("{}s", plan.unit)
        };
        return (Some(format!("Added {periods} {word} to your plan.")), false);
    }

    let Some(code) = query.err.as_deref() else {
        return (None, false);
    };

    let text = match code {
        "invalid_amount" => "Enter an amount above zero.".to_string(),
        "below_one_period" => format!("Enter at least ₦{} (one {}).", plan.amount, plan.unit),
        "not_active" => "This plan is closed, so it cannot be changed.".to_string(),
        "plan_full" => "This plan has reached its target, there is nothing left to fill.".to_string(),
        "locked" => format!(
            "Locked plans cannot be partly withdrawn. Use Close plan to take everything out, a {}% fee applies.",
            plan.early_fee_percent
        ),
        "not_enough_money" => {
            "Your wallet does not have enough money. Add money to your wallet first.".to_string()
        }
        "not_enough_in_plan" => {
            "That is more than this plan can pay, the fee is taken on top. Use the Max button or close the plan."
                .to_string()
        }
        "too_small" => "The balance in this plan is too small to withdraw.".to_string(),
        "remark_too_long" => "Your note can be at most 200 characters.".to_string(),
        "server" => "Something went wrong. Please try again later.".to_string(),
        _ => return (None, false),
    };
    (Some(text), true) // no semicolon
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

    // every plan needs an end date, the service checks it is in the future
    let end_date_text = form.end_date.unwrap_or_default();
    if end_date_text.trim().is_empty() {
        return Err(PlanError::MissingEndDate);
    }
    let end_date = NaiveDate::parse_from_str(end_date_text.trim(), "%Y-%m-%d")
        .map_err(|_| PlanError::InvalidEndDate)?;

    // with the target switch on we use the target, otherwise the typed amount
    let amount = if form.has_target.is_some() {
        let target_text = form.target.unwrap_or_default();
        let target = parse_naira_to_kobo(&target_text).ok_or(PlanError::InvalidTarget)?;
        PlanAmountInput::FromTarget(target)
    } else {
        let amount_text = form.amount.unwrap_or_default();
        let amount = parse_naira_to_kobo(&amount_text).ok_or(PlanError::InvalidAmount)?;
        PlanAmountInput::PerPeriod(amount)
    };

    Ok(CreatePlanInput {
        user_id,
        name: form.name,
        frequency,
        lock_mode,
        end_date,
        amount,
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
            "Please pick an end date in the future.",
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

// the new plan page again, with the error shown above the form and what the user typed
fn new_plan_error_page(err: &PlanError, values: NewPlanValues) -> Response {
    let (status, message) = plan_error_message(err);
    let page = NewPlanTemplate {
        active: "plans",
        error: Some(message.to_string()),
        form: values,
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
    let page = NewPlanTemplate {
        active: "plans",
        error: None,
        form: NewPlanValues::empty(),
    };
    render_page(page, StatusCode::OK)
}

// POST /plans/new: validate, save, then redirect so a refresh cannot resubmit
pub async fn create_plan_handler(
    auth: AuthUser,
    State(state): State<AppState>,
    Form(form): Form<NewPlanForm>,
) -> Response {
    // copy what was typed first, build_create_input takes the form
    let values = NewPlanValues::from_form(&form);

    let input = match build_create_input(auth.user_id, form) {
        Ok(input) => input,
        Err(err) => return new_plan_error_page(&err, values),
    };

    match services::create_plan(&state.db, input).await {
        Ok(_) => Redirect::to("/plans").into_response(), // 303 see other
        Err(err) => new_plan_error_page(&err, values),
    }
}

// GET /plans/:plan_id: one plan, a bad id or someone else's plan is a 404
// the url may carry a result code from add money, withdraw or close, shown as a banner
// the streak grid is loaded too, if that fails the page still opens with an empty grid
pub async fn plan_detail_handler(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(plan_id): Path<String>,
    Query(query): Query<DetailQuery>,
) -> Response {
    // uuid has no serde feature here, so the id arrives as text and is parsed by hand
    let Ok(plan_id) = Uuid::parse_str(&plan_id) else {
        return not_found_handler().await;
    };

    match services::get_plan(&state.db, plan_id, auth.user_id).await {
        Ok(plan) => {
            let view = plan_view(&plan);
            let (message, message_is_error) = detail_message(&query, &view);

            let grid_input = build_grid_input(&plan);
            let grid = match get_plan_grid(&state.db, &grid_input).await {
                Ok(cells) => cells,
                Err(err) => {
                    eprintln!("Plan grid error: {:?}", err);
                    Vec::new()
                }
            };

            render_page(
                PlanDetailTemplate {
                    active: "plans",
                    plan: view,
                    message,
                    message_is_error,
                    grid,
                },
                StatusCode::OK,
            )
        }
        Err(err) => plan_error_response(err).await,
    }
}
