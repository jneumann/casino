use std::sync::Arc;

use chrono::{DateTime, Utc};

use crate::auth::TokenService;
use crate::config::Config;
use crate::db::DynStore;

pub struct AppState {
    pub config: Arc<Config>,
    pub store: DynStore,
    pub tokens: TokenService,
    pub started_at: DateTime<Utc>,
}

impl AppState {
    pub fn new(config: Config, store: DynStore) -> Self {
        let tokens = TokenService::new(&config.jwt_secret, config.token_ttl);

        Self {
            config: Arc::new(config),
            store,
            tokens,
            started_at: Utc::now(),
        }
    }

    pub fn uptime_seconds(&self) -> i64 {
        (Utc::now() - self.started_at).num_seconds().max(0)
    }
}
