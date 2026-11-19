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
        PlanFundsError::NotEnoughMoney => "not_enough_money",
        PlanFundsError::Ledger(LedgerError::RemarkTooLong) => "remark_too_long",
        PlanFundsError::PlanNotFound
        | PlanFundsError::Ledger(_)
        | PlanFundsError::Database(_) => "server",
    }
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
        Err(PlanFundsError::PlanNotFound) => not_found_handler().await,
        Err(err) => {
            let code = error_code(&err);
            if code == "server" {
                eprintln!("Add money error: {:?}", err);
            }
            redirect_with_error(plan_id, code) // no semicolon
        }
    }
}
