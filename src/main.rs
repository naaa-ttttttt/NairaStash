use sqlx::PgPool;
use reqwest::Client;
use axum::Router;
use crate::domains::deposits::pay_stack::PaystackConfig;


// Register the domains module folder
mod domains;
// The shared global toolkit structure
#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub http_client: Client,
    pub paystack: PaystackConfig,
}


#[tokio::main]
async fn main() {
    // loads the env file for use
    dotenvy::dotenv().ok();

    let paystack = match domains::deposits::pay_stack::load_paystack_config() {
        Ok(config) => config, 
        Err(message) => {
            eprintln!("Startup stopped: {}", message);
            std::process::exit(1);
        }
    };

    // Connect to your local database vault
    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/nairastash".to_string());
    
    let pool = PgPool::connect(&db_url)
        .await
        .expect("CRITICAL: Failed to connect to PostgreSQL connection pool!");
    let client = Client::new();

    let shared_state = AppState { db: pool, http_client: client, paystack };
    // Merging all your clean flat files into the master router
    let app = Router::new()
        .merge(domains::dashboard::routes(shared_state.clone()))
        .merge(domains::auth::routes(shared_state.clone()))
        .merge(domains::plans::routes(shared_state.clone()))
        .merge(domains::groups::routes(shared_state.clone()))
        .merge(domains::withdrawals::routes(shared_state.clone()))
        
        // Nest your Paystack deposit engine slice under a clean API prefix
        .nest("/api/deposits", domains::deposits::routes::deposit_routes(shared_state.clone()))
        
        // Global visual fallback rule (The 404 page)
        .fallback(domains::dashboard::not_found_handler);

    // Bind server listener socket lines
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
        
    println!("Naira Stash Engine active at http://127.0.0.1:3000");
    axum::serve(listener, app).await.unwrap();
}

