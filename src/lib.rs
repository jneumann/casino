pub mod auth;
pub mod config;
pub mod db;
pub mod error;
pub mod games;
pub mod models;
pub mod routes;
pub mod state;

pub use config::Config;
pub use error::ApiError;
pub use state::AppState;

use actix_files::Files;
use actix_web::middleware::{Logger, NormalizePath};
use actix_web::{App, HttpServer, web};
use std::sync::Arc;

/// Boots the HTTP server. Returns once the server shuts down.
pub async fn run(config: Config, store: db::DynStore) -> std::io::Result<()> {
    let bind = (config.host.clone(), config.port);
    let state = web::Data::new(AppState::new(config, store));
    let static_dir = state.config.static_dir.clone();

    tracing::info!(
        host = %bind.0,
        port = bind.1,
        backend = state.store.backend(),
        "starting casino-backend"
    );

    HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .app_data(
                web::JsonConfig::default()
                    .limit(16 * 1024)
                    .error_handler(|err, _| ApiError::BadRequest(err.to_string()).into()),
            )
            .wrap(Logger::default())
            .wrap(NormalizePath::trim())
            .configure(routes::configure)
            .service(
                Files::new("/", static_dir.clone())
                    .index_file("index.html")
                    .prefer_utf8(true),
            )
    })
    .bind(bind)?
    .run()
    .await
}

/// Opens the configured database, applies migrations, and returns it as a [`db::DynStore`].
pub async fn open_store(config: &Config) -> anyhow::Result<db::DynStore> {
    let store = db::sqlite::SqliteStore::connect(&config.database_url).await?;
    store.migrate().await?;
    Ok(Arc::new(store))
}
