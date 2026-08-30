use anyhow::{Context, bail};
use casino_backend::auth::{hash_password, validate_password, validate_username};
use casino_backend::db::StoreError;
use casino_backend::{Config, open_store};

/// Creates a status-page login.
///
/// Usage: `create-user <username>`
/// The password is read from `CASINO_USER_PASSWORD` when set, otherwise
/// prompted for interactively.
#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt().with_target(false).init();

    let username = std::env::args()
        .nth(1)
        .context("usage: create-user <username>")?
        .trim()
        .to_string();

    if let Err(message) = validate_username(&username) {
        bail!("{message}");
    }

    let password = read_password()?;
    if let Err(message) = validate_password(&password) {
        bail!("{message}");
    }

    let config = Config::from_env()?;
    let store = open_store(&config).await?;

    let password_hash = hash_password(&password).map_err(|err| anyhow::anyhow!("{err}"))?;

    match store.create_user(&username, &password_hash).await {
        Ok(user) => {
            println!(
                "created user '{}' (id {}) with {} coins",
                user.username, user.id, user.balance
            );
            Ok(())
        }
        Err(StoreError::DuplicateUsername) => {
            bail!("a user named '{username}' already exists")
        }
        Err(err) => Err(err.into()),
    }
}

fn read_password() -> anyhow::Result<String> {
    if let Ok(password) =
        std::env::var("CASINO_USER_PASSWORD").map(|value| value.trim().to_string())
        && !password.is_empty()
    {
        return Ok(password);
    }

    let password = rpassword::prompt_password("password: ")?;
    let confirmation = rpassword::prompt_password("confirm password: ")?;

    if password != confirmation {
        bail!("passwords did not match");
    }

    Ok(password)
}
