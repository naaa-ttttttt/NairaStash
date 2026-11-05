use super::handlers::PaystackWebhook;
use sqlx::PgPool;

#[derive(Debug)]
pub enum DepositError {
   NotFound,
   AmountMismatch,
   InvalidCurrency,
   DatabaseError(sqlx::Error),
}

pub async fn process_deposit_webhook(
    pool: &PgPool,
    payload: PaystackWebhook,
) -> Result<(), String> {
    if payload.event != "charge.success" {
        return Ok(());
    }

    Ok(())
}
