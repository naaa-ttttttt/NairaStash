use chrono::{DateTime, Utc};
use reqwest::Client;
use sqlx::PgPool;
use uuid::Uuid;

use super::pay_stack::{self, TransferState, TransferStart};
use super::repo::{self, NewPayout, PayoutListItem};
use crate::domains::bank_accounts::repo as bank_repo;
use crate::domains::deposits::pay_stack::PaystackConfig;
use crate::domains::ledger::repo::{LedgerEntryType, NewLedgerEntry};
use crate::domains::ledger::services::{record_transaction, LedgerError};
use crate::domains::payout_pin::services::{check_pin, PinCheck, PinError};
use crate::domains::plan_funds::repo::{credit_wallet, debit_wallet_if_enough};

// the rules of payouts live here: what is allowed, in what order, and what happens when something fails
// the service decides where each db transaction begins and ends, the repo only runs sql inside it
// every amount in this file is in kobo, the constants named NAIRA are converted when they are used

// the smallest payout, in naira, this is the only place the number lives
pub const MIN_PAYOUT_NAIRA: i64 = 1_000;

// the most one user can send in one Lagos day, in naira, KYC will raise it later
pub const DAILY_PAYOUT_LIMIT_NAIRA: i64 = 200_000;

// a payout still pending after this many minutes is asked about at paystack
const STUCK_AFTER_MINUTES: i32 = 10;

// how many stuck payouts one run of the job looks at
const STUCK_BATCH_SIZE: i64 = 20;

// how often the stuck-payout job runs, main.rs uses it
pub const STUCK_JOB_INTERVAL_SECONDS: u64 = 60;

// how many payouts the payouts page shows
const RECENT_PAYOUTS_LIMIT: i64 = 10;

// paystack's transfer fee, by the size of the transfer, check paystack's pricing page when they change
const TRANSFER_FEE_UP_TO_5K_KOBO: i64 = 10 * 100;
const TRANSFER_FEE_UP_TO_50K_KOBO: i64 = 25 * 100;
const TRANSFER_FEE_ABOVE_50K_KOBO: i64 = 50 * 100;
const FEE_BAND_5K_KOBO: i64 = 5_000 * 100;
const FEE_BAND_50K_KOBO: i64 = 50_000 * 100;

// the government stamp duty on transfers of this size or more, it is charged on top of the transfer fee
const STAMP_DUTY_KOBO: i64 = 50 * 100;
const STAMP_DUTY_FROM_KOBO: i64 = 10_000 * 100;

// everything that can go wrong with a payout
#[derive(Debug)]
pub enum PayoutError {
    // below the minimum
    AmountTooSmall,
    // the amount plus the fee does not fit in a number
    AmountTooLarge,
    // the pin was not 4 digits
    InvalidPin,
    // the user has no pin yet
    PinNotSet,
    // the pin was wrong, the try is already counted
    WrongPin { tries_left: i32 },
    // too many wrong tries, the pin is locked until this time
    PinLocked { until: DateTime<Utc> },
    // the bank does not exist, was removed, or is someone else's
    BankNotFound,
    // the wallet cannot pay the amount and the fee
    NotEnoughMoney,
    // this payout would take today's total over the daily limit
    DailyLimitReached,
    // paystack refused to start the transfer, the money is already back in the wallet
    TransferRefused,
    // the wallet row to refund into is missing
    WalletNotFound,
    // a webhook said an amount that is not the amount we saved
    AmountMismatch,
    // a webhook said a currency that is not NGN
    InvalidCurrency,
    // an unexpected problem while checking the pin
    Pin(PinError),
    // anything unexpected from the database
    Database(sqlx::Error),
    // the ledger refused or failed to save the row
    Ledger(LedgerError),
}

// lets us use ? on repo calls, a sqlx error becomes PayoutError::Database
impl From<sqlx::Error> for PayoutError {
    fn from(err: sqlx::Error) -> Self {
        PayoutError::Database(err) // no semicolon
    }
}

// lets us use ? on ledger calls, a LedgerError becomes PayoutError::Ledger
impl From<LedgerError> for PayoutError {
    fn from(err: LedgerError) -> Self {
        PayoutError::Ledger(err) // no semicolon
    }
}

// how a payout request ended when it did not fail
#[derive(Debug)]
pub enum PayoutStart {
    // paystack took the transfer, it is on its way
    Started,
    // we could not tell if paystack got it, the payout stays pending and the job will find out
    // the money is held either way, it is never refunded on a guess
    Confirming,
}

