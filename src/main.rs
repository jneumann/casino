use casino_backend::{Config, open_store, run};
use tracing_subscriber::EnvFilter;

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,actix_web=info,sqlx=warn")),
        )
        .init();

    let config = Config::from_env()?;
    let store = open_store(&config).await?;

    run(config, store).await?;
    Ok(())
}
