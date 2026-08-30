use std::time::Duration;

use chrono::{DateTime, TimeDelta, Utc};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};

use crate::error::ApiError;
use crate::models::{Operator, User};

/// Which surface a token was issued for. Player tokens and operator tokens are
/// signed with the same key, so this claim is what keeps a self-registered
/// player out of the admin API.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Audience {
    #[default]
    Player,
    Operator,
}

impl Audience {
    fn label(self) -> &'static str {
        match self {
            Self::Player => "player",
            Self::Operator => "operator",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub username: String,
    /// Absent in tokens issued before audiences existed; those are treated as
    /// player tokens, which is the lower privilege of the two.
    #[serde(default)]
    pub kind: Audience,
    pub iat: i64,
    pub exp: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct IssuedToken {
    pub token: String,
    pub token_type: &'static str,
    pub expires_at: DateTime<Utc>,
    pub expires_in: i64,
}

pub struct TokenService {
    encoding: EncodingKey,
    decoding: DecodingKey,
    validation: Validation,
    ttl: TimeDelta,
}

impl TokenService {
    pub fn new(secret: &[u8], ttl: Duration) -> Self {
        let mut validation = Validation::new(Algorithm::HS256);
        validation.set_required_spec_claims(&["exp", "sub"]);
        validation.leeway = 5;

        let ttl = TimeDelta::from_std(ttl).unwrap_or_else(|_| TimeDelta::hours(1));

        Self {
            encoding: EncodingKey::from_secret(secret),
            decoding: DecodingKey::from_secret(secret),
            validation,
            ttl,
        }
    }

    pub fn issue(&self, user: &User) -> Result<IssuedToken, ApiError> {
        self.issue_for(user.id, &user.username, Audience::Player)
    }

    pub fn issue_for_operator(&self, operator: &Operator) -> Result<IssuedToken, ApiError> {
        self.issue_for(operator.id, &operator.username, Audience::Operator)
    }

    fn issue_for(
        &self,
        subject: i64,
        username: &str,
        kind: Audience,
    ) -> Result<IssuedToken, ApiError> {
        let issued_at = Utc::now();
        let expires_at = issued_at + self.ttl;

        let claims = Claims {
            sub: subject.to_string(),
            username: username.to_string(),
            kind,
            iat: issued_at.timestamp(),
            exp: expires_at.timestamp(),
        };

        let token = encode(&Header::new(Algorithm::HS256), &claims, &self.encoding)
            .map_err(|err| ApiError::Internal(anyhow::anyhow!("failed to sign token: {err}")))?;

        Ok(IssuedToken {
            token,
            token_type: "Bearer",
            expires_at,
            expires_in: self.ttl.num_seconds(),
        })
    }

    /// Verifies a token and requires it to have been issued for `expected`.
    pub fn verify(&self, token: &str, expected: Audience) -> Result<Claims, ApiError> {
        let claims = decode::<Claims>(token, &self.decoding, &self.validation)
            .map(|data| data.claims)
            .map_err(|err| {
                tracing::debug!(error = %err, "rejected bearer token");
                ApiError::Unauthenticated
            })?;

        if claims.kind != expected {
            tracing::debug!(
                presented = claims.kind.label(),
                expected = expected.label(),
                "rejected token issued for another audience"
            );
            return Err(ApiError::Unauthenticated);
        }

        Ok(claims)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user() -> User {
        User {
            id: 7,
            username: "pitboss".to_string(),
            password_hash: String::new(),
            balance: crate::models::STARTING_BALANCE,
            is_banned: false,
            created_at: Utc::now(),
            last_login_at: None,
        }
    }

    fn operator() -> Operator {
        Operator {
            id: 3,
            username: "floor-manager".to_string(),
            password_hash: String::new(),
            role: crate::models::OperatorRole::Admin,
            is_active: true,
            created_at: Utc::now(),
            last_login_at: None,
            password_changed_at: Utc::now(),
        }
    }

    #[test]
    fn round_trips_claims() {
        let service =
            TokenService::new(b"a-secret-long-enough-for-tests!!", Duration::from_secs(60));
        let issued = service.issue(&user()).expect("issue");
        let claims = service
            .verify(&issued.token, Audience::Player)
            .expect("verify");

        assert_eq!(claims.sub, "7");
        assert_eq!(claims.username, "pitboss");
        assert_eq!(issued.expires_in, 60);
    }

    #[test]
    fn refuses_a_player_token_on_the_operator_surface() {
        let service =
            TokenService::new(b"a-secret-long-enough-for-tests!!", Duration::from_secs(60));
        let player = service.issue(&user()).expect("issue");

        assert!(service.verify(&player.token, Audience::Operator).is_err());
        assert!(service.verify(&player.token, Audience::Player).is_ok());
    }

    #[test]
    fn refuses_an_operator_token_on_the_player_surface() {
        let service =
            TokenService::new(b"a-secret-long-enough-for-tests!!", Duration::from_secs(60));
        let staff = service.issue_for_operator(&operator()).expect("issue");

        assert!(service.verify(&staff.token, Audience::Player).is_err());
        assert!(service.verify(&staff.token, Audience::Operator).is_ok());
    }

    #[test]
    fn rejects_token_signed_with_another_secret() {
        let issuer = TokenService::new(
            b"first-secret-long-enough-for-test",
            Duration::from_secs(60),
        );
        let verifier = TokenService::new(
            b"other-secret-long-enough-for-test",
            Duration::from_secs(60),
        );
        let issued = issuer.issue(&user()).expect("issue");

        assert!(verifier.verify(&issued.token, Audience::Player).is_err());
    }

    #[test]
    fn rejects_expired_token() {
        let service =
            TokenService::new(b"a-secret-long-enough-for-tests!!", Duration::from_secs(60));
        let expired = Claims {
            sub: "7".to_string(),
            username: "pitboss".to_string(),
            kind: Audience::Player,
            iat: 0,
            exp: 1_000,
        };
        let token =
            encode(&Header::new(Algorithm::HS256), &expired, &service.encoding).expect("encode");

        assert!(service.verify(&token, Audience::Player).is_err());
    }
}
