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
pub struct TransactionView {
    pub entry_id: String,
    pub label: String,
    pub type_label: &'static str,
    pub amount: String,
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
