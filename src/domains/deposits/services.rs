use reqwest::Client;
use sqlx::PgPool;
use uuid::Uuid;

use super::pay_stack::{initialize_payment, PaystackConfig};
use super::repo::{
    claim_pending_deposit, create_pending_deposits, get_deposit_by_reference, get_user_email,
    update_wallet_balance, DepositRecord, DepositType, TransactionStatus,
};

use crate::domains::ledger::repo::{LedgerEntryType, NewLedgerEntry};
use crate::domains::ledger::services::{record_transaction, LedgerError};

const MIN_DEPOSIT_NAIRA: i64 = 100;

// ---------------------------------------------------------------- starting a deposit

#[derive(Debug)]
pub enum StartDepositError {
    AmountTooSmall,
    AmountTooLarge,
    UserNotFound,
    Database(sqlx::Error),
    Paystack(reqwest::Error),
}

impl From<sqlx::Error> for StartDepositError {
    fn from(err: sqlx::Error) -> Self {
        StartDepositError::Database(err)
    }
}

// lets us use ? on ledger calls, a LedgerError becomes DepositError::Ledger
impl From<LedgerError> for DepositError {
    fn from(err: LedgerError) -> Self {
        DepositError::Ledger(err)
    }
}

// checks the amount, saves a pending deposit, asks paystack for a checkout link
pub async fn start_deposit(
    pool: &PgPool,
    client: &Client,
    paystack: &PaystackConfig,
    user_id: Uuid,
    amount_in_naira: i64,
) -> Result<String, StartDepositError> {
    if amount_in_naira < MIN_DEPOSIT_NAIRA {
        return Err(StartDepositError::AmountTooSmall);
    }

    // checked_mul so a huge number cannot overflow
    let amount_in_kobo = amount_in_naira
        .checked_mul(100)
        .ok_or(StartDepositError::AmountTooLarge)?;

    let email = get_user_email(pool, user_id)
        .await?
        .ok_or(StartDepositError::UserNotFound)?;

    let reference = format!("NS-DEP-{}", &Uuid::new_v4().to_string()[..8]);

    create_pending_deposits(pool, user_id, &reference, amount_in_kobo, DepositType::Manual).await?;

    let checkout_url = initialize_payment(client, &email, amount_in_kobo, &reference, paystack)
        .await
        .map_err(StartDepositError::Paystack)?;

    Ok(checkout_url) // no semicolon
}

// ---------------------------------------------------------------- settling a deposit

#[derive(Debug)]
pub enum DepositError {
    NotFound,
    AmountMismatch,
    InvalidCurrency,
    InvalidStatus,
    WalletNotFound,
    DatabaseError(sqlx::Error),
    Ledger(LedgerError),
}

impl From<sqlx::Error> for DepositError {
    fn from(err: sqlx::Error) -> Self {
        DepositError::DatabaseError(err)
    }
}

// the facts of a paystack webhook that the rules care about, no http types in here
pub struct WebhookFacts {
    pub event: String,
    pub status: String,
    pub reference: String,
    pub amount: i64,
    pub currency: String,
}

// finds the deposit this webhook is about, or says it does not exist
async fn load_deposit(pool: &PgPool, reference: &str) -> Result<DepositRecord, DepositError> {
    let found_deposit = get_deposit_by_reference(pool, reference).await?;

    match found_deposit {
        Some(deposit) => Ok(deposit),
        None => Err(DepositError::NotFound),
    }
}

// checks that what paystack says matches what we saved
fn check_facts_match_deposit(
    facts: &WebhookFacts,
    deposit: &DepositRecord,
) -> Result<(), DepositError> {
    if facts.currency != "NGN" {
        return Err(DepositError::InvalidCurrency);
    }

    if facts.amount != deposit.amount {
        return Err(DepositError::AmountMismatch);
    }

    Ok(()) // no semicolon
}

// marks the deposit successful, credits the wallet and writes the ledger row, all in one transaction
async fn settle_successful_deposit(pool: &PgPool, reference: &str) -> Result<(), DepositError> {
    let mut tx = pool.begin().await?;

    let claimed = claim_pending_deposit(&mut tx, reference, TransactionStatus::Successful).await?;

    // none means another webhook already settled it, tx rolls back when it is dropped
    let claimed_deposit = match claimed {
        Some(deposit) => deposit,
        None => return Ok(()),
    };

    // the amount comes from our own row, not from the payload
    let new_balance =
        update_wallet_balance(&mut tx, claimed_deposit.user_id, claimed_deposit.amount).await?;

    // no wallet means no credit, so the whole transaction is thrown away
    // and the deposit stays pending, so a retry can work once the wallet exists
    let Some(wallet_balance_after) = new_balance else {
        return Err(DepositError::WalletNotFound);
    };

    // the ledger row is written on the same tx, so it is saved only if the credit is
    // a deposit is one single row, so it has no batch
    let ledger_row = NewLedgerEntry {
        fee: 0,
        user_id: claimed_deposit.user_id,
        entry_type: LedgerEntryType::Deposit,
        amount: claimed_deposit.amount,
        plan_id: None,
        batch_id: None,
        remark: None,
        reference: claimed_deposit.reference.clone(),
        wallet_balance_after,
    };
    record_transaction(&mut tx, ledger_row).await?;

    tx.commit().await?;

    Ok(()) // no semicolon
}

pub async fn process_deposit_webhook(
    pool: &PgPool,
    facts: WebhookFacts,
) -> Result<(), DepositError> {
    // only successful charges matter here
    if facts.event != "charge.success" {
        return Ok(());
    }

    if facts.status != "success" {
        return Err(DepositError::InvalidStatus);
    }

    let deposit = load_deposit(pool, &facts.reference).await?;

    // quick exit for a repeated webhook, the real guard is inside settle_successful_deposit
    if deposit.status != TransactionStatus::Pending {
        return Ok(());
    }

    check_facts_match_deposit(&facts, &deposit)?;

    settle_successful_deposit(pool, &deposit.reference).await // no semicolon
}
