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
