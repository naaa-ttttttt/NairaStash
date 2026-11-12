use sqlx::{Postgres, Transaction};
use uuid::Uuid;

pub struct NewUser {
    user_id: Uuid, 
    name: String,
    email: String,
    hashed_password: String,
    phone: String,
}

pub struct NewWallet {
    wallet_id: Uuid,
    user_id: Uuid,
}

pub async fn create_user(
    transaction: &mut sqlx::Transaction<'_, Postgres>,
    new_user: NewUser,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO users 
        (user_id, name, email, phone, hashed_password)
        VALUES ($1, $2, $3, $4, $5) 
        "#
    )
    .bind(new_user.user_id)
    .bind(new_user.name)
    .bind(new_user.email)
    .bind(new_user.phone)
    .bind(new_user.hashed_password)
    .execute(&mut **transaction)
    .await?;

    Ok(())
}

pub async fn create_wallet(
    transaction: &mut sqlx::Transaction<'_, Postgres>,
    new_wallet: NewWallet,
) -> Result<(), sqlx::Error> {

}
