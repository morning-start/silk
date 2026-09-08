use std::collections::HashMap;
use std::sync::Arc;

use sqlx::SqlitePool;
use tokio::sync::RwLock;

use crate::gateway::{GatewayContext, GatewayServerHandle};

#[derive(Debug, Clone, Default)]
pub struct LookupCache {
    pub provider_names: HashMap<String, String>,
    pub model_mapping_names: HashMap<String, String>,
}

#[derive(Clone)]
pub struct AppState {
    pub gateway: Arc<RwLock<GatewayContext>>,
    pub gateway_server: Arc<RwLock<Option<GatewayServerHandle>>>,
    pub lookup_cache: Arc<RwLock<LookupCache>>,
    pub settings_change_tx: tokio::sync::broadcast::Sender<()>,
}

impl AppState {
    pub async fn invalidate_cache(&self, id: &str) {
        self.gateway.read().await.provider_cache.invalidate(id).await;
    }

    pub async fn refresh_lookup(&self) {
        if let Some(pool) = crate::get_db_pool() {
            let cache = load_lookup_cache(pool).await;
            *self.lookup_cache.write().await = cache;
        }
    }
}

pub async fn load_lookup_cache(pool: &SqlitePool) -> LookupCache {
    crate::persistence::LookupCacheRepo::load(pool).await
}
