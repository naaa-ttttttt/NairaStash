use chrono::{DateTime, FixedOffset, Utc};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use super::repo::{self, LedgerEntry, LedgerEntryType, NewLedgerEntry};
use crate::shared::money::format_kobo_as_naira;

const MAX_REMARK_LENGTH: usize = 200;

// everything that can go wrong in the ledger feature
#[derive(Debug)]
pub enum LedgerError {
    InvalidAmount,
    InvalidBalance,
    InvalidFee,
    PlanRequired,
    PlanNotAllowed,
    RemarkTooLong,
    NotFound,
    Database(sqlx::Error),
}

// lets us use ? on repo calls, a sqlx error becomes LedgerError::Database
impl From<sqlx::Error> for LedgerError {
    fn from(error: sqlx::Error) -> Self {
        LedgerError::Database(error)
    }
}

// a transaction already formatted for display, templates never touch kobo
// the sign and balance_after are from the wallet's point of view
// fee is None when the row has no fee
pub struct TransactionView {
    pub entry_id: String,
    pub label: String,
    pub type_label: &'static str,
    pub amount: String,
    pub fee: Option<String>,
    pub sign: &'static str,
    pub is_credit: bool,
    pub date: String,
    pub reference: String,
    pub remark: Option<String>,
    pub plan_id: Option<String>,
    pub plan_name: String,
    pub balance_after: String,
}

// Nigeria is UTC+1 all year with no daylight saving
fn format_lagos_time(time: DateTime<Utc>) -> String {
    match FixedOffset::east_opt(60 * 60) {
        Some(lagos) => time
            .with_timezone(&lagos)
            .format("%b %d, %Y, %I:%M %p")
            .to_string(),
        None => time.format("%b %d, %Y, %I:%M %p UTC").to_string(),
    }
}

// turns one saved row into display text
fn build_view(entry: &LedgerEntry) -> TransactionView {
    let plan_name = entry
        .plan_name
        .clone()
        .unwrap_or_else(|| "your plan".to_string());

    // is_credit means the wallet went up
    let (type_label, label, is_credit) = match entry.entry_type {
        LedgerEntryType::Deposit => ("Deposit", "Added to wallet".to_string(), true),
        LedgerEntryType::PlanContribution => {
            ("Plan contribution", format!("Added to {plan_name}"), false)
        }
        LedgerEntryType::PlanWithdrawal => {
            ("Plan withdrawal", format!("Withdrawn from {plan_name}"), true)
        }
        LedgerEntryType::Payout => ("Payout", "Sent to bank".to_string(), false),
    };

    TransactionView {
        entry_id: entry.entry_id.to_string(),
        label,
        type_label,
        amount: format_kobo_as_naira(entry.amount),
        fee: if entry.fee > 0 {
            Some(format_kobo_as_naira(entry.fee))
        } else {
            None
        },
        sign: if is_credit { "+" } else { "-" },
        is_credit,
        date: format_lagos_time(entry.created_at),
        reference: entry.reference.clone(),
        remark: entry.remark.clone(),
        plan_id: entry.plan_id.map(|id| id.to_string()),
        plan_name,
        balance_after: format_kobo_as_naira(entry.wallet_balance_after),
    }
}

// the amount must be more than zero
fn check_amount(amount: i64) -> Result<(), LedgerError> {
    if amount <= 0 {
        return Err(LedgerError::InvalidAmount);
    }
    Ok(()) // no semicolon
}

// the wallet can never hold a negative balance
fn check_balance(balance: i64) -> Result<(), LedgerError> {
    if balance < 0 {
        return Err(LedgerError::InvalidBalance);
    }
    Ok(()) // no semicolon
}

// a fee is never negative, and only a plan withdrawal can have one
fn check_fee(entry_type: LedgerEntryType, fee: i64) -> Result<(), LedgerError> {
    if fee < 0 {
        return Err(LedgerError::InvalidFee);
    }
    if fee > 0 && entry_type != LedgerEntryType::PlanWithdrawal {
        return Err(LedgerError::InvalidFee);
    }
    Ok(()) // no semicolon
}

// plan movements need a plan, wallet movements must not have one
fn check_plan_matches_type(
    entry_type: LedgerEntryType,
    plan_id: Option<Uuid>,
) -> Result<(), LedgerError> {
    match (entry_type, plan_id) {
        (LedgerEntryType::PlanContribution | LedgerEntryType::PlanWithdrawal, None) => {
            Err(LedgerError::PlanRequired)
        }
        (LedgerEntryType::Deposit | LedgerEntryType::Payout, Some(_)) => {
            Err(LedgerError::PlanNotAllowed)
        }
        _ => Ok(()), // no semicolon
    }
}

// trims the remark, an empty one becomes nothing, a long one is refused
fn clean_remark(remark: Option<String>) -> Result<Option<String>, LedgerError> {
    let Some(text) = remark else {
        return Ok(None);
    };
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if trimmed.chars().count() > MAX_REMARK_LENGTH {
        return Err(LedgerError::RemarkTooLong);
    }
    Ok(Some(trimmed.to_string())) // no semicolon
}

// checks every rule, then saves the transaction on the database transaction it is given (tx)
// deposits and plans both call this, so the rules live in one place
pub async fn record_transaction(
    tx: &mut Transaction<'_, Postgres>,
    mut new_entry: NewLedgerEntry,
) -> Result<(), LedgerError> {
    check_amount(new_entry.amount)?;
    check_balance(new_entry.wallet_balance_after)?;
    check_fee(new_entry.entry_type, new_entry.fee)?;
    check_plan_matches_type(new_entry.entry_type, new_entry.plan_id)?;
    new_entry.remark = clean_remark(new_entry.remark)?;
    repo::save_transaction(tx, &new_entry).await?;
    Ok(()) // no semicolon
}

// the latest transactions of one user, for the activity page
pub async fn get_user_activity(
    pool: &PgPool,
    user_id: Uuid,
    limit: i64,
) -> Result<Vec<TransactionView>, LedgerError> {
    let entries = repo::list_user_transactions(pool, user_id, limit).await?;
    let views = entries.iter().map(build_view).collect();
    Ok(views) // no semicolon
}

// the latest transactions of one plan, for the plan page
pub async fn get_plan_history(
    pool: &PgPool,
    plan_id: Uuid,
    user_id: Uuid,
    limit: i64,
) -> Result<Vec<TransactionView>, LedgerError> {
    let entries = repo::list_plan_transactions(pool, plan_id, user_id, limit).await?;
    let views = entries.iter().map(build_view).collect();
    Ok(views) // no semicolon
}

// one transaction of the user, or NotFound if it is missing or not theirs
pub async fn get_transaction_details(
    pool: &PgPool,
    entry_id: Uuid,
    user_id: Uuid,
) -> Result<TransactionView, LedgerError> {
    let found = repo::get_transaction_details(pool, entry_id, user_id).await?;
    let entry = found.ok_or(LedgerError::NotFound)?;
    Ok(build_view(&entry)) // no semicolon
}
