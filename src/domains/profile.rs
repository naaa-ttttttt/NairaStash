use askama::Template;
use axum::{
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
    routing::get,
    Router,
};
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::domains::auth::handlers::AuthUser;
use crate::AppState;

// ---------------------------------------------------------------- repo

// what the profile page needs from the database
#[derive(sqlx::FromRow)]
struct ProfileRow {
    name: String,
    email: String,
    phone: String,
    created_at: DateTime<Utc>,
}

// one user, nothing if the user does not exist
async fn find_profile_row(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Option<ProfileRow>, sqlx::Error> {
    let row = sqlx::query_as::<_, ProfileRow>(
        r#"
        SELECT name, email, phone, created_at
        FROM users
        WHERE user_id = $1
        "#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    Ok(row) // no semicolon
}

// how many savings plans this user owns
async fn count_plans(pool: &PgPool, user_id: Uuid) -> Result<i64, sqlx::Error> {
    let count =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM savings_plans WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(pool)
            .await?;

    Ok(count) // no semicolon
}

// ---------------------------------------------------------------- service

#[derive(Debug)]
pub enum ProfileError {
    NotFound,
    DatabaseError,
}

// logs the real database error, then gives back our own error
fn profile_database_error(err: sqlx::Error) -> ProfileError {
    eprintln!("Profile database error: {:?}", err);
    ProfileError::DatabaseError // no semicolon
}

// what the page shows, already formatted
pub struct ProfileView {
    pub name: String,
    pub initials: String,
    pub handle: String,
    pub email: String,
    pub phone: String,
    pub member_since: String,
    pub plan_count: i64,
}

// first letters of the first two words, "Chinedu Okafor" becomes "CO"
fn initials_of(name: &str) -> String {
    name.split_whitespace()
        .filter_map(|word| word.chars().next())
        .take(2)
        .collect::<String>()
        .to_uppercase() // no semicolon
}

// the part of the email before the @
fn handle_of(email: &str) -> String {
    email.split('@').next().unwrap_or("").to_string() // no semicolon
}

// builds the profile for one logged-in user
pub async fn get_profile(pool: &PgPool, user_id: Uuid) -> Result<ProfileView, ProfileError> {
    let row = find_profile_row(pool, user_id)
        .await
        .map_err(profile_database_error)?
        .ok_or(ProfileError::NotFound)?;

    let plan_count = count_plans(pool, user_id)
        .await
        .map_err(profile_database_error)?;

    Ok(ProfileView {
        initials: initials_of(&row.name),
        handle: handle_of(&row.email),
        member_since: row.created_at.format("%b '%y").to_string(),
        name: row.name,
        email: row.email,
        phone: row.phone,
        plan_count,
    }) // no semicolon
}

// ---------------------------------------------------------------- handler

#[derive(Template)]
#[template(path = "profile.html")]
pub struct ProfileTemplate {
    pub active: &'static str,
    pub name: String,
    pub initials: String,
    pub handle: String,
    pub email: String,
    pub phone: String,
    pub member_since: String,
    pub plan_count: i64,
}

// AuthUser is the guard: no valid session sends the visitor to /login
pub async fn profile_handler(auth: AuthUser, State(state): State<AppState>) -> Response {
    let view = match get_profile(&state.db, auth.user_id).await {
        Ok(view) => view,
        Err(ProfileError::NotFound) => return Redirect::to("/login").into_response(),
        Err(ProfileError::DatabaseError) => {
            return StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    };

    let page = ProfileTemplate {
        active: "profile",
        name: view.name,
        initials: view.initials,
        handle: view.handle,
        email: view.email,
        phone: view.phone,
        member_since: view.member_since,
        plan_count: view.plan_count,
    };

    match page.render() {
        Ok(html) => Html(html).into_response(),
        Err(e) => {
            eprintln!("profile template render failed: {e:?}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

// ---------------------------------------------------------------- routes

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/profile", get(profile_handler))
        .with_state(state)
}
