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

// how many whole periods the amount can pay, the leftover is dropped
fn count_whole_periods(amount: i64, amount_per_frequency: i64) -> i64 {
    amount / amount_per_frequency // no semicolon
}


// how many cells of the streak grid are still empty
// the target is always a multiple of the period amount, so this divides cleanly
fn count_empty_cells(target_amount: i64, amount_per_frequency: i64, filled_cells: i64) -> i64 {
    let total_cells = target_amount / amount_per_frequency;
    total_cells - filled_cells // no semicolon
}


// a fresh unique reference for one ledger row, the reference column must be unique
fn make_contribution_reference() -> String {
    format!("PC-{}", Uuid::new_v4().simple()) // no semicolon
}
