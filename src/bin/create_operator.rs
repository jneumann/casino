use anyhow::{Context, bail};
use casino_backend::auth::{hash_password, validate_password, validate_username};
use casino_backend::db::StoreError;
use casino_backend::models::OperatorRole;
use casino_backend::{Config, open_store};

/// Creates a dashboard login for a member of staff.
///
/// Usage: `create-operator <username> [--role admin|viewer]`
///
/// The role defaults to `admin`, since this command is how the first operator
/// gets created and anyone who can run it already has server access. The
/// password is read from `CASINO_OPERATOR_PASSWORD` when set, otherwise
/// prompted for interactively.
#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt().with_target(false).init();

    let args: Vec<String> = std::env::args().skip(1).collect();
    let username = args
        .first()
        .context("usage: create-operator <username> [--role admin|viewer]")?
        .trim()
        .to_string();

    let role = parse_role(&args[1..])?;

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

    match store.create_operator(&username, &password_hash, role).await {
        Ok(operator) => {
            println!(
                "created {} operator '{}' (id {})",
                match operator.role {
                    OperatorRole::Admin => "admin",
                    OperatorRole::Viewer => "viewer",
                },
                operator.username,
                operator.id
            );
            Ok(())
        }
        Err(StoreError::DuplicateUsername) => {
            bail!("an operator named '{username}' already exists")
        }
        Err(err) => Err(err.into()),
    }
}

fn parse_role(args: &[String]) -> anyhow::Result<OperatorRole> {
    let mut role = OperatorRole::Admin;
    let mut rest = args.iter();

    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--role" => {
                let value = rest.next().context("--role needs a value")?;
                role = match value.as_str() {
                    "admin" => OperatorRole::Admin,
                    "viewer" => OperatorRole::Viewer,
                    other => bail!("unknown role '{other}', expected admin or viewer"),
                };
            }
            other => bail!("unexpected argument '{other}'"),
        }
    }

    Ok(role)
}

fn read_password() -> anyhow::Result<String> {
    if let Ok(password) =
        std::env::var("CASINO_OPERATOR_PASSWORD").map(|value| value.trim().to_string())
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
