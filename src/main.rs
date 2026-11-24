//importing of external dependencies/libraries/crates
use sqlx::PgPool;
use reqwest::Client;
use axum::Router;
use tower_http::services::ServeDir;
use crate::domains::deposits::pay_stack::PaystackConfig;


// Register the domains & shared module folder
mod domains;
mod shared;


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

    // loads paystack details from beginning of the run of the program, instead of the program crashing during runtime
    let paystack = match domains::deposits::pay_stack::load_paystack_config() {
        Ok(config) => config,
        Err(message) => {
            eprintln!("Startup stopped: {}", message);
            std::process::exit(1);
        }
    };

    // the database url must come from .env, no guessed fallback
    let db_url = match std::env::var("DATABASE_URL") {
        Ok(url) if !url.trim().is_empty() => url,
        _ => {
            eprintln!("Startup stopped: DATABASE_URL is missing or empty in .env");
            std::process::exit(1);
        }
    };

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
        .merge(domains::plan_funds::routes(shared_state.clone()))
        .merge(domains::groups::routes(shared_state.clone()))
        .merge(domains::withdrawals::routes(shared_state.clone()))
        .merge(domains::profile::routes(shared_state.clone()))
        .merge(domains::ledger::routes(shared_state.clone()))

        // saved banks, the payout pin and payouts (wallet to bank)
        .merge(domains::bank_accounts::routes::routes(shared_state.clone()))
        .merge(domains::payout_pin::routes::routes(shared_state.clone()))
        .merge(domains::payouts::routes::routes(shared_state.clone()))

        // Nest your Paystack deposit engine slice under a clean API prefix
        .nest("/api/deposits", domains::deposits::routes::deposit_routes(shared_state.clone()))

        // public files (the app manifest and icons), no login needed, read from the static folder
        .nest_service("/static", ServeDir::new("static"))

        // Global visual fallback rule (The 404 page)
        .fallback(domains::dashboard::not_found_handler);

    // Bind server listener socket lines
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();

    println!("Naira Stash Engine active at http://127.0.0.1:3000");

    // the stuck-payout job: every minute it asks paystack about payouts that stayed pending too long
    // it only asks, it never starts a transfer, so it cannot send money twice
    let job_state = shared_state.clone();
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(std::time::Duration::from_secs(
            domains::payouts::services::STUCK_JOB_INTERVAL_SECONDS,
        ));
        loop {
            ticker.tick().await;
            domains::payouts::services::settle_stuck_payouts(
                &job_state.db,
                &job_state.http_client,
                &job_state.paystack,
            )
            .await;
        }
    });

    axum::serve(listener, app).await.unwrap();
}
