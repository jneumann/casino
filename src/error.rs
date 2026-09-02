use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError};
use serde::Serialize;

use crate::db::StoreError;

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("invalid username or password")]
    InvalidCredentials,

    #[error("authentication required")]
    Unauthenticated,

    #[error("{0}")]
    Forbidden(String),

    #[error("{0}")]
    NotFound(String),

    #[error("{0}")]
    BadRequest(String),

    #[error("username is already taken")]
    UsernameTaken,

    #[error("you do not have enough coins")]
    InsufficientFunds,

    #[error("you already have an outstanding loan")]
    LoanOutstanding,

    #[error("you do not have an outstanding loan")]
    NoOpenLoan,

    #[error("you already have a hand in play")]
    HandInProgress,

    #[error("you do not have a hand in play")]
    NoOpenHand,

    #[error("internal server error")]
    Store(#[from] StoreError),

    #[error("internal server error")]
    Internal(#[source] anyhow::Error),
}

impl ApiError {
    fn code(&self) -> &'static str {
        match self {
            Self::InvalidCredentials => "invalid_credentials",
            Self::Unauthenticated => "unauthenticated",
            Self::Forbidden(_) => "forbidden",
            Self::NotFound(_) => "not_found",
            Self::BadRequest(_) => "bad_request",
            Self::UsernameTaken => "username_taken",
            Self::InsufficientFunds => "insufficient_funds",
            Self::LoanOutstanding => "loan_outstanding",
            Self::NoOpenLoan => "no_open_loan",
            Self::HandInProgress => "hand_in_progress",
            Self::NoOpenHand => "no_open_hand",
            Self::Store(_) | Self::Internal(_) => "internal_error",
        }
    }
}

impl ResponseError for ApiError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::InvalidCredentials | Self::Unauthenticated => StatusCode::UNAUTHORIZED,
            Self::Forbidden(_) => StatusCode::FORBIDDEN,
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::UsernameTaken => StatusCode::CONFLICT,
            Self::InsufficientFunds => StatusCode::PAYMENT_REQUIRED,
            Self::LoanOutstanding => StatusCode::CONFLICT,
            Self::NoOpenLoan => StatusCode::BAD_REQUEST,
            Self::HandInProgress => StatusCode::CONFLICT,
            Self::NoOpenHand => StatusCode::BAD_REQUEST,
            Self::Store(_) | Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn error_response(&self) -> HttpResponse {
        if let Self::Store(err) = self {
            tracing::error!(error = %err, "database failure");
        }
        if let Self::Internal(err) = self {
            tracing::error!(error = ?err, "unhandled failure");
        }

        let mut response = HttpResponse::build(self.status_code()).json(ErrorBody {
            error: self.code(),
            message: self.to_string(),
        });

        if matches!(self, Self::Unauthenticated | Self::InvalidCredentials) {
            response.headers_mut().insert(
                actix_web::http::header::WWW_AUTHENTICATE,
                actix_web::http::header::HeaderValue::from_static("Bearer"),
            );
        }

        response
    }
}

#[derive(Serialize)]
struct ErrorBody {
    error: &'static str,
    message: String,
}
