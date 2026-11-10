use sqlx::PgPool;
use reqwest::Client;
use axum::Router;

// Register the domains module folder
mod domains;

// The shared global toolkit structure
#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub http_client: Client,
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    
    // Connect to your local database vault
    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/nairastash".to_string());
    
    let pool = PgPool::connect(&db_url)
        .await
        .expect("CRITICAL: Failed to connect to PostgreSQL connection pool!");
    let client = Client::new();

    let shared_state = AppState {
        db: pool,
        http_client: client,
    };

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

