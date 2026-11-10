use std::env;
use reqwest::Client;
use hmac::{Hmac, Mac, KeyInit};
use sha2::Sha512;
use subtle::ConstantTimeEq;

#[derive(serde::Serialize)]
pub struct InitializePaymentRequest<'a> {
    pub email: &'a str,
    pub amount: i64,
    pub reference: &'a str,
    pub callback_url: &'a str,
}

#[derive(serde::Deserialize)]
pub struct PaystackResponse {
    pub status: bool,
    pub message: String,
    pub data: PaystackResponseData,
}

#[derive(serde::Deserialize)]
pub struct PaystackResponseData {
    pub authorization_url: String,
}

// where paystack sends the user's browser after paying
fn read_callback_url() -> String {
    let default_url = "http://127.0.0.1:3000/".to_string();
    env::var("APP_CALLBACK_URL").unwrap_or(default_url) // no semicolon
}

pub async fn initialize_payment(
    email: &str,
    amount: i64,
    reference: &str,
) -> Result<String, reqwest::Error> {
    let secret_key = env::var("PAYSTACK_SECRET_KEY")
        .unwrap_or_else(|_| "sk_test_placeholder".to_string());

    let callback_url = read_callback_url();

    let payload = InitializePaymentRequest {
        email,
        amount,
        reference,
        callback_url: &callback_url,
    };

    let client = Client::new();

    // FIXED API ENDPOINT PATH:
    let response = client
        .post("https://api.paystack.co/transaction/initialize")
        .header("Authorization", format!("Bearer {}", secret_key))
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await?
        .error_for_status()?;

    let parsed_response = response.json::<PaystackResponse>().await?;

    Ok(parsed_response.data.authorization_url)
}

pub fn verify_paystack_signature(
    payload_bytes: &[u8],
    header_signature: &str,
    secret_key: &str,
) -> bool {
    type HmacSha512 = Hmac<Sha512>;

    let mut mac = HmacSha512::new_from_slice(secret_key.as_bytes())
        .expect("HMAC can take key of any size");

    mac.update(payload_bytes);

    let result = mac.finalize();
    let code_bytes = result.into_bytes();

    let my_hex_string = hex::encode(code_bytes);

    let equality_check = my_hex_string.as_bytes().ct_eq(header_signature.as_bytes());

    equality_check.into()
}
