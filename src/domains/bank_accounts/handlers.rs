use axum::Json;
use serde::Serialize;
use askama::Template;
use axum::{
    extract::{Form, Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Redirect, Response},
};
use serde::Deserialize;
use uuid::Uuid;

use super::repo::BankAccount;
use super::services::{self, BankAccountError, BankOption, ResolvedAccount, MAX_SAVED_BANKS};
use crate::domains::auth::handlers::AuthUser;
use crate::domains::dashboard::not_found_handler;
use crate::shared::render::render_page;
use crate::AppState;

// ---------------------------------------------------------------- what the pages receive

// a saved bank already formatted for display, templates never see the full account number
pub struct BankView {
    pub bank_account_id: String, // used in the remove button's url
    pub bank_name: String,
    pub masked_number: String, // "******1234"
    pub account_name: String,
}

// the form of both steps: step 1 sends it from the add page, step 2 from the confirm screen
// there is no name field on purpose, the service gets the name from paystack
#[derive(Deserialize, Clone)]
pub struct BankForm {
    pub bank_code: String,
    pub account_number: String,
}

impl BankForm {
    // the first visit to the add page: nothing typed, nothing picked
    fn empty() -> Self {
        BankForm {
            bank_code: String::new(),
            account_number: String::new(),
        } // no semicolon
    }
}

// the banks list page
#[derive(Template)]
#[template(path = "banks.html")]
pub struct BanksTemplate {
    pub active: &'static str,
    pub banks: Vec<BankView>,
    pub can_add_more: bool, // false once the user has MAX_SAVED_BANKS
    pub max_banks: i64,
    pub error: Option<String>, // banner from a failed save or remove
}

// the add page, it has two looks: the form, or the confirm screen when confirm is Some
#[derive(Template)]
#[template(path = "add_bank.html")]
pub struct AddBankTemplate {
    pub active: &'static str,
    pub error: Option<String>,
    pub banks: Vec<BankOption>, // the dropdown, empty on the confirm screen
    pub form: BankForm,         // what the user typed, so the form can be filled again
    pub confirm: Option<ResolvedAccount>, // the account paystack found, shown for confirming
}

// the short code the banks page can get in its url after a failed save or remove
#[derive(Deserialize)]
pub struct BanksQuery {
    err: Option<String>,
}

// ---------------------------------------------------------------- helpers

// shows only the last 4 digits of an account number
fn mask_account_number(number: &str) -> String {
    let length = number.len();

    // too short to hide anything, show as is
    if length <= 4 {
        return number.to_string();
    }

    format!("******{}", &number[length - 4..]) // no semicolon
}

// turns a saved bank into display strings
fn bank_view(bank: &BankAccount) -> BankView {
    BankView {
        bank_account_id: bank.bank_account_id.to_string(),
        bank_name: bank.bank_name.clone(),
        masked_number: mask_account_number(&bank.account_number),
        account_name: bank.account_name.clone(),
    } // no semicolon
}

// gives each error a short code, used in the url when we redirect and as the key for the text
// the two errors the user cannot fix are logged here and share the code "server"
fn error_code(err: &BankAccountError) -> &'static str {
    match err {
        BankAccountError::InvalidAccountNumber => "invalid_account_number",
        BankAccountError::UnknownBank => "unknown_bank",
        BankAccountError::AccountNotFound => "account_not_found",
        BankAccountError::PaystackUnavailable => "paystack_unavailable",
        BankAccountError::TooManyBanks => "too_many_banks",
        BankAccountError::AlreadySaved => "already_saved",
        BankAccountError::BankAccountNotFound => "bank_not_found",
        BankAccountError::UserNotFound => {
            eprintln!("Bank accounts: logged in user has no row in users");
            "server"
        }
        BankAccountError::Database(e) => {
            eprintln!("Bank accounts database error: {:?}", e);
            "server"
        }
    } // no semicolon
}

// the text the user sees for a code, an unknown code shows nothing
// this is the only place the texts live
fn code_message(code: &str) -> Option<String> {
    let text = match code {
        "invalid_account_number" => "Account numbers have exactly 10 digits.".to_string(),
        "unknown_bank" => "Please choose your bank from the list.".to_string(),
        "account_not_found" => "We could not find that account. Check the number and the bank.".to_string(),
        "paystack_unavailable" => "We could not reach the bank service. Please try again in a moment.".to_string(),
        "too_many_banks" => format!(
            "You can save at most {} bank accounts. Remove one to add another.",
            MAX_SAVED_BANKS
        ),
        "already_saved" => "You have already saved this account.".to_string(),
        "bank_not_found" => "We could not find that bank account.".to_string(),
        "server" => "Something went wrong. Please try again later.".to_string(),
        _ => return None,
    };

    Some(text) // no semicolon
}

// the status code for each error, used when we render a page instead of redirecting
fn error_status(err: &BankAccountError) -> StatusCode {
    match err {
        BankAccountError::InvalidAccountNumber
        | BankAccountError::UnknownBank
        | BankAccountError::AccountNotFound => StatusCode::BAD_REQUEST,
        BankAccountError::TooManyBanks | BankAccountError::AlreadySaved => StatusCode::CONFLICT,
        BankAccountError::BankAccountNotFound => StatusCode::NOT_FOUND,
        BankAccountError::PaystackUnavailable => StatusCode::SERVICE_UNAVAILABLE,
        BankAccountError::UserNotFound | BankAccountError::Database(_) => {
            StatusCode::INTERNAL_SERVER_ERROR
        }
    } // no semicolon
}