// what the review page shows before the user types the pin
#[derive(Debug, Clone)]
pub struct PayoutReview {
    pub bank_account_id: Uuid,
    pub bank_name: String,
    pub account_name: String,
    pub account_number: String,
    // what the bank receives
    pub amount: i64,
    // what the user pays on top
    pub fee: i64,
    // what leaves the wallet
    pub total: i64,
}

// the facts of a transfer webhook that the rules care about, no http types in here
pub struct TransferFacts {
    pub event: String,
    pub reference: String,
    // paystack sends these, but a missing one must not make us drop the event
    pub amount: Option<i64>,
    pub currency: Option<String>,
}

// the three ways a transfer can end
#[derive(Debug, Clone, Copy, PartialEq)]
enum TransferOutcome {
    Success,
    Failed,
    Reversed,
}

// what a payout of this size costs the user on top: paystack's transfer fee plus the stamp duty when it applies
pub fn payout_fee(amount_in_kobo: i64) -> i64 {
    let transfer_fee = if amount_in_kobo <= FEE_BAND_5K_KOBO {
        TRANSFER_FEE_UP_TO_5K_KOBO
    } else if amount_in_kobo <= FEE_BAND_50K_KOBO {
        TRANSFER_FEE_UP_TO_50K_KOBO
    } else {
        TRANSFER_FEE_ABOVE_50K_KOBO
    };

    let stamp_duty = if amount_in_kobo >= STAMP_DUTY_FROM_KOBO {
        STAMP_DUTY_KOBO
    } else {
        0
    };

    transfer_fee + stamp_duty // no semicolon
}

// checks the amount and works out the fee and the total that leaves the wallet
fn check_amount(amount_in_kobo: i64) -> Result<(i64, i64), PayoutError> {
    if amount_in_kobo < MIN_PAYOUT_NAIRA * 100 {
        return Err(PayoutError::AmountTooSmall);
    }

    let fee = payout_fee(amount_in_kobo);

    // checked_add so a huge number cannot overflow
    let total = amount_in_kobo
        .checked_add(fee)
        .ok_or(PayoutError::AmountTooLarge)?;

    Ok((fee, total)) // no semicolon
}

