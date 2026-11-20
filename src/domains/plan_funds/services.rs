use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use super::repo::{self, LockedPlan};
use crate::domains::ledger::repo::{LedgerEntryType, NewLedgerEntry};
use crate::domains::ledger::services::{self as ledger_services, LedgerError};

// the plan columns come back as text, so the two values we compare are named once here
const PLAN_STATUS_ACTIVE: &str = "active";
const PLAN_MODE_LOCKED: &str = "locked";

// the early fee in percent, flexible plans pay less than locked plans
// this is the only place these numbers live, the pages ask for them
const FLEXIBLE_FEE_PERCENT: i64 = 3;
const LOCKED_FEE_PERCENT: i64 = 10;

// everything that can go wrong when money moves between wallet and plan
#[derive(Debug)]
pub enum PlanFundsError {
    InvalidAmount,
    BelowOnePeriod,
    PlanNotFound,
    PlanNotActive,
    PlanFull,
    PlanLocked,
    NotEnoughMoney,
    NotEnoughInPlan,
    BalanceTooSmall,
    WalletNotFound,
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
#[derive(Debug)]
pub struct AddMoneyResult {
    pub periods_added: i64,
    pub amount_moved: i64,
    pub amount_not_moved: i64,
    pub wallet_balance_after: i64,
    pub plan_balance_after: i64,
}

// what withdraw_from_plan and liquidate_plan give back, money is in kobo
// amount_received is what reached the wallet, fee is what the plan paid on top
#[derive(Debug)]
pub struct WithdrawResult {
    pub amount_received: i64,
    pub fee: i64,
}

// ---------------------------------------------------------------- small rules

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
fn make_reference(prefix: &str) -> String {
    format!("{prefix}-{}", Uuid::new_v4().simple()) // no semicolon
}

// the early fee percent for a plan, locked plans pay more
pub fn early_fee_percent(is_locked: bool) -> i64 {
    if is_locked {
        LOCKED_FEE_PERCENT
    } else {
        FLEXIBLE_FEE_PERCENT
    }
}

// the early fee on an amount, rounded UP to a whole kobo
// i128 so a huge amount can never overflow while we multiply
pub fn calculate_withdrawal_fee(amount: i64, fee_percent: i64) -> i64 {
    let fee = (amount as i128 * fee_percent as i128 + 99) / 100;
    fee as i64 // no semicolon
}

// the biggest amount a plan can pay out when the fee is taken on top
// amount + fee must stay at or under the balance
pub fn calculate_max_withdrawal(balance: i64, fee_percent: i64) -> i64 {
    if balance <= 0 {
        return 0;
    }
    // first guess is balance / (1 + percent), then we nudge it to the exact edge
    let mut amount = (balance as i128 * 100 / (100 + fee_percent as i128)) as i64;
    while amount > 0 && amount + calculate_withdrawal_fee(amount, fee_percent) > balance {
        amount -= 1;
    }
    while amount + 1 + calculate_withdrawal_fee(amount + 1, fee_percent) <= balance {
        amount += 1;
    }
    amount // no semicolon
}

// only an active plan can move money
fn check_plan_is_active(plan: &LockedPlan) -> Result<(), PlanFundsError> {
    if plan.status != PLAN_STATUS_ACTIVE {
        return Err(PlanFundsError::PlanNotActive);
    }
    Ok(()) // no semicolon
}

// a locked plan cannot be partly withdrawn, it can only be closed whole
fn check_plan_is_not_locked(plan: &LockedPlan) -> Result<(), PlanFundsError> {
    if plan.lock_mode == PLAN_MODE_LOCKED {
        return Err(PlanFundsError::PlanLocked);
    }
    Ok(()) // no semicolon
}

// ---------------------------------------------------------------- add money

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
    check_plan_is_active(&plan)?;

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

    // one batch id for this whole tap, made once here and shared by every period row
    let batch_id = Uuid::new_v4();

