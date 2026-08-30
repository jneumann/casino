use std::time::Instant;

use actix_web::{HttpResponse, get, web};
use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::auth::OperatorUser;
use crate::error::ApiError;
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct StatusResponse {
    pub generated_at: DateTime<Utc>,
    pub overall: Health,
    pub service: ServiceStatus,
    pub database: DatabaseStatus,
    pub checks: Vec<Check>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Health {
    Ok,
    Degraded,
    Down,
}

impl Health {
    fn severity(self) -> u8 {
        match self {
            Self::Ok => 0,
            Self::Degraded => 1,
            Self::Down => 2,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ServiceStatus {
    pub name: &'static str,
    pub version: &'static str,
    pub environment: String,
    pub started_at: DateTime<Utc>,
    pub uptime_seconds: i64,
}

#[derive(Debug, Serialize)]
pub struct DatabaseStatus {
    pub backend: &'static str,
    pub reachable: bool,
    pub latency_ms: Option<f64>,
    pub users: Option<i64>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Check {
    pub name: &'static str,
    pub health: Health,
    pub detail: String,
}

#[get("/status")]
pub async fn status(
    state: web::Data<AppState>,
    _identity: OperatorUser,
) -> Result<HttpResponse, ApiError> {
    let database = probe_database(&state).await;
    let checks = collect_checks(&state, &database);

    let overall = checks
        .iter()
        .map(|check| check.health)
        .max_by_key(|candidate| candidate.severity())
        .unwrap_or(Health::Ok);

    let response = StatusResponse {
        generated_at: Utc::now(),
        overall,
        service: ServiceStatus {
            name: env!("CARGO_PKG_NAME"),
            version: env!("CARGO_PKG_VERSION"),
            environment: state.config.environment.clone(),
            started_at: state.started_at,
            uptime_seconds: state.uptime_seconds(),
        },
        database,
        checks,
    };

    Ok(HttpResponse::Ok().json(response))
}

/// Unauthenticated liveness probe for load balancers and uptime monitors.
#[get("/health")]
pub async fn health(state: web::Data<AppState>) -> HttpResponse {
    match state.store.ping().await {
        Ok(()) => HttpResponse::Ok().json(serde_json::json!({
            "status": "ok",
            "database": "up",
        })),
        Err(err) => {
            tracing::error!(error = %err, "health probe failed");
            HttpResponse::ServiceUnavailable().json(serde_json::json!({
                "status": "unavailable",
                "database": "down",
            }))
        }
    }
}

async fn probe_database(state: &AppState) -> DatabaseStatus {
    let backend = state.store.backend();
    let started = Instant::now();

    match state.store.ping().await {
        Ok(()) => {
            let latency_ms = started.elapsed().as_secs_f64() * 1000.0;
            let users = state.store.user_count().await.ok();

            DatabaseStatus {
                backend,
                reachable: true,
                latency_ms: Some((latency_ms * 100.0).round() / 100.0),
                users,
                error: None,
            }
        }
        Err(err) => {
            tracing::error!(error = %err, "database probe failed");
            DatabaseStatus {
                backend,
                reachable: false,
                latency_ms: None,
                users: None,
                error: Some(err.to_string()),
            }
        }
    }
}

/// The list of panels rendered on the status page. Casino-specific checks
/// (table availability, wallet service, game servers) get appended here.
fn collect_checks(state: &AppState, database: &DatabaseStatus) -> Vec<Check> {
    let mut checks = vec![Check {
        name: "api",
        health: Health::Ok,
        detail: format!("serving for {}", format_duration(state.uptime_seconds())),
    }];

    checks.push(match (database.reachable, database.latency_ms) {
        (false, _) => Check {
            name: "database",
            health: Health::Down,
            detail: database
                .error
                .clone()
                .unwrap_or_else(|| "unreachable".to_string()),
        },
        (true, Some(latency)) if latency > 250.0 => Check {
            name: "database",
            health: Health::Degraded,
            detail: format!("slow response ({latency:.2} ms)"),
        },
        (true, latency) => Check {
            name: "database",
            health: Health::Ok,
            detail: match latency {
                Some(latency) => format!("{} responding in {latency:.2} ms", database.backend),
                None => format!("{} responding", database.backend),
            },
        },
    });

    checks
}

fn format_duration(seconds: i64) -> String {
    let days = seconds / 86_400;
    let hours = (seconds % 86_400) / 3_600;
    let minutes = (seconds % 3_600) / 60;
    let secs = seconds % 60;

    if days > 0 {
        format!("{days}d {hours}h {minutes}m")
    } else if hours > 0 {
        format!("{hours}h {minutes}m")
    } else if minutes > 0 {
        format!("{minutes}m {secs}s")
    } else {
        format!("{secs}s")
    }
}
