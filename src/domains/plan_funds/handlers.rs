use axum::{
    extract::{Form, Path, State},
    response::{IntoResponse, Redirect, Response},
};
use serde::Deserialize;
use uuid::Uuid;

use super::services::{self, PlanFundsError};
use crate::domains::auth::handlers::AuthUser;
use crate::domains::dashboard::not_found_handler;
use crate::domains::ledger::services::LedgerError;
use crate::shared::money::parse_naira_to_kobo;
use crate::AppState;

// the add money form as the browser sends it
#[derive(Deserialize)]
pub struct AddMoneyForm {
    amount: String,         // naira text like "10,000"
    remark: Option<String>, // optional note
}

// the withdraw form as the browser sends it
#[derive(Deserialize)]
pub struct WithdrawForm {
    amount: String,         // naira text like "10,000"
    remark: Option<String>, // optional note
}

// sends the user back to the plan page, the code tells the page which message to show
fn redirect_with_error(plan_id: Uuid, code: &str) -> Response {
    Redirect::to(&format!("/plans/{plan_id}?err={code}")).into_response() // no semicolon
}

// one short code per error, the plan page turns each code into a message
fn error_code(err: &PlanFundsError) -> &'static str {
    match err {
        PlanFundsError::InvalidAmount => "invalid_amount",
        PlanFundsError::BelowOnePeriod => "below_one_period",
        PlanFundsError::PlanNotActive => "not_active",
        PlanFundsError::PlanFull => "plan_full",
        PlanFundsError::PlanLocked => "locked",
        PlanFundsError::NotEnoughMoney => "not_enough_money",
        PlanFundsError::NotEnoughInPlan => "not_enough_in_plan",
        PlanFundsError::BalanceTooSmall => "too_small",
        PlanFundsError::Ledger(LedgerError::RemarkTooLong) => "remark_too_long",
        PlanFundsError::PlanNotFound
        | PlanFundsError::WalletNotFound
        | PlanFundsError::Ledger(_)
        | PlanFundsError::Database(_) => "server",
    }
}

// one place that turns any plan_funds error into a page: a missing plan is a 404, the rest go back with a code
async fn error_response(plan_id: Uuid, err: PlanFundsError) -> Response {
    if let PlanFundsError::PlanNotFound = err {
        return not_found_handler().await;
    }
    let code = error_code(&err);
    if code == "server" {
        eprintln!("Plan funds error: {:?}", err);
    }
    redirect_with_error(plan_id, code) // no semicolon
}

// POST /plans/:plan_id/add: move money from the wallet into the plan, then go back to the plan page
pub async fn add_money_handler(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(plan_id): Path<String>,
    Form(form): Form<AddMoneyForm>,
) -> Response {
    // uuid has no serde feature here, so the id arrives as text and is parsed by hand
    let Ok(plan_id) = Uuid::parse_str(&plan_id) else {
        return not_found_handler().await;
    };

    let Some(amount) = parse_naira_to_kobo(&form.amount) else {
        return redirect_with_error(plan_id, "invalid_amount");
    };

    match services::add_money(&state.db, auth.user_id, plan_id, amount, form.remark).await {
        Ok(result) => {
            Redirect::to(&format!("/plans/{plan_id}?added={}", result.periods_added))
                .into_response() // no semicolon
        }
        Err(err) => error_response(plan_id, err).await,
    }
}

// POST /plans/:plan_id/withdraw: take part of the money out of a flexible plan
pub async fn withdraw_handler(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(plan_id): Path<String>,
    Form(form): Form<WithdrawForm>,
) -> Response {
    let Ok(plan_id) = Uuid::parse_str(&plan_id) else {
        return not_found_handler().await;
    };

    let Some(amount) = parse_naira_to_kobo(&form.amount) else {
        return redirect_with_error(plan_id, "invalid_amount");
    };

    match services::withdraw_from_plan(&state.db, auth.user_id, plan_id, amount, form.remark).await
    {
        Ok(result) => Redirect::to(&format!(
            "/plans/{plan_id}?withdrawn={}&fee={}",
            result.amount_received, result.fee
        ))
        .into_response(), // no semicolon
        Err(err) => error_response(plan_id, err).await,
    }
}

// POST /plans/:plan_id/liquidate: close the plan, the whole balance goes to the wallet
pub async fn liquidate_handler(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(plan_id): Path<String>,
) -> Response {
    let Ok(plan_id) = Uuid::parse_str(&plan_id) else {
        return not_found_handler().await;
    };

    match services::liquidate_plan(&state.db, auth.user_id, plan_id).await {
        Ok(result) => Redirect::to(&format!(
            "/plans/{plan_id}?closed={}&fee={}",
            result.amount_received, result.fee
        ))
        .into_response(), // no semicolon
        Err(err) => error_response(plan_id, err).await,
    }
}
