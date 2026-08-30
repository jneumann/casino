use actix_web::{HttpResponse, get, post, web};
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::auth::{
    AuthUser, IssuedToken, decoy_hash, hash_password, validate_password, validate_username,
    verify_password,
};
use crate::db::StoreError;
use crate::error::ApiError;
use crate::models::User;
use crate::state::AppState;

use super::bank::LoanView;

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    #[serde(flatten)]
    pub token: IssuedToken,
    pub user: UserResponse,
}

#[derive(Debug, Serialize)]
pub struct UserResponse {
    pub id: i64,
    pub username: String,
    pub balance: i64,
    pub last_login_at: Option<chrono::DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loan: Option<LoanView>,
}

impl From<&User> for UserResponse {
    fn from(user: &User) -> Self {
        Self {
            id: user.id,
            username: user.username.clone(),
            balance: user.balance,
            last_login_at: user.last_login_at,
            loan: None,
        }
    }
}

impl UserResponse {
    fn with_loan(mut self, loan: Option<LoanView>) -> Self {
        self.loan = loan;
        self
    }
}

/// Creates a player account and signs them straight in, so the client does not
/// have to follow up with a separate login round trip.
#[post("/register")]
pub async fn register(
    state: web::Data<AppState>,
    body: web::Json<RegisterRequest>,
) -> Result<HttpResponse, ApiError> {
    let RegisterRequest { username, password } = body.into_inner();
    let username = username.trim().to_string();

    validate_username(&username).map_err(ApiError::BadRequest)?;
    validate_password(&password).map_err(ApiError::BadRequest)?;

    let password_hash = web::block(move || hash_password(&password))
        .await
        .map_err(|err| ApiError::Internal(anyhow::anyhow!("password hashing failed: {err}")))??;

    let user = match state.store.create_user(&username, &password_hash).await {
        Ok(user) => user,
        Err(StoreError::DuplicateUsername) => return Err(ApiError::UsernameTaken),
        Err(err) => return Err(err.into()),
    };

    let token = state.tokens.issue(&user)?;
    state.store.record_login(user.id, Utc::now()).await?;
    tracing::info!(user_id = user.id, username = %user.username, "registered new player");

    Ok(HttpResponse::Created().json(LoginResponse {
        token,
        user: UserResponse::from(&user),
    }))
}

#[post("/login")]
pub async fn login(
    state: web::Data<AppState>,
    body: web::Json<LoginRequest>,
) -> Result<HttpResponse, ApiError> {
    let LoginRequest { username, password } = body.into_inner();
    let username = username.trim().to_string();

    if username.is_empty() || password.is_empty() {
        return Err(ApiError::BadRequest(
            "username and password are required".into(),
        ));
    }

    let user = state.store.user_by_username(&username).await?;

    // Hash even when the account is unknown, so response time does not reveal
    // which usernames exist.
    let candidate_hash = user
        .as_ref()
        .map(|user| user.password_hash.clone())
        .unwrap_or_else(|| decoy_hash().to_string());

    let matched = web::block(move || verify_password(&password, &candidate_hash))
        .await
        .map_err(|err| ApiError::Internal(anyhow::anyhow!("password check failed: {err}")))?;

    let Some(user) = user.filter(|_| matched) else {
        tracing::info!(username = %username, "failed login attempt");
        return Err(ApiError::InvalidCredentials);
    };

    // Only reachable with the correct password, so naming the reason leaks
    // nothing an attacker does not already have.
    if user.is_banned {
        tracing::info!(user_id = user.id, "banned player attempted to sign in");
        return Err(ApiError::Forbidden(
            "this account has been suspended".into(),
        ));
    }

    let token = state.tokens.issue(&user)?;
    let loan = state
        .store
        .open_loan(user.id)
        .await?
        .as_ref()
        .map(LoanView::from);
    let response = LoginResponse {
        token,
        user: UserResponse::from(&user).with_loan(loan),
    };

    state.store.record_login(user.id, Utc::now()).await?;
    tracing::info!(user_id = user.id, username = %user.username, "login succeeded");

    Ok(HttpResponse::Ok().json(response))
}

#[get("/me")]
pub async fn me(state: web::Data<AppState>, identity: AuthUser) -> Result<HttpResponse, ApiError> {
    let user = state
        .store
        .user_by_id(identity.id)
        .await?
        .ok_or(ApiError::Unauthenticated)?;
    let loan = state
        .store
        .open_loan(user.id)
        .await?
        .as_ref()
        .map(LoanView::from);

    Ok(HttpResponse::Ok().json(UserResponse::from(&user).with_loan(loan)))
}
