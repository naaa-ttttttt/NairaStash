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

#[derive(Clone)]
pub struct PaystackConfig {
    pub secret_key: String,
    pub callback_url: String
}

pub fn read_required_env(name: &str) -> Result<String, String> {
    // reading paystack secret key from the env file
    let raw_value = env::var(name).map_err(|_| format!("{} is not set", name))?;
    let trimmed_value = raw_value.trim().to_string();

    if trimmed_value.is_empty() {
        return Err(format!("{} is empty", name));
    }

    Ok(trimmed_value)
}

pub fn load_paystack_config() -> Result<PaystackConfig, String> {
    let secret_key = read_required_env("PAYSTACK_SECRET_KEY")?;
    let callback_url = read_required_env("APP_CALLBACK_URL")?;

    Ok(PaystackConfig { secret_key, callback_url })
}


pub async fn initialize_payment(
    email: &str,
    amount: i64,
    reference: &str,
    config: &PaystackConfig,
) -> Result<String, reqwest::Error> {
    let payload = InitializePaymentRequest {
        email,
        amount,
        reference,
        callback_url: &config.callback_url,

    };

    let client = Client::new();

    // FIXED API ENDPOINT PATH:
    let response = client
        .post("https://api.paystack.co/transaction/initialize")
        .header("Authorization", format!("Bearer {}", config.secret_key))
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
