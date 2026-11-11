use super::handlers::PaystackWebhook;
use super::repo::{
    claim_pending_deposit, get_deposit_by_reference, update_wallet_balance,
    DepositRecord, TransactionStatus,
};
use sqlx::PgPool;

#[derive(Debug)]
pub enum DepositError {
    NotFound,
    AmountMismatch,
    InvalidCurrency,
    InvalidStatus,
    WalletNotFound,
    DatabaseError(sqlx::Error),
}

impl From<sqlx::Error> for DepositError {
    fn from(err: sqlx::Error) -> Self {
        DepositError::DatabaseError(err)
    }
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
fn check_payload_matches_deposit(
    payload: &PaystackWebhook,
    deposit: &DepositRecord,
) -> Result<(), DepositError> {
    if payload.data.currency != "NGN" {
        return Err(DepositError::InvalidCurrency);
    }

    if payload.data.amount != deposit.amount {
        return Err(DepositError::AmountMismatch);
    }

    Ok(()) // no semicolon
}

// marks the deposit successful and credits the wallet, all in one transaction
async fn settle_successful_deposit(pool: &PgPool, reference: &str) -> Result<(), DepositError> {
    let mut tx = pool.begin().await?;

    let claimed = claim_pending_deposit(&mut tx, reference, TransactionStatus::Successful).await?;

    // none means another webhook already settled it, tx rolls back when it is dropped
    let claimed_deposit = match claimed {
        Some(deposit) => deposit,
        None => return Ok(()),
    };

    // the amount comes from our own row, not from the payload
    let wallet_was_credited =
        update_wallet_balance(&mut tx, claimed_deposit.user_id, claimed_deposit.amount).await?;

    // no wallet means no credit, so the whole transaction is thrown away
    // and the deposit stays pending, so a retry can work once the wallet exists
    if !wallet_was_credited {
        return Err(DepositError::WalletNotFound);
    }

    tx.commit().await?;

    Ok(()) // no semicolon
}

pub async fn process_deposit_webhook(
    pool: &PgPool,
    payload: PaystackWebhook,
) -> Result<(), DepositError> {
    // only successful charges matter here
    if payload.event != "charge.success" {
        return Ok(());
    }

    if payload.data.status != "success" {
        return Err(DepositError::InvalidStatus);
    }

    let deposit = load_deposit(pool, &payload.data.reference).await?;

    // quick exit for a repeated webhook, the real guard is inside settle_successful_deposit
    if deposit.status != TransactionStatus::Pending {
        return Ok(());
    }

    check_payload_matches_deposit(&payload, &deposit)?;

    settle_successful_deposit(pool, &deposit.reference).await // no semicolon
}
