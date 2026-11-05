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


    let deposits = match super::repo::get_deposit_by_reference(pool, &payload.data.id).await? {
        Some(record) => record,
        None => return Err(DepositError::NotFound),
    }

    if deposits.status != TransactionStatus::Pending {
        return Ok(());
    }

    if payload.data.status != "Success" {
        return Err("Deposits failes".into());
    }

    if payload.data.currency != "NGN" {
        return Err(DepositError::InvalidCurrency);
    }

    if payload.data.amount != deposits.amount {
        return Err(DepositError::AmountMismatch)
    }



    Ok(())
}
