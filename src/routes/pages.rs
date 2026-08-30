use actix_files::NamedFile;
use actix_web::{get, web};
use std::path::Path;

use crate::error::ApiError;
use crate::state::AppState;

/// Serves the operator sign-in page. The player-facing game lives at `/`, so
/// the admin pages are kept under their own prefix.
#[get("/admin")]
pub async fn admin_page(state: web::Data<AppState>) -> Result<NamedFile, ApiError> {
    open_admin_page(&state, "index.html")
}

/// Serves the status dashboard shell. The page itself is unauthenticated HTML;
/// its data comes from `GET /api/status`, which requires an operator token.
#[get("/status")]
pub async fn status_page(state: web::Data<AppState>) -> Result<NamedFile, ApiError> {
    open_admin_page(&state, "status.html")
}

/// Serves the account management shell. As with the status page, everything
/// sensitive on it arrives from an authenticated API call.
#[get("/admin/users")]
pub async fn users_page(state: web::Data<AppState>) -> Result<NamedFile, ApiError> {
    open_admin_page(&state, "users.html")
}

fn open_admin_page(state: &AppState, file: &str) -> Result<NamedFile, ApiError> {
    let path = state.config.static_dir.join(Path::new("admin")).join(file);

    NamedFile::open(path)
        .map_err(|err| ApiError::Internal(anyhow::anyhow!("{file} unavailable: {err}")))
}
