use std::time::Duration;

use reqwest::{Client, Response, StatusCode, Url};
use serde::{Deserialize, Serialize};

use crate::domains::deposits::pay_stack::PaystackConfig;

// this file only talks to paystack, it knows nothing about our database or our rules
// it reuses PaystackConfig (the secret key) from deposits, so there is one place that loads the key

// the address every paystack call starts from
const PAYSTACK_BASE_URL: &str = "https://api.paystack.co";

// how long we wait for paystack before giving up, reqwest has no timeout of its own
const REQUEST_TIMEOUT_SECONDS: u64 = 15;

// how many banks we ask for in one page of the bank list
const BANKS_PER_PAGE: u32 = 100;

// the most pages we will read, so a bad cursor from paystack can never loop forever
const MAX_BANK_PAGES: u32 = 10;

// the two ways a paystack call can go wrong, the service decides what the user sees for each
#[derive(Debug)]
pub enum PaystackError {
    // paystack understood us and said no, for example the account number does not exist at that bank
    Rejected,
    // we could not get a usable answer: network failure, timeout, wrong secret key, rate limit, outage
    // the text is only for our logs
    Unavailable(String),
}

// lets us use ? on reqwest calls, a reqwest error becomes PaystackError::Unavailable
impl From<reqwest::Error> for PaystackError {
    fn from(err: reqwest::Error) -> Self {
        // without_url so the account number inside the url never lands in our logs
        PaystackError::Unavailable(err.without_url().to_string()) // no semicolon
    }
}

// one bank from paystack's list, we keep only what we use
#[derive(Debug, Clone, Deserialize)]
pub struct PaystackBank {
    pub name: String,
    pub code: String,
    // paystack marks banks it has switched off, a missing value counts as active
    active: Option<bool>,
}

// the shape of one page of the bank list: the banks, and the cursor for the next page
#[derive(Deserialize)]
struct BankListResponse {
    data: Vec<PaystackBank>,
    meta: Option<BankListMeta>,
}

#[derive(Deserialize)]
struct BankListMeta {
    // empty on the last page
    next: Option<String>,
}

// the shape of the answer when we resolve an account, we only need the name
#[derive(Deserialize)]
struct ResolveResponse {
    data: ResolveData,
}

#[derive(Deserialize)]
struct ResolveData {
    account_name: String,
}

// what we send to paystack to create a recipient
// type is a rust keyword, so the field is renamed when it is turned into json
#[derive(Serialize)]
struct CreateRecipientRequest<'a> {
    #[serde(rename = "type")]
    recipient_type: &'a str,
    name: &'a str,
    account_number: &'a str,
    bank_code: &'a str,
    currency: &'a str,
}

// the shape of the answer when a recipient is created, we only need its code
#[derive(Deserialize)]
struct CreateRecipientResponse {
    data: CreateRecipientData,
}

#[derive(Deserialize)]
struct CreateRecipientData {
    recipient_code: String,
}

// builds a full url from a path and its query values
// parse_with_params encodes the values, so nothing a user typed can change the url
fn build_url(path: &str, params: &[(&str, &str)]) -> Result<Url, PaystackError> {
    Url::parse_with_params(&format!("{PAYSTACK_BASE_URL}{path}"), params)
        .map_err(|err| PaystackError::Unavailable(err.to_string())) // no semicolon
}

// sends one GET to paystack with our secret key and the timeout
async fn send_get(
    client: &Client,
    config: &PaystackConfig,
    url: Url,
) -> Result<Response, PaystackError> {
    let response = client
        .get(url)
        .bearer_auth(&config.secret_key)
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECONDS))
        .send()
        .await?;

    Ok(response) // no semicolon
}

// gets every active nigerian bank from paystack, page by page, for the bank picker and the bank code check
// gives back an error if the list cannot be read in full, a half list would wrongly reject real banks
pub async fn list_banks(
    client: &Client,
    config: &PaystackConfig,
) -> Result<Vec<PaystackBank>, PaystackError> {
    let per_page = BANKS_PER_PAGE.to_string();
    let mut all_banks: Vec<PaystackBank> = Vec::new();
    // the cursor paystack gave us for the next page, none on the first page
    let mut cursor: Option<String> = None;

    for _page in 0..MAX_BANK_PAGES {
        let mut params = vec![
            ("country", "nigeria"),
            ("currency", "NGN"),
            ("use_cursor", "true"),
            ("perPage", per_page.as_str()),
        ];
        if let Some(next) = &cursor {
            params.push(("next", next.as_str()));
        }

        let url = build_url("/bank", &params)?;
        let response = send_get(client, config, url).await?;

        if !response.status().is_success() {
            return Err(PaystackError::Unavailable(format!(
                "bank list failed with status {}",
                response.status()
            )));
        }

        let page = response.json::<BankListResponse>().await?;

        // keep only banks paystack has not switched off
        all_banks.extend(
            page.data
                .into_iter()
                .filter(|bank| bank.active != Some(false)),
        );

        let next_cursor = page
            .meta
            .and_then(|meta| meta.next)
            .filter(|next| !next.is_empty() && next != "null");

        match next_cursor {
            Some(next) => cursor = Some(next),
            // no cursor means this was the last page
            None => return Ok(all_banks),
        }
    }

    Err(PaystackError::Unavailable(
        "bank list had too many pages".to_string(),
    )) // no semicolon
}

// asks paystack whose account this is, gives back the account name exactly as the bank holds it
// Rejected means the number does not exist at that bank, Unavailable means we could not find out
pub async fn resolve_account_name(
    client: &Client,
    config: &PaystackConfig,
    account_number: &str,
    bank_code: &str,
) -> Result<String, PaystackError> {
    let url = build_url(
        "/bank/resolve",
        &[("account_number", account_number), ("bank_code", bank_code)],
    )?;
    let response = send_get(client, config, url).await?;
    let status = response.status();

    // paystack answers a number it cannot find with a 4xx, that is a real "no", not an outage
    if status == StatusCode::UNPROCESSABLE_ENTITY
        || status == StatusCode::BAD_REQUEST
        || status == StatusCode::NOT_FOUND
    {
        return Err(PaystackError::Rejected);
    }

    // anything else that is not a success (401 wrong key, 429 too many calls, 5xx outage) is a problem on our side
    if !status.is_success() {
        return Err(PaystackError::Unavailable(format!(
            "resolve account failed with status {status}"
        )));
    }

    let parsed = response.json::<ResolveResponse>().await?;
    let account_name = parsed.data.account_name.trim().to_string();

    // a success with no name is no use to the user, treat it as not found
    if account_name.is_empty() {
        return Err(PaystackError::Rejected);
    }

    Ok(account_name) // no semicolon
}

// makes paystack remember this account as a place we can send money to, gives back its recipient code
// call it only after the user confirmed the resolved name
pub async fn create_transfer_recipient(
    client: &Client,
    config: &PaystackConfig,
    account_name: &str,
    account_number: &str,
    bank_code: &str,
) -> Result<String, PaystackError> {
    let payload = CreateRecipientRequest {
        // nuban is the type for normal nigerian bank accounts
        recipient_type: "nuban",
        name: account_name,
        account_number,
        bank_code,
        currency: "NGN",
    };

    let response = client
        .post(format!("{PAYSTACK_BASE_URL}/transferrecipient"))
        .bearer_auth(&config.secret_key)
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECONDS))
        .json(&payload)
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(PaystackError::Unavailable(format!(
            "create recipient failed with status {}",
            response.status()
        )));
    }

    let parsed = response.json::<CreateRecipientResponse>().await?;

    Ok(parsed.data.recipient_code) // no semicolon
}
