use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use super::repo;
use crate::domains::ledger::repo::{LedgerEntryType, NewLedgerEntry};
use crate::domains::ledger::services::{self as ledger_services, LedgerError};


// everything that can go wrong when money moves between wallet and plan
#[derive(Debug)]
pub enum PlanFundsError {
    InvalidAmount,
    BelowOnePeriod,
    PlanNotFound,
    PlanNotActive,
    PlanFull,
    NotEnoughMoney,
    Ledger(LedgerError),
    Database(sqlx::Error),
}

// lets us use ? on repo calls, a sqlx error becomes PlanFundsError::Database
impl From<sqlx::Error> for PlanFundsError {
    fn from(error: sqlx::Error) -> Self {
        PlanFundsError::Database(error) // no semicolon
    }
}

// lets us use ? on ledger calls, a LedgerError becomes PlanFundsError::Ledger
impl From<LedgerError> for PlanFundsError {
    fn from(error: LedgerError) -> Self {
        PlanFundsError::Ledger(error) // no semicolon
    }
}
