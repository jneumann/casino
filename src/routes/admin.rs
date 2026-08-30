use actix_web::{HttpResponse, delete, get, patch, post, put, web};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::auth::{
    AdminOperator, IssuedToken, OperatorUser, decoy_hash, hash_password, validate_password,
    validate_username, verify_password,
};
use crate::db::{AdjustmentFilter, BalanceAdjustmentRecord, PlayerFilter, StoreError};
use crate::error::ApiError;
use crate::models::{BalanceAdjustment, Operator, OperatorRole, User};
use crate::state::AppState;

const MAX_PAGE_SIZE: i64 = 200;
const MAX_ABS_DELTA: i64 = 1_000_000_000;
const MAX_REASON_LEN: usize = 280;

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateOperatorRequest {
    pub username: String,
    pub password: String,
    #[serde(default = "default_role")]
    pub role: OperatorRole,
}

fn default_role() -> OperatorRole {
    OperatorRole::Viewer
}

#[derive(Debug, Deserialize)]
pub struct UpdateOperatorRequest {
    pub role: Option<OperatorRole>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct ResetPasswordRequest {
    pub new_password: String,
}

#[derive(Debug, Deserialize)]
pub struct ChangeOwnPasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePlayerRequest {
    pub is_banned: bool,
}

#[derive(Debug, Deserialize)]
pub struct ListPlayersQuery {
    pub search: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct AdjustBalanceRequest {
    pub delta: i64,
    pub reason: String,
}

#[derive(Debug, Deserialize)]
pub struct ListAdjustmentsQuery {
    pub player_id: Option<i64>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct OperatorResponse {
    pub id: i64,
    pub username: String,
    pub role: OperatorRole,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub last_login_at: Option<DateTime<Utc>>,
}

impl From<&Operator> for OperatorResponse {
    fn from(operator: &Operator) -> Self {
        Self {
            id: operator.id,
            username: operator.username.clone(),
            role: operator.role,
            is_active: operator.is_active,
            created_at: operator.created_at,
            last_login_at: operator.last_login_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    #[serde(flatten)]
    pub token: IssuedToken,
    pub operator: OperatorResponse,
}

#[derive(Debug, Serialize)]
pub struct PlayerResponse {
    pub id: i64,
    pub username: String,
    pub balance: i64,
    pub is_banned: bool,
    pub created_at: DateTime<Utc>,
    pub last_login_at: Option<DateTime<Utc>>,
}

impl From<&User> for PlayerResponse {
    fn from(player: &User) -> Self {
        Self {
            id: player.id,
            username: player.username.clone(),
            balance: player.balance,
            is_banned: player.is_banned,
            created_at: player.created_at,
            last_login_at: player.last_login_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct AdjustmentResponse {
    pub id: i64,
    pub player_id: i64,
    pub player_username: String,
    pub operator_id: Option<i64>,
    pub operator_username: String,
    pub delta: i64,
    pub balance_before: i64,
    pub balance_after: i64,
    pub reason: String,
    pub created_at: DateTime<Utc>,
}

impl From<&BalanceAdjustment> for AdjustmentResponse {
    fn from(adjustment: &BalanceAdjustment) -> Self {
        Self {
            id: adjustment.id,
            player_id: adjustment.user_id,
            player_username: adjustment.player_username.clone(),
            operator_id: adjustment.operator_id,
            operator_username: adjustment.operator_username.clone(),
            delta: adjustment.delta,
            balance_before: adjustment.balance_before,
            balance_after: adjustment.balance_after,
            reason: adjustment.reason.clone(),
            created_at: adjustment.created_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct AdjustBalanceResponse {
    pub player: PlayerResponse,
    pub adjustment: AdjustmentResponse,
}

/// Signs an operator in. Deliberately separate from the player login so a
/// player token can never be presented to the admin API.
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

    let operator = state.store.operator_by_username(&username).await?;

    // Hash even when the account is unknown, so response time does not reveal
    // which operator names exist.
    let candidate_hash = operator
        .as_ref()
        .map(|operator| operator.password_hash.clone())
        .unwrap_or_else(|| decoy_hash().to_string());

    let matched = web::block(move || verify_password(&password, &candidate_hash))
        .await
        .map_err(|err| ApiError::Internal(anyhow::anyhow!("password check failed: {err}")))?;

    let Some(operator) = operator.filter(|_| matched) else {
        tracing::info!(username = %username, "failed operator login");
        return Err(ApiError::InvalidCredentials);
    };

    // Only reachable with the correct password, so naming the reason leaks
    // nothing an attacker does not already have.
    if !operator.is_active {
        return Err(ApiError::Forbidden(
            "this operator account has been deactivated".into(),
        ));
    }

    let token = state.tokens.issue_for_operator(&operator)?;
    state
        .store
        .record_operator_login(operator.id, Utc::now())
        .await?;
    tracing::info!(
        operator_id = operator.id,
        username = %operator.username,
        role = ?operator.role,
        "operator login succeeded"
    );

    Ok(HttpResponse::Ok().json(LoginResponse {
        token,
        operator: OperatorResponse::from(&operator),
    }))
}

#[get("/me")]
pub async fn me(
    state: web::Data<AppState>,
    identity: OperatorUser,
) -> Result<HttpResponse, ApiError> {
    let operator = state
        .store
        .operator_by_id(identity.id)
        .await?
        .ok_or(ApiError::Unauthenticated)?;

    Ok(HttpResponse::Ok().json(OperatorResponse::from(&operator)))
}

/// Lets any operator rotate their own password. Requires the current one, and
/// signs their other sessions out on success.
#[put("/me/password")]
pub async fn change_own_password(
    state: web::Data<AppState>,
    identity: OperatorUser,
    body: web::Json<ChangeOwnPasswordRequest>,
) -> Result<HttpResponse, ApiError> {
    let ChangeOwnPasswordRequest {
        current_password,
        new_password,
    } = body.into_inner();

    validate_password(&new_password).map_err(ApiError::BadRequest)?;

    let operator = state
        .store
        .operator_by_id(identity.id)
        .await?
        .ok_or(ApiError::Unauthenticated)?;

    let stored = operator.password_hash.clone();
    let matched = web::block(move || verify_password(&current_password, &stored))
        .await
        .map_err(|err| ApiError::Internal(anyhow::anyhow!("password check failed: {err}")))?;

    if !matched {
        return Err(ApiError::Forbidden("current password is incorrect".into()));
    }

    let hashed = hash_on_blocking_pool(new_password).await?;
    state
        .store
        .update_operator_password(operator.id, &hashed)
        .await?;

    tracing::info!(
        operator_id = operator.id,
        "operator changed their own password"
    );

    Ok(HttpResponse::NoContent().finish())
}

#[get("/operators")]
pub async fn list_operators(
    state: web::Data<AppState>,
    _identity: OperatorUser,
) -> Result<HttpResponse, ApiError> {
    let operators = state.store.list_operators().await?;
    let body: Vec<OperatorResponse> = operators.iter().map(OperatorResponse::from).collect();

    Ok(HttpResponse::Ok().json(body))
}

#[post("/operators")]
pub async fn create_operator(
    state: web::Data<AppState>,
    admin: AdminOperator,
    body: web::Json<CreateOperatorRequest>,
) -> Result<HttpResponse, ApiError> {
    let CreateOperatorRequest {
        username,
        password,
        role,
    } = body.into_inner();
    let username = username.trim().to_string();

    validate_username(&username).map_err(ApiError::BadRequest)?;
    validate_password(&password).map_err(ApiError::BadRequest)?;

    let password_hash = hash_on_blocking_pool(password).await?;

    let operator = match state
        .store
        .create_operator(&username, &password_hash, role)
        .await
    {
        Ok(operator) => operator,
        Err(StoreError::DuplicateUsername) => return Err(ApiError::UsernameTaken),
        Err(err) => return Err(err.into()),
    };

    tracing::info!(
        by = %admin.username(),
        operator_id = operator.id,
        username = %operator.username,
        role = ?operator.role,
        "created operator"
    );

    Ok(HttpResponse::Created().json(OperatorResponse::from(&operator)))
}

#[patch("/operators/{id}")]
pub async fn update_operator(
    state: web::Data<AppState>,
    admin: AdminOperator,
    path: web::Path<i64>,
    body: web::Json<UpdateOperatorRequest>,
) -> Result<HttpResponse, ApiError> {
    let id = path.into_inner();
    let UpdateOperatorRequest { role, is_active } = body.into_inner();

    if role.is_none() && is_active.is_none() {
        return Err(ApiError::BadRequest(
            "provide role, is_active, or both".into(),
        ));
    }

    // Refusing self-demotion and self-deactivation is what guarantees at least
    // one active admin always remains.
    if id == admin.id() {
        if is_active == Some(false) {
            return Err(ApiError::Forbidden(
                "you cannot deactivate your own account".into(),
            ));
        }
        if role == Some(OperatorRole::Viewer) {
            return Err(ApiError::Forbidden(
                "you cannot remove your own admin role".into(),
            ));
        }
    }

    if let Some(role) = role
        && !state.store.set_operator_role(id, role).await?
    {
        return Err(unknown_operator());
    }

    if let Some(is_active) = is_active
        && !state.store.set_operator_active(id, is_active).await?
    {
        return Err(unknown_operator());
    }

    let operator = state
        .store
        .operator_by_id(id)
        .await?
        .ok_or_else(unknown_operator)?;

    tracing::info!(
        by = %admin.username(),
        operator_id = operator.id,
        role = ?operator.role,
        is_active = operator.is_active,
        "updated operator"
    );

    Ok(HttpResponse::Ok().json(OperatorResponse::from(&operator)))
}

/// Resets another operator's password. Your own goes through
/// `PUT /api/admin/me/password`, which proves you know the current one.
#[put("/operators/{id}/password")]
pub async fn reset_operator_password(
    state: web::Data<AppState>,
    admin: AdminOperator,
    path: web::Path<i64>,
    body: web::Json<ResetPasswordRequest>,
) -> Result<HttpResponse, ApiError> {
    let id = path.into_inner();
    let ResetPasswordRequest { new_password } = body.into_inner();

    if id == admin.id() {
        return Err(ApiError::Forbidden(
            "use the change-my-password endpoint for your own account".into(),
        ));
    }

    validate_password(&new_password).map_err(ApiError::BadRequest)?;

    let password_hash = hash_on_blocking_pool(new_password).await?;

    if !state
        .store
        .update_operator_password(id, &password_hash)
        .await?
    {
        return Err(unknown_operator());
    }

    tracing::info!(by = %admin.username(), operator_id = id, "reset operator password");

    Ok(HttpResponse::NoContent().finish())
}

#[delete("/operators/{id}")]
pub async fn delete_operator(
    state: web::Data<AppState>,
    admin: AdminOperator,
    path: web::Path<i64>,
) -> Result<HttpResponse, ApiError> {
    let id = path.into_inner();

    if id == admin.id() {
        return Err(ApiError::Forbidden(
            "you cannot delete your own account".into(),
        ));
    }

    if !state.store.delete_operator(id).await? {
        return Err(unknown_operator());
    }

    tracing::info!(by = %admin.username(), operator_id = id, "deleted operator");

    Ok(HttpResponse::NoContent().finish())
}

#[get("/players")]
pub async fn list_players(
    state: web::Data<AppState>,
    _identity: OperatorUser,
    query: web::Query<ListPlayersQuery>,
) -> Result<HttpResponse, ApiError> {
    let ListPlayersQuery {
        search,
        limit,
        offset,
    } = query.into_inner();

    let search = search
        .map(|term| term.trim().to_string())
        .filter(|term| !term.is_empty());

    let filter = PlayerFilter {
        search: search.as_deref(),
        limit: limit.unwrap_or(50).clamp(1, MAX_PAGE_SIZE),
        offset: offset.unwrap_or(0).max(0),
    };

    let players = state.store.list_players(filter).await?;
    let body: Vec<PlayerResponse> = players.iter().map(PlayerResponse::from).collect();

    Ok(HttpResponse::Ok().json(body))
}

#[patch("/players/{id}")]
pub async fn update_player(
    state: web::Data<AppState>,
    admin: AdminOperator,
    path: web::Path<i64>,
    body: web::Json<UpdatePlayerRequest>,
) -> Result<HttpResponse, ApiError> {
    let id = path.into_inner();
    let UpdatePlayerRequest { is_banned } = body.into_inner();

    if !state.store.set_player_banned(id, is_banned).await? {
        return Err(ApiError::NotFound("no player with that id".into()));
    }

    tracing::info!(by = %admin.username(), player_id = id, is_banned, "updated player");

    let player = state
        .store
        .user_by_id(id)
        .await?
        .ok_or_else(|| ApiError::NotFound("no player with that id".into()))?;

    Ok(HttpResponse::Ok().json(PlayerResponse::from(&player)))
}

/// Credits or debits a player's coins. The reason and the operator who made
/// the change are written to the audit log in the same transaction.
#[post("/players/{id}/balance")]
pub async fn adjust_player_balance(
    state: web::Data<AppState>,
    admin: AdminOperator,
    path: web::Path<i64>,
    body: web::Json<AdjustBalanceRequest>,
) -> Result<HttpResponse, ApiError> {
    let id = path.into_inner();
    let AdjustBalanceRequest { delta, reason } = body.into_inner();
    let reason = reason.trim().to_string();

    if delta == 0 {
        return Err(ApiError::BadRequest(
            "delta must be a non-zero amount".into(),
        ));
    }
    if !(-MAX_ABS_DELTA..=MAX_ABS_DELTA).contains(&delta) {
        return Err(ApiError::BadRequest(format!(
            "delta must be between -{MAX_ABS_DELTA} and {MAX_ABS_DELTA}"
        )));
    }
    if reason.is_empty() {
        return Err(ApiError::BadRequest("a reason is required".into()));
    }
    if reason.chars().count() > MAX_REASON_LEN {
        return Err(ApiError::BadRequest(format!(
            "reason must be {MAX_REASON_LEN} characters or fewer"
        )));
    }

    let (player, adjustment) = match state
        .store
        .adjust_player_balance(BalanceAdjustmentRecord {
            user_id: id,
            operator_id: admin.id(),
            operator_username: admin.username(),
            delta,
            reason: &reason,
        })
        .await
    {
        Ok(result) => result,
        Err(StoreError::UnknownUser) => {
            return Err(ApiError::NotFound("no player with that id".into()));
        }
        Err(StoreError::InsufficientFunds) => {
            return Err(ApiError::BadRequest(
                "this adjustment would take the balance below zero".into(),
            ));
        }
        Err(err) => return Err(err.into()),
    };

    tracing::info!(
        by = %admin.username(),
        player_id = player.id,
        delta,
        balance_after = player.balance,
        reason = %reason,
        "adjusted player balance"
    );

    Ok(HttpResponse::Ok().json(AdjustBalanceResponse {
        player: PlayerResponse::from(&player),
        adjustment: AdjustmentResponse::from(&adjustment),
    }))
}

#[get("/adjustments")]
pub async fn list_adjustments(
    state: web::Data<AppState>,
    _identity: OperatorUser,
    query: web::Query<ListAdjustmentsQuery>,
) -> Result<HttpResponse, ApiError> {
    let ListAdjustmentsQuery {
        player_id,
        limit,
        offset,
    } = query.into_inner();

    let filter = AdjustmentFilter {
        user_id: player_id.filter(|id| *id > 0),
        limit: limit.unwrap_or(50).clamp(1, MAX_PAGE_SIZE),
        offset: offset.unwrap_or(0).max(0),
    };

    let adjustments = state.store.list_balance_adjustments(filter).await?;
    let body: Vec<AdjustmentResponse> = adjustments.iter().map(AdjustmentResponse::from).collect();

    Ok(HttpResponse::Ok().json(body))
}

async fn hash_on_blocking_pool(password: String) -> Result<String, ApiError> {
    web::block(move || hash_password(&password))
        .await
        .map_err(|err| ApiError::Internal(anyhow::anyhow!("password hashing failed: {err}")))?
}

fn unknown_operator() -> ApiError {
    ApiError::NotFound("no operator with that id".into())
}
