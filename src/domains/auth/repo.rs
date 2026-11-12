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

fn create_user(
    transaction: &mut sqlx::Transaction<'_, Postgres>,
    new_user: NewUser,
) -> Result<(), sqlx::Error> {

}
