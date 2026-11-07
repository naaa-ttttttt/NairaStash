use reqwest::{Client, header};
use serde::{Serialize, Deserialize};
use hmac::{Hmac, Mac};
use sha2::Sha512;
use std::env;
use subtle::ConstantTIme;
  

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
    let secret_key = std::env::var("PAYSTACK_SECRET_KEY")
        .unwrap_or_else(|_| "sk_test_placeholder".to_string());

    // 2. Instantiate our request payload blueprint using the incoming function arguments
    let payload = InitializePaymentRequest {
        email,
        amount,
        reference,
    };

    // 3. Initialize the HTTP client
    let client = reqwest::Client::new();

    // 4. Send the POST request to the Paystack API endpoint
    let response = client
        .post("https://api.paystack.co/transaction/initialize")
        .header("Authorization", format!("Bearer {}", secret_key))
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await?
        .error_for_status()?; // Catches API/auth errors safely before parsing JSON

    // 5. Extract and parse the JSON string response into our custom Rust structs
    let parsed_response = response.json::<PaystackResponse>().await?;

    // 6. Return the target authorization checkout URL string
    Ok(parsed_response.data.authorization_url)
}


pub fn verify__paystack_signature(
    payload_bytes: &[u8],
    header_signature: &str,
    secret_key: &str,
) -> bool {
   type HmacSha512 = Hmac<Sha512>;

   let mut mac = HmacSha512::new_from_slice(secret_key.as_bytes()).expect("Hmac can take key of any size");

   mac.update(payload_bytes);
   let result = mac.finalize();
   let code_bytes = result.into_bytes();
   
   let my_hex_string = hex::encode(code_bytes);

   let equality_check = my_hex_string.as_bytes().ct_eq(header_signature.as_bytes());
   equality_check.into()

}
