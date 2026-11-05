use sqlx:::{Pool, Postgres, FromRow};
use uuid::Uuid;
use chrono::{DateTime, Utc};


#[derive(Debug, sqlx::Type)]
#[sqlx(type_name= "deposit_type", rename_all="lowercase")]
pub enum DepositTpye {
    Auto_debit,
    Manual,
}

#[derive(Debug, sqlx::Type)]
#[sqlx(type_name= "transation_status", rename_all="lowercase")]
pub enum TransactionStatus {
    Successful, 
    Pending,
    Failed,
}
#[derive(Debug, FromRow)]
pub struct DepositRecord {
    pub deposit_id: uuid,
    pub user_id: uuid,
    pub plan_id: uuid,
    pub amount: i64, 
    pub status: TransactionStatus,
    pub create_at: DateTime<Utc>,
    pub deposit_type: DepositTpye,
}

pub async fn create_pending_deposits(
    pool: &Pool<Postgres>,
    user_id: Uuid, 
    plan_id: Uuid, 
    amount: i64,
    deposit_type: DepositTpye,
) -> Result<DepositRecord, sqlx::Error> {
    sqlx::query_as::<_, DepositRecord>(
        r#"
        INSERT INTO deposits (user_id, plan_id, amount, deposit_type, status)
        VALUES ($1, $2, $3, $4, 'pending')
        RETURNING deposit_id, user_id, plan_id, amount, status, create_at, deposit_type
        "#
    )    
    .bind(user_id)
    .bind(plan_id)
    .bind(amount)
    .bind(deposit_type)
    .fetch_one(pool)
    .await?
}

pub async fn get_deposit_by_reference(
    pool: &Pool<Postgres>,
    reference: &str,
) -> Result<Option<DepositRecord, sqlx::Error>> {
    sqlx::query_as::<_, DepositRecord>(
        r#"
        SELECT deposit_id, user_id, plan_id, amount, status, create_at, deposit_type
        FROM deposits   
        WHERE reference = $1
        "#
    )

    .bind(reference)
    .fetch_optional(pool)
    .await?
}


pub async fn update_deposits_status(
    pool: &Pool<Postgres>,
    reference: &str,
    status: TransactionStatus,
) -> Result<DepositRecord, sqlx::Error> {
    sqlx::query_as::<_, DepositRecord>(
        r#"
        UPDATE deposits 
        SET status = $1 
        WHERE reference = $2
        RETURNING deposit_id, user_id, plan_id, amount, status, create_at, deposit_type
        "#
    )

    .bind(status)
    .bind(reference)
    .fetch_one(pool)
    .await?
}