// the add page again with the error above the form and what the user typed
// the dropdown is loaded again, if paystack fails now it stays empty
async fn add_bank_error_page(state: &AppState, err: &BankAccountError, form: BankForm) -> Response {
    let message = code_message(error_code(err));
    let banks = services::list_available_banks(&state.http_client, &state.paystack)
        .await
        .unwrap_or_default();

    let page = AddBankTemplate {
        active: "banks",
        error: message,
        banks,
        form,
        confirm: None,
    };
    render_page(page, error_status(err)) // no semicolon
}

// ---------------------------------------------------------------- handlers

// GET /banks: every active saved bank of the logged in user
// the url may carry an error code from a failed save or remove, shown as a banner
pub async fn banks_page_handler(
    auth: AuthUser,
    State(state): State<AppState>,
    Query(query): Query<BanksQuery>,
) -> Response {
    match services::list_bank_accounts(&state.db, auth.user_id).await {
        Ok(banks) => {
            let page = BanksTemplate {
                active: "banks",
                can_add_more: (banks.len() as i64) < MAX_SAVED_BANKS,
                max_banks: MAX_SAVED_BANKS,
                error: query.err.as_deref().and_then(code_message),
                banks: banks.iter().map(bank_view).collect(),
            };
            render_page(page, StatusCode::OK)
        }
        Err(err) => {
            let status = error_status(&err);
            let message = code_message(error_code(&err));
            eprintln!("Banks page failed with status {}: {:?}", status, message);
            status.into_response()
        }
    }
}

// GET /banks/new: the empty form with the bank dropdown
// if paystack is down the form still opens, with an empty dropdown and the message
pub async fn new_bank_page_handler(auth: AuthUser, State(state): State<AppState>) -> Response {
    let _ = auth; // the guard is all we need here, the page has no user data

    match services::list_available_banks(&state.http_client, &state.paystack).await {
        Ok(banks) => {
            let page = AddBankTemplate {
                active: "banks",
                error: None,
                banks,
                form: BankForm::empty(),
                confirm: None,
            };
            render_page(page, StatusCode::OK)
        }
        Err(err) => add_bank_error_page(&state, &err, BankForm::empty()).await,
    }
}

// POST /banks/resolve: step 1, asks paystack whose account it is and shows the confirm screen
// it writes nothing, so a refresh is harmless
pub async fn resolve_bank_handler(
    _auth: AuthUser,
    State(state): State<AppState>,
    Form(form): Form<BankForm>,
) -> Response {
    let result = services::resolve_bank_account(
        &state.http_client,
        &state.paystack,
        &form.bank_code,
        &form.account_number,
    )
    .await;

    match result {
        Ok(account) => {
            let page = AddBankTemplate {
                active: "banks",
                error: None,
                banks: Vec::new(), // the confirm screen has no dropdown
                form,
                confirm: Some(account),
            };
            render_page(page, StatusCode::OK)
        }
        Err(err) => add_bank_error_page(&state, &err, form).await,
    }
}

// POST /banks: step 2, saves the bank, the user_id comes from the session only
// the service resolves the name again, so nothing from the browser is trusted
// we redirect either way so a refresh cannot save twice
pub async fn save_bank_handler(
    auth: AuthUser,
    State(state): State<AppState>,
    Form(form): Form<BankForm>,
) -> Response {
    let result = services::save_bank_account(
        &state.db,
        &state.http_client,
        &state.paystack,
        auth.user_id,
        &form.bank_code,
        &form.account_number,
    )
    .await;

    match result {
        Ok(_) => Redirect::to("/banks").into_response(), // 303 see other
        Err(err) => Redirect::to(&format!("/banks?err={}", error_code(&err))).into_response(),
    }
}

// POST /banks/:bank_account_id/remove: soft deletes one saved bank
// a bad id is a 404 page, someone else's bank gets the same "not found" as a missing one
pub async fn remove_bank_handler(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(bank_account_id): Path<String>,
) -> Response {
    // uuid has no serde feature here, so the id arrives as text and is parsed by hand
    let Ok(bank_account_id) = Uuid::parse_str(&bank_account_id) else {
        return not_found_handler().await;
    };

    match services::remove_bank_account(&state.db, auth.user_id, bank_account_id).await {
        Ok(()) => Redirect::to("/banks").into_response(),
        Err(err) => Redirect::to(&format!("/banks?err={}", error_code(&err))).into_response(),
    }
}

// what the add page's script receives when it asks for an account name
#[derive(Serialize)]
struct LookupAnswer {
    ok: bool,
    account_name: Option<String>,
    message: Option<String>,
}

// POST /banks/lookup: the add page calls this once 10 digits are typed, and shows the name
// it needs a logged in user, writes nothing, and the answer is small json instead of a page
pub async fn lookup_bank_handler(
    _auth: AuthUser,
    State(state): State<AppState>,
    Form(form): Form<BankForm>,
) -> Response {
    let result = services::resolve_bank_account(
        &state.http_client,
        &state.paystack,
        &form.bank_code,
        &form.account_number,
    )
    .await;

    match result {
        Ok(account) => Json(LookupAnswer {
            ok: true,
            account_name: Some(account.account_name),
            message: None,
        })
        .into_response(),
        Err(err) => {
            let answer = LookupAnswer {
                ok: false,
                account_name: None,
                message: code_message(error_code(&err)),
            };
            (error_status(&err), Json(answer)).into_response()
        }
    } // no semicolon
}
