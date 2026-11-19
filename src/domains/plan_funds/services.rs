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


// moves money from the wallet to the plan, whole periods only
// everything happens in one database transaction: if any step fails, nothing is kept
pub async fn add_money(
    pool: &PgPool,
    user_id: Uuid,
    plan_id: Uuid,
    amount: i64,
    remark: Option<String>,
) -> Result<AddMoneyResult, PlanFundsError> {
    // rule 1: the amount must be positive and pay at least one period
    if amount <= 0 {
        return Err(PlanFundsError::InvalidAmount);
    }

    let mut tx: Transaction<'_, Postgres> = pool.begin().await?;

    // lock the plan first, so nobody else can change it until we commit
    let plan = repo::lock_plan_for_update(&mut tx, plan_id, user_id)
        .await?
        .ok_or(PlanFundsError::PlanNotFound)?;

    // rule 2: only an active plan can receive money
    if plan.status != "active" {
        return Err(PlanFundsError::PlanNotActive);
    }

    let periods_requested = count_whole_periods(amount, plan.amount_per_frequency);
    if periods_requested == 0 {
        return Err(PlanFundsError::BelowOnePeriod);
    }

    // rule 3: count the filled cells inside the tx, after the lock, so the count is fresh
    let filled_cells = repo::count_plan_contributions_in_tx(&mut tx, plan_id).await?;
    let empty_cells =
        count_empty_cells(plan.target_amount, plan.amount_per_frequency, filled_cells);
    if empty_cells <= 0 {
        return Err(PlanFundsError::PlanFull);
    }

    // never fill more cells than are empty, the rest stays in the wallet
    let periods_to_move = periods_requested.min(empty_cells);
    let amount_to_move = periods_to_move * plan.amount_per_frequency;

    // rule 4: the wallet must have the money, the guarded debit checks it safely
    let wallet_after_debit = repo::debit_wallet_if_enough(&mut tx, user_id, amount_to_move)
        .await?
        .ok_or(PlanFundsError::NotEnoughMoney)?;

    let plan_balance_after = repo::add_to_plan_balance(&mut tx, plan_id, amount_to_move).await?;

    // one ledger row per period, each row shows the wallet balance right after that period
    let wallet_before_debit = wallet_after_debit + amount_to_move;
    for period_number in 1..=periods_to_move {
        let new_entry = NewLedgerEntry {
            fee: 0,
            user_id,
            entry_type: LedgerEntryType::PlanContribution,
            amount: plan.amount_per_frequency,
            plan_id: Some(plan_id),
            remark: remark.clone(),
            reference: make_contribution_reference(),
            wallet_balance_after: wallet_before_debit
                - period_number * plan.amount_per_frequency,
        };
        ledger_services::record_transaction(&mut tx, new_entry).await?;
    }

    tx.commit().await?;

    let result = AddMoneyResult {
        periods_added: periods_to_move,
        amount_moved: amount_to_move,
        amount_not_moved: amount - amount_to_move,
        wallet_balance_after: wallet_after_debit,
        plan_balance_after,
    };
    Ok(result) // no semicolon
}
