use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use reqwest::Client;
use sqlx::PgPool;
use uuid::Uuid;

use super::pay_stack::{self, PaystackBank, PaystackError};
use super::repo::{self, BankAccount, NewBankAccount};
use crate::domains::deposits::pay_stack::PaystackConfig;

// the rules of saved banks live here: what is allowed, in what order, and what error to give
// the service decides where the db transaction begins and ends, the repo only runs sql inside it

// the most active saved banks one user can have, this is the only place the number lives
pub const MAX_SAVED_BANKS: i64 = 5;

// a nigerian bank account number is always exactly this many digits
const ACCOUNT_NUMBER_LENGTH: usize = 10;

// how long the bank list is kept in memory before we ask paystack again, 6 hours
const BANK_LIST_CACHE_SECONDS: u64 = 6 * 60 * 60;

// the bank list and the moment we got it, shared by every request
// the lock is only held for a quick read or write, never while waiting on paystack
static BANK_CACHE: OnceLock<Mutex<Option<(Instant, Vec<PaystackBank>)>>> = OnceLock::new();

// everything that can go wrong when saving, resolving or removing a bank
#[derive(Debug)]
pub enum BankAccountError {
    // not exactly 10 digits, covers too short, too long and letters
    InvalidAccountNumber,
    // the bank code is not in paystack's list
    UnknownBank,
    // paystack looked and that number does not exist at that bank
    AccountNotFound,
    // we could not reach paystack or it failed, the user should try again later
    PaystackUnavailable,
    // the user is already at MAX_SAVED_BANKS
    TooManyBanks,
    // this user already has this account saved and active
    AlreadySaved,
    // the bank does not exist, was removed, or is someone else's, one message for all three
    BankAccountNotFound,
    // the logged in user has no row in users
    UserNotFound,
    // anything unexpected from the database
    Database(sqlx::Error),
}

// lets us use ? on repo calls, a sqlx error becomes BankAccountError::Database
impl From<sqlx::Error> for BankAccountError {
    fn from(err: sqlx::Error) -> Self {
        BankAccountError::Database(err) // no semicolon
    }
}

// a bank the user can pick from the list on the add-bank page
#[derive(Debug, Clone)]
pub struct BankOption {
    pub bank_name: String,
    pub bank_code: String,
}

// what the confirm screen shows: the account paystack found, the user must say it is theirs
#[derive(Debug, Clone)]
pub struct ResolvedAccount {
    pub bank_name: String,
    pub bank_code: String,
    pub account_number: String,
    pub account_name: String,
}

// turns a paystack failure into the user-facing error and logs the detail for us
fn paystack_failed(err: PaystackError) -> BankAccountError {
    eprintln!("Paystack bank call failed: {:?}", err);
    BankAccountError::PaystackUnavailable // no semicolon
}

// gives back the cached bank list when it is still fresh, none when it is missing or too old
fn read_bank_cache() -> Option<Vec<PaystackBank>> {
    let cache = BANK_CACHE.get_or_init(|| Mutex::new(None));
    let guard = cache.lock().ok()?;
    let (fetched_at, banks) = guard.as_ref()?;

    if fetched_at.elapsed() < Duration::from_secs(BANK_LIST_CACHE_SECONDS) {
        Some(banks.clone())
    } else {
        None
    } // no semicolon
}

// keeps a fresh copy of the bank list with the time we got it
fn store_bank_cache(banks: &[PaystackBank]) {
    let cache = BANK_CACHE.get_or_init(|| Mutex::new(None));

    if let Ok(mut guard) = cache.lock() {
        *guard = Some((Instant::now(), banks.to_vec()));
    }
}

// the bank list from memory when fresh, otherwise from paystack (and then kept in memory)
// a failed fetch is never stored, so the next request tries again
async fn list_banks_cached(
    client: &Client,
    paystack: &PaystackConfig,
) -> Result<Vec<PaystackBank>, PaystackError> {
    if let Some(banks) = read_bank_cache() {
        return Ok(banks);
    }

    let mut banks = pay_stack::list_banks(client, paystack).await?;
    pay_stack::add_test_bank_if_missing(&mut banks, paystack);
    store_bank_cache(&banks);

    Ok(banks) // no semicolon
}

// trims the typed account number and checks it is exactly 10 digits
fn clean_account_number(raw: &str) -> Result<String, BankAccountError> {
    let trimmed = raw.trim();
    let all_digits = trimmed.chars().all(|character| character.is_ascii_digit());

    if trimmed.len() != ACCOUNT_NUMBER_LENGTH || !all_digits {
        return Err(BankAccountError::InvalidAccountNumber);
    }

    Ok(trimmed.to_string()) // no semicolon
}

// true when an insert failed because the same account is already saved and active for this user
fn is_duplicate_error(err: &sqlx::Error) -> bool {
    match err {
        sqlx::Error::Database(db_err) => db_err.is_unique_violation(),
        _ => false,
    }
}

