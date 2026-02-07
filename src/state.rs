use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::sync::Arc;
use tokio::sync::RwLock;
use web_push::SubscriptionInfo;

use crate::push_service::PushService;

const SUBSCRIPTIONS_FILE: &str = "subscriptions.json";
const DEFAULT_CHANNEL: &str = "default";

/// Satu subscription push + daftar channel (gaya Pusher).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StoredSubscription {
    pub endpoint: String,
    pub keys: SubscriptionKeys,
    #[serde(default)]
    pub channels: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SubscriptionKeys {
    pub p256dh: String,
    pub auth: String,
}

impl StoredSubscription {
    pub fn to_subscription_info(&self) -> SubscriptionInfo {
        SubscriptionInfo::new(
            self.endpoint.clone(),
            self.keys.p256dh.clone(),
            self.keys.auth.clone(),
        )
    }
}

/// Format simpan: array subscriptions (mendukung backward compat load dari by_endpoint).
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct SubscriptionStore {
    #[serde(default)]
    pub subscriptions: Vec<StoredSubscription>,
}

impl SubscriptionStore {
    /// Menambah atau memperbarui subscription (merge channels by endpoint).
    pub fn add(&mut self, endpoint: String, keys: SubscriptionKeys, channels: Vec<String>) {
        let channels = if channels.is_empty() {
            vec![DEFAULT_CHANNEL.to_string()]
        } else {
            channels
        };
        if let Some(stored) = self.subscriptions.iter_mut().find(|s| s.endpoint == endpoint) {
            stored.keys = keys;
            for ch in channels {
                if !stored.channels.contains(&ch) {
                    stored.channels.push(ch);
                }
            }
        } else {
            self.subscriptions.push(StoredSubscription {
                endpoint,
                keys,
                channels,
            });
        }
    }

    /// Semua subscription (untuk broadcast / notify lama).
    pub fn all(&self) -> Vec<SubscriptionInfo> {
        self.subscriptions
            .iter()
            .map(StoredSubscription::to_subscription_info)
            .collect()
    }

    /// Subscription yang berlangganan minimal salah satu channel yang diberikan.
    pub fn by_channels(&self, channels: &[String]) -> Vec<SubscriptionInfo> {
        if channels.is_empty() {
            return self.all();
        }
        self.subscriptions
            .iter()
            .filter(|s| s.channels.iter().any(|c| channels.contains(c)))
            .map(StoredSubscription::to_subscription_info)
            .collect()
    }

    pub fn len(&self) -> usize {
        self.subscriptions.len()
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
    let value: serde_json::Value = serde_json::from_str(&data)?;
    let store = if let Some(obj) = value.get("by_endpoint") {
        // Format lama: by_endpoint -> { url: SubscriptionInfo }
        let by_endpoint = obj.as_object().ok_or_else(|| anyhow::anyhow!("by_endpoint not object"))?;
        let mut subscriptions = Vec::new();
        for (_url, v) in by_endpoint {
            if let Some(ep) = v.get("endpoint").and_then(|x| x.as_str()) {
                let keys = v.get("keys").ok_or_else(|| anyhow::anyhow!("missing keys"))?;
                let p256dh = keys.get("p256dh").and_then(|x| x.as_str()).unwrap_or("").to_string();
                let auth = keys.get("auth").and_then(|x| x.as_str()).unwrap_or("").to_string();
                subscriptions.push(StoredSubscription {
                    endpoint: ep.to_string(),
                    keys: SubscriptionKeys { p256dh, auth },
                    channels: vec![DEFAULT_CHANNEL.to_string()],
                });
            }
        }
        SubscriptionStore { subscriptions }
    } else {
        serde_json::from_value(value).unwrap_or_default()
    };
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
