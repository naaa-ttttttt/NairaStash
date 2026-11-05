use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct PaystackWebhook {
    pub event: String,
    pub data: WebhookData,
}

#[derive(Debug, Deserialize)]
pub struct WebhookData {
    pub id: u64,
    pub status: String,
    pub reference: String,
    pub amount: i64,
    pub currency: String,
    pub customer: WebhookCustomer,
}

#[derive(Debug, Deserialize)]
pub struct WebhookCustomer {
    pub email: String,
}

pub async fn handle_paystack_webhook(
    raw_bytes: axum::body::Bytes,
    headers: axum::http::HeaderMap,
) -> impl axum::response::IntoResponse {
    
    //syntax for extracting the header signature from the captured request
    let header_signature = match headers.get("x-paystack-signature") {
        Some(header_value) => {
            header_value.to_str().unwrap_or("")
        }
        None => {
            return axum::http::StatusCode::BAD_REQUEST.into_response();
        }
    };


}
