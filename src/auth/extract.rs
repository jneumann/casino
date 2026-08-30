use std::future::Future;
use std::pin::Pin;

use actix_web::http::header::AUTHORIZATION;
use actix_web::{FromRequest, HttpRequest, web};

use crate::auth::token::Audience;
use crate::error::ApiError;
use crate::models::{Operator, OperatorRole};
use crate::state::AppState;

type Extraction<T> = Pin<Box<dyn Future<Output = Result<T, ApiError>>>>;

/// A verified player identity. Requesting this in a handler makes the endpoint
/// player-authenticated; operator tokens are refused.
#[derive(Debug, Clone)]
pub struct AuthUser {
    pub id: i64,
    pub username: String,
}

/// A verified operator identity, loaded fresh from the database on every
/// request so deactivations and password changes take effect immediately.
#[derive(Debug, Clone)]
pub struct OperatorUser {
    pub id: i64,
    pub username: String,
    pub role: OperatorRole,
}

impl OperatorUser {
    fn from_record(operator: &Operator) -> Self {
        Self {
            id: operator.id,
            username: operator.username.clone(),
            role: operator.role,
        }
    }
}

/// An operator who additionally holds the admin role. Anything that changes
/// accounts requires this rather than [`OperatorUser`].
#[derive(Debug, Clone)]
pub struct AdminOperator(pub OperatorUser);

impl AdminOperator {
    pub fn id(&self) -> i64 {
        self.0.id
    }

    pub fn username(&self) -> &str {
        &self.0.username
    }
}

impl FromRequest for AuthUser {
    type Error = ApiError;
    type Future = Extraction<Self>;

    fn from_request(req: &HttpRequest, _payload: &mut actix_web::dev::Payload) -> Self::Future {
        let req = req.clone();

        Box::pin(async move {
            let (state, claims) = verified_claims(&req, Audience::Player)?;
            let id: i64 = claims.sub.parse().map_err(|_| ApiError::Unauthenticated)?;

            let player = state
                .store
                .user_by_id(id)
                .await?
                .ok_or(ApiError::Unauthenticated)?;

            if player.is_banned {
                tracing::info!(user_id = player.id, "refused a banned player");
                return Err(ApiError::Forbidden(
                    "this account has been suspended".into(),
                ));
            }

            Ok(AuthUser {
                id: player.id,
                username: player.username,
            })
        })
    }
}

impl FromRequest for OperatorUser {
    type Error = ApiError;
    type Future = Extraction<Self>;

    fn from_request(req: &HttpRequest, _payload: &mut actix_web::dev::Payload) -> Self::Future {
        let req = req.clone();
        Box::pin(async move { load_operator(&req).await })
    }
}

impl FromRequest for AdminOperator {
    type Error = ApiError;
    type Future = Extraction<Self>;

    fn from_request(req: &HttpRequest, _payload: &mut actix_web::dev::Payload) -> Self::Future {
        let req = req.clone();

        Box::pin(async move {
            let operator = load_operator(&req).await?;

            if !operator.role.is_admin() {
                tracing::info!(
                    operator = %operator.username,
                    "refused a non-admin operator"
                );
                return Err(ApiError::Forbidden(
                    "this action requires an admin operator".into(),
                ));
            }

            Ok(AdminOperator(operator))
        })
    }
}

async fn load_operator(req: &HttpRequest) -> Result<OperatorUser, ApiError> {
    let (state, claims) = verified_claims(req, Audience::Operator)?;
    let id: i64 = claims.sub.parse().map_err(|_| ApiError::Unauthenticated)?;

    let operator = state
        .store
        .operator_by_id(id)
        .await?
        .ok_or(ApiError::Unauthenticated)?;

    if !operator.is_active {
        tracing::info!(operator = %operator.username, "refused a deactivated operator");
        return Err(ApiError::Unauthenticated);
    }

    // A token minted before the current password is a leftover from an earlier
    // session, so a password change logs those sessions out.
    if claims.iat < operator.password_changed_at.timestamp() {
        tracing::info!(
            operator = %operator.username,
            "refused a token predating the current password"
        );
        return Err(ApiError::Unauthenticated);
    }

    Ok(OperatorUser::from_record(&operator))
}

fn verified_claims(
    req: &HttpRequest,
    expected: Audience,
) -> Result<(web::Data<AppState>, crate::auth::Claims), ApiError> {
    let state = req
        .app_data::<web::Data<AppState>>()
        .ok_or_else(|| ApiError::Internal(anyhow::anyhow!("application state is missing")))?
        .clone();

    let token = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .ok_or(ApiError::Unauthenticated)?;

    let claims = state.tokens.verify(token, expected)?;

    Ok((state, claims))
}
