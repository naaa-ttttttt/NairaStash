use argon2::password_hash::{rand_core::OsRng, SaltString};
use argon2::{Argon2, PasswordHasher};
use sqlx::PgPool;
use uuid::Uuid;

use super::repo::{create_user, create_wallet, NewUser, NewWallet};

pub struct SignupInput {
    name: String,
    email: String,
    phone: String,
    password: String,
}

pub enum SignupError {
    InvalidName,
    InvalidEmail,
    InvalidPhone,
    WeakPassword,
    AccountExist,
}

