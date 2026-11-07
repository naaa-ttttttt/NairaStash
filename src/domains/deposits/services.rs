use super::handlers::PaystackWebhook;
use super::repo::TransactionStatus;
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
    if payload.event != "charge.success" {
        return Ok(());
    }


    //idempotency check, for reference
    let deposits = match super::repo::get_deposit_by_reference(pool, &payload.data.reference).await? {
        Some(record) => record,
        None => return Err(DepositError::NotFound),
    }

    // a check if the idempotency key has been processed or not
    if deposits.status != TransactionStatus::Pending {
        return Ok(());
    }

    if payload.data.status != "Success" {
        return Err(DepositError::InvalidStatus);
    }

    if payload.data.currency != "NGN" {
        return Err(DepositError::InvalidCurrency);
    }

    if payload.data.amount != deposits.amount {
        return Err(DepositError::AmountMismatch)
    }

    match payload.data.status.as_str() {
        "success" => {
            update_deposits_status(
                pool,
                &payload.data.reference,
                TransactionStatus::Successful,
            ).await?;
        }
        "failed" => {
            Update_deposits_status(     
                pool,
                &payload.data.reference,
                TransactionStatus::Failed,
            ).await?;
        }
        _ => return Err(DepositError::InvalidStatus),
    }
    
    Ok(())
}