    // one ledger row per period, each row shows the wallet balance right after that period
    let wallet_before_debit = wallet_after_debit + amount_to_move;
    for period_number in 1..=periods_to_move {
        let new_entry = NewLedgerEntry {
            user_id,
            entry_type: LedgerEntryType::PlanContribution,
            amount: plan.amount_per_frequency,
            fee: 0,
            plan_id: Some(plan_id),
            batch_id: Some(batch_id),
            remark: remark.clone(),
            reference: make_reference("PC"),
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

// ---------------------------------------------------------------- withdraw side

// takes amount + fee out of the plan, gives amount to the wallet, writes one plan_withdrawal row
// every exit from a plan (withdraw, close) goes through here, so the money movement is written once
async fn move_plan_money_to_wallet(
    tx: &mut Transaction<'_, Postgres>,
    plan_id: Uuid,
    user_id: Uuid,
    amount: i64,
    fee: i64,
    remark: Option<String>,
) -> Result<(), PlanFundsError> {
    let total_leaving_plan = amount
        .checked_add(fee)
        .ok_or(PlanFundsError::NotEnoughInPlan)?;

    // the guarded subtract refuses when the plan has less than amount + fee
    repo::subtract_from_plan_balance(tx, plan_id, total_leaving_plan)
        .await?
        .ok_or(PlanFundsError::NotEnoughInPlan)?;

    let wallet_balance_after = repo::credit_wallet(tx, user_id, amount)
        .await?
        .ok_or(PlanFundsError::WalletNotFound)?;

    // a withdrawal is one single row, so it has no batch
    let new_entry = NewLedgerEntry {
        user_id,
        entry_type: LedgerEntryType::PlanWithdrawal,
        amount,
        fee,
        plan_id: Some(plan_id),
        batch_id: None,
        remark,
        reference: make_reference("PW"),
        wallet_balance_after,
    };
    ledger_services::record_transaction(tx, new_entry).await?;
    Ok(()) // no semicolon
}

// takes part of the money out of a flexible plan, the fee is taken on top
// the plan loses amount + fee, the wallet receives amount
// locked plans cannot do this, they can only be closed whole
pub async fn withdraw_from_plan(
    pool: &PgPool,
    user_id: Uuid,
    plan_id: Uuid,
    amount: i64,
    remark: Option<String>,
) -> Result<WithdrawResult, PlanFundsError> {
    if amount <= 0 {
        return Err(PlanFundsError::InvalidAmount);
    }

    let mut tx: Transaction<'_, Postgres> = pool.begin().await?;

    let plan = repo::lock_plan_for_update(&mut tx, plan_id, user_id)
        .await?
        .ok_or(PlanFundsError::PlanNotFound)?;

    check_plan_is_active(&plan)?;
    check_plan_is_not_locked(&plan)?;

    let fee = calculate_withdrawal_fee(amount, early_fee_percent(false));
    let total_leaving_plan = amount
        .checked_add(fee)
        .ok_or(PlanFundsError::NotEnoughInPlan)?;
    if total_leaving_plan > plan.balance {
        return Err(PlanFundsError::NotEnoughInPlan);
    }

    move_plan_money_to_wallet(&mut tx, plan_id, user_id, amount, fee, remark).await?;

    tx.commit().await?;

    Ok(WithdrawResult { amount_received: amount, fee }) // no semicolon
}

// closes a plan: the whole balance goes to the wallet (fee on top), then the plan is closed
// flexible plans pay the lower fee, locked plans the higher one
// an empty plan just closes, with no fee and no ledger row
pub async fn liquidate_plan(
    pool: &PgPool,
    user_id: Uuid,
    plan_id: Uuid,
) -> Result<WithdrawResult, PlanFundsError> {
    let mut tx: Transaction<'_, Postgres> = pool.begin().await?;

    let plan = repo::lock_plan_for_update(&mut tx, plan_id, user_id)
        .await?
        .ok_or(PlanFundsError::PlanNotFound)?;

    check_plan_is_active(&plan)?;

    // nothing inside: close it, no money moves, so there is nothing to charge
    if plan.balance == 0 {
        repo::close_plan(&mut tx, plan_id).await?;
        tx.commit().await?;
        return Ok(WithdrawResult { amount_received: 0, fee: 0 });
    }

    // the wallet gets the biggest amount the fee allows, the fee is what is left
    // so the plan ends at exactly zero
    let fee_percent = early_fee_percent(plan.lock_mode == PLAN_MODE_LOCKED);
    let amount = calculate_max_withdrawal(plan.balance, fee_percent);
    if amount <= 0 {
        return Err(PlanFundsError::BalanceTooSmall);
    }
    let fee = plan.balance - amount;

    let remark = Some("Plan closed early".to_string());
    move_plan_money_to_wallet(&mut tx, plan_id, user_id, amount, fee, remark).await?;
    repo::close_plan(&mut tx, plan_id).await?;

    tx.commit().await?;

    Ok(WithdrawResult { amount_received: amount, fee }) // no semicolon
}

// ---------------------------------------------------------------- tests

#[cfg(test)]
mod tests {
    use super::*;

    // 3 percent of 10,000 naira (1,000,000 kobo) is 300 naira
    #[test]
    fn flexible_fee_is_three_percent() {
        assert_eq!(calculate_withdrawal_fee(1_000_000, 3), 30_000);
    }

    // 10 percent of 10,000 naira is 1,000 naira
    #[test]
    fn locked_fee_is_ten_percent() {
        assert_eq!(calculate_withdrawal_fee(1_000_000, 10), 100_000);
    }

    // 3 percent of 1,000,001 kobo is 30,000.03, so it rounds up to 30,001
    #[test]
    fn fee_rounds_up() {
        assert_eq!(calculate_withdrawal_fee(1_000_001, 3), 30_001);
        assert_eq!(calculate_withdrawal_fee(1_000_001, 10), 100_001);
    }

    // locked plans pay the higher percent, flexible the lower
    #[test]
    fn locked_pays_more_than_flexible() {
        assert_eq!(early_fee_percent(false), 3);
        assert_eq!(early_fee_percent(true), 10);
    }

    // a balance of 10,300 naira can pay out exactly 10,000 naira plus 300 fee
    #[test]
    fn max_withdrawal_matches_the_flexible_example() {
        assert_eq!(calculate_max_withdrawal(1_030_000, 3), 1_000_000);
    }

    // a balance of 11,000 naira can pay out exactly 10,000 naira plus 1,000 fee
    #[test]
    fn max_withdrawal_matches_the_locked_example() {
        assert_eq!(calculate_max_withdrawal(1_100_000, 10), 1_000_000);
    }

    // the max is the exact edge: it fits, and one kobo more does not
    #[test]
    fn max_withdrawal_is_the_exact_edge() {
        for percent in [3, 10] {
            for balance in [1, 2, 3, 99, 100, 101, 103, 5_000, 999_999, 1_030_000, 51_500_000] {
                let amount = calculate_max_withdrawal(balance, percent);
                assert!(amount + calculate_withdrawal_fee(amount, percent) <= balance);
                assert!(amount + 1 + calculate_withdrawal_fee(amount + 1, percent) > balance);
            }
        }
    }

    // closing a plan leaves nothing behind: amount + fee is the whole balance
    #[test]
    fn closing_leaves_zero() {
        for percent in [3, 10] {
            for balance in [2, 100, 4_000_000, 51_500_000] {
                let amount = calculate_max_withdrawal(balance, percent);
                let fee = balance - amount;
                assert_eq!(amount + fee, balance);
                assert!(fee >= calculate_withdrawal_fee(amount, percent));
            }
        }
    }

    // an empty plan pays nothing
    #[test]
    fn empty_plan_pays_nothing() {
        assert_eq!(calculate_max_withdrawal(0, 3), 0);
        assert_eq!(calculate_max_withdrawal(0, 10), 0);
    }
}