// the banks for the picker, sorted by name
pub async fn list_available_banks(
    client: &Client,
    paystack: &PaystackConfig,
) -> Result<Vec<BankOption>, BankAccountError> {
    let banks = list_banks_cached(client, paystack)
        .await
        .map_err(paystack_failed)?;

    let mut options: Vec<BankOption> = banks
        .into_iter()
        .map(|bank| BankOption {
            bank_name: bank.name,
            bank_code: bank.code,
        })
        .collect();

    options.sort_by(|first, second| first.bank_name.cmp(&second.bank_name));

    Ok(options) // no semicolon
}

// step 1 of adding a bank: checks the input and asks paystack whose account it is
// writes nothing, it only gives the confirm screen (and the live name lookup) what they need
pub async fn resolve_bank_account(
    client: &Client,
    paystack: &PaystackConfig,
    bank_code: &str,
    account_number: &str,
) -> Result<ResolvedAccount, BankAccountError> {
    let account_number = clean_account_number(account_number)?;
    let bank_code = bank_code.trim();

    // the bank name comes from paystack's list, never from the browser
    // a bank code that is not in the list is a fake or a switched off bank
    let banks = list_banks_cached(client, paystack)
        .await
        .map_err(paystack_failed)?;
    let bank = banks
        .into_iter()
        .find(|bank| bank.code == bank_code)
        .ok_or(BankAccountError::UnknownBank)?;

    let account_name =
        pay_stack::resolve_account_name(client, paystack, &account_number, &bank.code)
            .await
            .map_err(|err| match err {
                // paystack said the number does not exist, tell the user
                PaystackError::Rejected => BankAccountError::AccountNotFound,
                // anything else is our problem or paystack's, not the user's typing
                other => paystack_failed(other),
            })?;

    Ok(ResolvedAccount {
        bank_name: bank.name,
        bank_code: bank.code,
        account_number,
        account_name,
    }) // no semicolon
}

// step 2 of adding a bank: runs after the user confirmed the name
// it resolves again, because the name sent back from the browser could have been edited
// the paystack calls come first and the db transaction last, so we never hold a lock while waiting on paystack
pub async fn save_bank_account(
    pool: &PgPool,
    client: &Client,
    paystack: &PaystackConfig,
    user_id: Uuid,
    bank_code: &str,
    account_number: &str,
) -> Result<BankAccount, BankAccountError> {
    // the name we save is the one paystack just gave us, not one from the form
    let resolved = resolve_bank_account(client, paystack, bank_code, account_number).await?;

    let recipient_code = pay_stack::create_transfer_recipient(
        client,
        paystack,
        &resolved.account_name,
        &resolved.account_number,
        &resolved.bank_code,
    )
    .await
    .map_err(paystack_failed)?;

    // the table has no default for the id, so we make it here
    let new_bank = NewBankAccount {
        bank_account_id: Uuid::new_v4(),
        user_id,
        bank_name: resolved.bank_name,
        bank_code: resolved.bank_code,
        account_number: resolved.account_number,
        account_name: resolved.account_name,
        recipient_code,
    };

    let mut tx = pool.begin().await?;

    // lock first: a second save by the same user waits here until this one finishes
    let user_exists = repo::lock_user_for_bank_change(&mut tx, user_id).await?;
    if !user_exists {
        return Err(BankAccountError::UserNotFound);
    }

    // count after the lock, so the number cannot change underneath us
    let active_count = repo::count_active_bank_accounts(&mut tx, user_id).await?;
    if active_count >= MAX_SAVED_BANKS {
        return Err(BankAccountError::TooManyBanks);
    }

    // the unique index is the real guard against duplicates, we only translate its error
    let saved_bank = match repo::insert_bank_account(&mut tx, &new_bank).await {
        Ok(row) => row,
        Err(err) if is_duplicate_error(&err) => return Err(BankAccountError::AlreadySaved),
        Err(err) => return Err(err.into()),
    };

    tx.commit().await?;

    // a recipient left behind at paystack by an early return above is harmless, it is never used
    Ok(saved_bank) // no semicolon
}

// all active saved banks of this user, for the banks page and the payout picker
pub async fn list_bank_accounts(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Vec<BankAccount>, BankAccountError> {
    let banks = repo::list_active_bank_accounts(pool, user_id).await?;

    Ok(banks) // no semicolon
}

// soft delete: the row stays so payout history keeps its link, it just stops being active
pub async fn remove_bank_account(
    pool: &PgPool,
    user_id: Uuid,
    bank_account_id: Uuid,
) -> Result<(), BankAccountError> {
    let mut tx = pool.begin().await?;

    let removed = repo::mark_bank_account_removed(&mut tx, bank_account_id, user_id).await?;

    // nothing changed means it does not exist, was already removed, or is not theirs
    if !removed {
        return Err(BankAccountError::BankAccountNotFound);
    }

    tx.commit().await?;

    Ok(()) // no semicolon
}
