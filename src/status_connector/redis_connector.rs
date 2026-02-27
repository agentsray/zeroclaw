//! Redis-backed status connector.

use super::traits::{AgentStatus, StatusConnector};
use async_trait::async_trait;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

use crate::config::SidecarStatusConfig;

async fn publish_to_redis(redis_url: &str, key: &str, status: AgentStatus) {
    let value = {
        let now = chrono::Utc::now();
        serde_json::json!({
            "status": status.as_str(),
            "updated_at": now.to_rfc3339(),
        })
        .to_string()
    };

    let client = match redis::Client::open(redis_url) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("Redis status connector: failed to create client: {e}");
            return;
        }
    };

    let mut conn = match redis::aio::ConnectionManager::new(client).await {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("Redis status connector: failed to connect: {e}");
            return;
        }
    };

    if let Err(e) = redis::cmd("SET")
        .arg(key)
        .arg(&value)
        .query_async::<()>(&mut conn)
        .await
    {
        tracing::warn!("Redis status connector: failed to SET {key}: {e}");
    }
}

/// Redis status connector — publishes status to a Redis key.
pub struct RedisStatusConnector {
    redis_url: String,
    key: String,
    idle_timeout_secs: u64,
    cancel_idle: Arc<Mutex<Option<tokio_util::sync::CancellationToken>>>,
}

impl RedisStatusConnector {
    pub fn new(config: &SidecarStatusConfig) -> anyhow::Result<Self> {
        let agent_id = config
            .agent_id
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or("default")
            .to_string();
        let user_id = config
            .user_id
            .as_ref()
            .filter(|s| !s.trim().is_empty());
        let key_prefix = config
            .key_prefix
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or("zeroclaw:agent");
        let key = if let Some(uid) = user_id {
            format!("{}:{}:{}:status", key_prefix, agent_id, uid)
        } else {
            format!("{}:{}:status", key_prefix, agent_id)
        };
        Ok(Self {
            redis_url: config.redis_url.clone(),
            key,
            idle_timeout_secs: config.idle_timeout_secs,
            cancel_idle: Arc::new(Mutex::new(None)),
        })
    }
}

#[async_trait]
impl StatusConnector for RedisStatusConnector {
    async fn publish(&self, status: AgentStatus) {
        publish_to_redis(&self.redis_url, &self.key, status).await;
    }

    fn schedule_idle_completion(&self) {
        let cancel_token = tokio_util::sync::CancellationToken::new();
        let token_clone = cancel_token.clone();
        let idle_timeout = self.idle_timeout_secs;
        let redis_url = self.redis_url.clone();
        let key = self.key.clone();

        let prev = {
            let mut guard = self.cancel_idle.blocking_lock();
            let prev = guard.take();
            *guard = Some(cancel_token);
            prev
        };
        if let Some(prev) = prev {
            prev.cancel();
        }

        tokio::spawn(async move {
            let sleep = tokio::time::sleep(Duration::from_secs(idle_timeout));
            tokio::select! {
                _ = sleep => {
                    publish_to_redis(&redis_url, &key, AgentStatus::CompletedAwaiting).await;
                    tracing::debug!("Published status completed_awaiting after {}s idle", idle_timeout);
                }
                _ = token_clone.cancelled() => {
                    tracing::debug!("Idle completion cancelled (new message received)");
                }
            }
        });
    }

    fn on_new_message(&self) {
        let prev = {
            let mut guard = self.cancel_idle.blocking_lock();
            guard.take()
        };
        if let Some(token) = prev {
            token.cancel();
        }

        let redis_url = self.redis_url.clone();
        let key = self.key.clone();
        tokio::spawn(async move {
            publish_to_redis(&redis_url, &key, AgentStatus::Working).await;
        });
    }
}
