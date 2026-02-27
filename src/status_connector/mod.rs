//! Sidecar status connector — publishes agent status to Redis for pod lifecycle signaling.
//!
//! When enabled, the agent publishes status changes (starting, working, completed_awaiting)
//! to Redis. The sidecar can poll or subscribe to detect when the agent has finished
//! all work and been idle long enough, allowing safe pod termination.
//!
//! Requires `status-redis` feature.

mod traits;

pub use traits::{AgentStatus, StatusConnector};

#[cfg(feature = "status-redis")]
mod redis_connector;

#[cfg(feature = "status-redis")]
pub use redis_connector::RedisStatusConnector;

/// Create a status connector from config, or None if disabled.
pub fn create_status_connector(
    config: &crate::config::SidecarStatusConfig,
) -> Option<std::sync::Arc<dyn StatusConnector>> {
    if !config.enabled || config.redis_url.trim().is_empty() {
        return None;
    }

    #[cfg(feature = "status-redis")]
    {
        match RedisStatusConnector::new(config) {
            Ok(conn) => Some(std::sync::Arc::new(conn)),
            Err(e) => {
                tracing::warn!("Failed to create Redis status connector: {e:#}");
                None
            }
        }
    }

    #[cfg(not(feature = "status-redis"))]
    {
        tracing::warn!(
            "Sidecar status enabled but status-redis feature not compiled in; status will not be published"
        );
        None
    }
}
