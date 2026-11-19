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

// what add_money gives back, money is in kobo
// the handler uses it to show what moved and what stayed in the wallet
#[derive(Debug)]
pub struct AddMoneyResult {
    pub periods_added: i64,
    pub amount_moved: i64,
    pub amount_not_moved: i64,
    pub wallet_balance_after: i64,
    pub plan_balance_after: i64,
}
