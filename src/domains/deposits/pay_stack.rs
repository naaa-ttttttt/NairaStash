use reqwest::{Client, header};
use serde::{Serialize, Deserialize};
use hmac::{Hmac, Mac};
use sha2::Sha512;
use std::env;

#[derive(Serialize)]
pub struct InitializePaymentRequest<'a> {
    pub email: &'a str,
    pub amount: i64,
    pub reference: &'a str,
}

#[derive(Deserialize)]
struct PaystackResponse {
    status: bool,
    message: String,
    data: PaystackResponseData,
}

#[derive(Deserialize)]
struct PaystackResponseData {
    authorization_url: String,
}

pub async fn initialize_payment(
    email: &str,
    amount: i64,
    reference: &str,
) -> Result<String, reqwest::Error> {
    // 1. Get the private secret key from the environment variables (.env)
    let secret_key = env::var("PAYSTACK_SECRET_KEY")
        .unwrap_or_else(|_| "sk_test_placeholder".to_string());

    // 2. Instantiate our request payload blueprint using the incoming function arguments
    let payload = InitializePaymentRequest {
        email,
        amount,
        reference,
    };

    // 3. Initialize the HTTP client and construct headers
    let client = Client::new();
    let auth_header_value = format!("Bearer {}", secret_key);

    // 4. Send the POST request to the Paystack API endpoint
    let response = client
        .post("https://paystack.co")
        .header(header::AUTHORIZATION, auth_header_value)
        .header(header::CONTENT_TYPE, "application/json")
        .json(&payload)
        .send()
        .await?;

    // 5. Extract and parse the JSON string response into our custom Rust structs
    let parsed_response = response.json::<PaystackResponse>().await?;

    // 6. Return the target authorization checkout URL string
    Ok(parsed_response.data.authorization_url)
}


pub async fn verify__paystack_signature(
    payload_bytes: &[u8],
    header_signature: &str,
    secret_key: &str,
) -> bool {
    
}
