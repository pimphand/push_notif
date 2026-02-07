use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use web_push::SubscriptionInfo;

use crate::push_service::PushService;

const SUBSCRIPTIONS_FILE: &str = "subscriptions.json";

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct SubscriptionStore {
    pub by_endpoint: HashMap<String, SubscriptionInfo>,
}

impl SubscriptionStore {
    pub fn add(&mut self, sub: SubscriptionInfo) {
        self.by_endpoint.insert(sub.endpoint.clone(), sub);
    }

    pub fn all(&self) -> Vec<SubscriptionInfo> {
        self.by_endpoint.values().cloned().collect()
    }

    pub fn len(&self) -> usize {
        self.by_endpoint.len()
    }
}

#[derive(Clone, Default)]
pub struct LastNotification {
    pub id: u64,
    pub title: String,
    pub body: String,
}

#[derive(Clone)]
pub struct AppState {
    pub push_service: Arc<PushService>,
    pub subscriptions: Arc<RwLock<SubscriptionStore>>,
    pub last_notification: Arc<RwLock<Option<LastNotification>>>,
    pub db: PgPool,
    pub jwt_secret: Arc<[u8]>,
}

impl AppState {
    pub async fn new() -> anyhow::Result<Self> {
        let push_service = PushService::new()?;
        let subscriptions = load_subscriptions().await.unwrap_or_default();
        let db = crate::db::create_pool().await?;
        crate::db::run_migrations(&db).await?;
        crate::db::seed_admin_if_empty(&db).await?;
        let jwt_secret = std::env::var("JWT_SECRET")
            .unwrap_or_else(|_| "push-notif-secret-change-in-production".to_string());
        let jwt_secret = Arc::from(jwt_secret.as_bytes());
        Ok(Self {
            push_service: Arc::new(push_service),
            subscriptions: Arc::new(RwLock::new(subscriptions)),
            last_notification: Arc::new(RwLock::new(None)),
            db,
            jwt_secret,
        })
    }
}

async fn load_subscriptions() -> anyhow::Result<SubscriptionStore> {
    let data = tokio::fs::read_to_string(SUBSCRIPTIONS_FILE).await?;
    let store: SubscriptionStore = serde_json::from_str(&data)?;
    Ok(store)
}

pub async fn save_subscriptions(store: &SubscriptionStore) -> anyhow::Result<()> {
    let data = serde_json::to_string_pretty(store)?;
    if let Some(p) = std::path::Path::new(SUBSCRIPTIONS_FILE).parent() {
        let _ = tokio::fs::create_dir_all(p).await;
    }
    tokio::fs::write(SUBSCRIPTIONS_FILE, data).await?;
    Ok(())
}
