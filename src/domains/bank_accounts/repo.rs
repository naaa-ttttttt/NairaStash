use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool, Postgres, Transaction};
use uuid::Uuid;

// every function here takes &mut tx, so the service decides where the tx begins and ends, and this file has no rules, only sql
// the one exception is list_active_bank_accounts, it only reads, so it takes the pool

// the columns of a saved bank, status comes as text so we do not copy the enum
const BANK_ACCOUNT_COLUMNS: &str = "bank_account_id, user_id, bank_name, bank_code, \
    account_number, account_name, recipient_code, status::text AS status, \
    created_at, removed_at";
