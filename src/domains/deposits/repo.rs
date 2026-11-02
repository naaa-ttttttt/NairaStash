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
pub struct DatabaseRecords {
    pub deposit_id: uuid,
    pub user_id: uuid,
    pub plan_id: uuid,
    pub amount: i64, 
    pub status: TransactionStatus,
    pub create_at: DateTime<Utc>,
    pub deposit_type: DepositTpye,
}