// the latest payouts of this user for the payouts page
pub async fn list_recent_payouts(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Vec<PayoutListItem>, PayoutError> {
    let payouts = repo::list_user_payouts(pool, user_id, RECENT_PAYOUTS_LIMIT).await?;

    Ok(payouts) // no semicolon
}

// step 1: checks the amount and the bank and works out the fee, nothing is saved
// the review page shows it, request_payout works it all out again, nothing from the page is trusted
pub async fn review_payout(
    pool: &PgPool,
    user_id: Uuid,
    bank_account_id: Uuid,
    amount_in_kobo: i64,
) -> Result<PayoutReview, PayoutError> {
    let (fee, total) = check_amount(amount_in_kobo)?;

    let banks = bank_repo::list_active_bank_accounts(pool, user_id).await?;
    let bank = banks
        .into_iter()
        .find(|bank| bank.bank_account_id == bank_account_id)
        .ok_or(PayoutError::BankNotFound)?;

    Ok(PayoutReview {
        bank_account_id: bank.bank_account_id,
        bank_name: bank.bank_name,
        account_name: bank.account_name,
        account_number: bank.account_number,
        amount: amount_in_kobo,
        fee,
        total,
    }) // no semicolon
}

// step 2: holds the money and starts the transfer
// the pin is checked first and commits its own try count, so a failed payout cannot undo a wrong try
// the money is held (taken from the wallet) before paystack is called, so two payouts cannot spend the same money
pub async fn request_payout(
    pool: &PgPool,
    client: &Client,
    paystack: &PaystackConfig,
    user_id: Uuid,
    bank_account_id: Uuid,
    amount_in_kobo: i64,
    pin: &str,
) -> Result<PayoutStart, PayoutError> {
    let (fee, total) = check_amount(amount_in_kobo)?;

    // the pin comes before anything touches money
    match check_pin(pool, user_id, pin).await {
        Ok(PinCheck::Correct) => {}
        Ok(PinCheck::Wrong { tries_left }) => return Err(PayoutError::WrongPin { tries_left }),
        Ok(PinCheck::Locked { until }) => return Err(PayoutError::PinLocked { until }),
        Err(PinError::PinNotSet) => return Err(PayoutError::PinNotSet),
        Err(PinError::InvalidPinFormat) => return Err(PayoutError::InvalidPin),
        Err(other) => return Err(PayoutError::Pin(other)),
    }

    let mut tx = pool.begin().await?;

    let bank = bank_repo::find_active_bank_account(&mut tx, bank_account_id, user_id)
        .await?
        .ok_or(PayoutError::BankNotFound)?;

    // the hold: this takes the money out of the wallet and locks the wallet row until the tx ends
    // none means the wallet is too low
    let wallet_balance_after = debit_wallet_if_enough(&mut tx, user_id, total)
        .await?
        .ok_or(PayoutError::NotEnoughMoney)?;

    // the wallet row is locked now, so two payouts of the same user take turns here
    // if the limit is passed we return, the tx rolls back and the debit above is undone
    let sent_today = repo::sum_payouts_today(&mut tx, user_id).await?;
    let day_total = sent_today
        .checked_add(amount_in_kobo)
        .ok_or(PayoutError::AmountTooLarge)?;
    if day_total > DAILY_PAYOUT_LIMIT_NAIRA * 100 {
        return Err(PayoutError::DailyLimitReached);
    }

    // our own unique reference, paystack refuses a reference it has seen, so a payout can never be sent twice
    let reference = format!("ns-pay-{}", Uuid::new_v4().simple());

    let new_payout = NewPayout {
        payout_id: Uuid::new_v4(),
        user_id,
        bank_account_id: bank.bank_account_id,
        amount: amount_in_kobo,
        fee,
        reference,
        wallet_balance_after,
    };
    let payout = repo::insert_pending_payout(&mut tx, &new_payout).await?;

    tx.commit().await?;

    // from here the money is held and the payout is pending, now we ask paystack to send it
    let started = pay_stack::initiate_transfer(
        client,
        paystack,
        &bank.recipient_code,
        amount_in_kobo,
        &payout.reference,
        "NairaStash payout",
    )
    .await;

    match started {
        Ok(TransferStart::Accepted {
            transfer_code,
            status,
        }) => {
            // otp means paystack is waiting for someone to approve it, it will never finish by itself
            if status == "otp" {
                eprintln!(
                    "WARNING: paystack wants OTP approval for payout {}, turn OTP off for transfers",
                    payout.reference
                );
            }
            store_transfer_code(pool, payout.payout_id, &transfer_code).await;
            Ok(PayoutStart::Started)
        }
        Ok(TransferStart::Refused(why)) => {
            // paystack said no before creating anything, so the money can safely go back
            eprintln!("Paystack refused payout {}: {}", payout.reference, why);
            settle_as_failed(pool, &payout.reference, "paystack refused the transfer").await?;
            Err(PayoutError::TransferRefused)
        }
        Err(err) => {
            // we cannot tell if paystack created the transfer, so we do not refund
            // the payout stays pending and the stuck-payout job asks paystack by reference
            eprintln!("Payout {} has an unknown outcome: {:?}", payout.reference, err);
            Ok(PayoutStart::Confirming)
        }
    }
}

// keeps paystack's transfer code on the payout, a failure here is only logged, the money has already been sent
async fn store_transfer_code(pool: &PgPool, payout_id: Uuid, transfer_code: &str) {
    let result: Result<(), sqlx::Error> = async {
        let mut tx = pool.begin().await?;
        repo::attach_transfer_code(&mut tx, payout_id, transfer_code).await?;
        tx.commit().await?;
        Ok(())
    }
    .await;

    if let Err(err) = result {
        eprintln!("Could not save transfer code for payout {payout_id}: {:?}", err);
    }
}

// marks a payout successful and writes its ledger row, all in one transaction
// claim-once: if another call already settled it, this does nothing
async fn settle_as_success(pool: &PgPool, reference: &str) -> Result<(), PayoutError> {
    let mut tx = pool.begin().await?;

    let claimed = repo::claim_payout_as_success(&mut tx, reference).await?;

    // none means another webhook or the job already settled it, the tx rolls back when it is dropped
    let Some(payout) = claimed else {
        return Ok(());
    };

    // the ledger rule: amount matches the wallet movement, so it is everything that left the wallet
    // fee is the part of it that was the fee
    let total = payout
        .amount
        .checked_add(payout.fee)
        .ok_or(PayoutError::AmountTooLarge)?;

    let ledger_row = NewLedgerEntry {
        user_id: payout.user_id,
        entry_type: LedgerEntryType::Payout,
        amount: total,
        fee: payout.fee,
        plan_id: None,
        batch_id: None,
        remark: None,
        reference: payout.reference.clone(),
        // the balance right after the hold, not the balance now
        wallet_balance_after: payout.wallet_balance_after,
    };
    record_transaction(&mut tx, ledger_row).await?;

    tx.commit().await?;

    Ok(()) // no semicolon
}

// marks a payout failed and gives the held money back, all in one transaction
// claim-once: if another call already settled it, this does nothing, so the refund can never happen twice
async fn settle_as_failed(pool: &PgPool, reference: &str, reason: &str) -> Result<(), PayoutError> {
    let mut tx = pool.begin().await?;

    let claimed = repo::claim_payout_as_failed(&mut tx, reference, reason).await?;

    let Some(payout) = claimed else {
        return Ok(());
    };

    let total = payout
        .amount
        .checked_add(payout.fee)
        .ok_or(PayoutError::AmountTooLarge)?;

    // no wallet means no refund, so the whole transaction is thrown away and the payout stays pending
    // the job will try again
    let refunded = credit_wallet(&mut tx, payout.user_id, total).await?;
    if refunded.is_none() {
        return Err(PayoutError::WalletNotFound);
    }

    tx.commit().await?;

    Ok(()) // no semicolon
}

// handles a transfer webhook from paystack: transfer.success, transfer.failed or transfer.reversed
// the signature was already checked by the handler
pub async fn process_transfer_webhook(
    pool: &PgPool,
    facts: TransferFacts,
) -> Result<(), PayoutError> {
    let outcome = match facts.event.as_str() {
        "transfer.success" => TransferOutcome::Success,
        "transfer.failed" => TransferOutcome::Failed,
        "transfer.reversed" => TransferOutcome::Reversed,
        // any other event is not about payouts
        _ => return Ok(()),
    };

    let found = repo::get_payout_by_reference(pool, &facts.reference).await?;

    // not ours, for example a transfer made by hand from the paystack dashboard, answer ok so paystack stops sending it
    let Some(payout) = found else {
        eprintln!("Transfer webhook for an unknown reference: {}", facts.reference);
        return Ok(());
    };

    // what paystack says must match what we saved
    if let Some(amount) = facts.amount {
        if amount != payout.amount {
            return Err(PayoutError::AmountMismatch);
        }
    }
    if let Some(currency) = &facts.currency {
        if currency != "NGN" {
            return Err(PayoutError::InvalidCurrency);
        }
    }

    // already settled: a repeated event is fine, a contradicting one needs a human
    if payout.status != "pending" {
        let consistent = matches!(
            (outcome, payout.status.as_str()),
            (TransferOutcome::Success, "success")
                | (TransferOutcome::Failed, "failed")
                | (TransferOutcome::Reversed, "failed")
        );
        if !consistent {
            eprintln!(
                "CRITICAL: {} arrived for payout {} that is already {}, needs manual review",
                facts.event, payout.reference, payout.status
            );
        }
        return Ok(());
    }

    match outcome {
        TransferOutcome::Success => settle_as_success(pool, &payout.reference).await,
        TransferOutcome::Failed => {
            settle_as_failed(pool, &payout.reference, "paystack reported the transfer failed").await
        }
        TransferOutcome::Reversed => {
            settle_as_failed(pool, &payout.reference, "paystack reversed the transfer").await
        }
    } // no semicolon
}

// the stuck-payout job: asks paystack about payouts that stayed pending too long, and settles them from the answer
// it never starts a transfer, it only asks, so it can never send money twice
// a problem with one payout is logged and the job moves on to the next one
pub async fn settle_stuck_payouts(pool: &PgPool, client: &Client, paystack: &PaystackConfig) {
    let references =
        match repo::list_stale_pending_references(pool, STUCK_AFTER_MINUTES, STUCK_BATCH_SIZE).await
        {
            Ok(references) => references,
            Err(err) => {
                eprintln!("Stuck payouts: could not list them: {:?}", err);
                return;
            }
        };

    for reference in references {
        let state = match pay_stack::verify_transfer(client, paystack, &reference).await {
            Ok(state) => state,
            Err(err) => {
                // we could not find out, so we leave the payout alone until the next run
                eprintln!("Stuck payout {reference}: could not ask paystack: {:?}", err);
                continue;
            }
        };

        let result = match state {
            TransferState::Success => settle_as_success(pool, &reference).await,
            TransferState::Failed => {
                settle_as_failed(pool, &reference, "paystack reported the transfer failed").await
            }
            TransferState::Reversed => {
                settle_as_failed(pool, &reference, "paystack reversed the transfer").await
            }
            // paystack has no such transfer, so it was never created and the money can go back
            TransferState::NotFound => {
                settle_as_failed(pool, &reference, "paystack has no record of this transfer").await
            }
            // still in progress, ask again next time
            TransferState::Pending => Ok(()),
        };

        if let Err(err) = result {
            eprintln!("Stuck payout {reference}: could not settle it: {:?}", err);
        }
    }
}
