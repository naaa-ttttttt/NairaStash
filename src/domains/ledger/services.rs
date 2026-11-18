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
