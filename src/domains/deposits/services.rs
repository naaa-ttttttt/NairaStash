use super::handlers::PaystackWebhook;
use super::repo::{TransactionStatus, update_deposits_status, update_wallet_balance};
use sqlx::PgPool;

#[derive(Debug)]
pub enum DepositError {
    NotFound,
    AmountMismatch,
    InvalidCurrency,
    InvalidStatus,
    DatabaseError(sqlx::Error),
}

impl From<sqlx::Error> for DepositError {
    fn from(err: sqlx::Error) -> Self {
        DepositError::DatabaseError(err)
    }
}

pub async fn process_deposit_webhook(
    pool: &PgPool,
    payload: PaystackWebhook,
) -> Result<(), DepositError> {
    // 1. Filter out any non-success event hooks early
    if payload.event != "charge.success" {
        return Ok(());
    }

    // 2. Idempotency Check: Query the database for the tracking reference key
    let deposits = match super::repo::get_deposit_by_reference(pool, &payload.data.reference).await? {
        Some(record) => record,
        None => return Err(DepositError::NotFound),
    };

    // 3. Idempotency Guard: If the transaction status isn't Pending, exit early to prevent double-crediting
    if deposits.status != TransactionStatus::Pending {
        return Ok(());
    }

    // 4. Validate Amount and Currency Policies
    if payload.data.currency != "NGN" {
        return Err(DepositError::InvalidCurrency);
    }

    if payload.data.amount != deposits.amount {
        return Err(DepositError::AmountMismatch);
    }

    // 5. Atomic Transaction Lifecycle Orchestration
    match payload.data.status.as_str() {
        "success" => {
            let mut tx = pool.begin().await?;

            // Mark deposit status as Successful
            update_deposits_status(&mut tx, &payload.data.reference, TransactionStatus::Successful).await?;

            // Inline atomic assignment increment to wallet balance
            update_wallet_balance(&mut tx, deposits.user_id, payload.data.amount).await?;

            tx.commit().await?;
        }
        "failed" => {
            let mut tx = pool.begin().await?;

            // Mark deposit status as Failed
            update_deposits_status(&mut tx, &payload.data.reference, TransactionStatus::Failed).await?;

            tx.commit().await?;
        }
        _ => return Err(DepositError::InvalidStatus),
    }

    Ok(())
}

