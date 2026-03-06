//! Status connector traits — abstraction for publishing agent status.

use async_trait::async_trait;

/// Agent status values published to the sidecar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentStatus {
    /// Agent process has started and is initializing.
    Starting,

    /// Agent is actively processing a user request.
    Working,

    /// Agent has completed all in-flight work and been idle for `idle_timeout_secs`.
    /// Safe for sidecar to consider pod termination.
    CompletedAwaiting,
}

impl AgentStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Starting => "starting",
            Self::Working => "working",
            Self::CompletedAwaiting => "completed_awaiting",
        }
    }
}

/// Status connector — publishes agent status for sidecar integration.
#[async_trait]
pub trait StatusConnector: Send + Sync {
    /// Publish current status. Non-blocking; errors are logged.
    async fn publish(&self, status: AgentStatus);

    /// Called when all workers have completed and agent is idle. Schedules a delayed
    /// publish of `CompletedAwaiting` after `idle_timeout_secs`, unless `on_new_message`
    /// is called first.
    fn schedule_idle_completion(&self);

    /// Called when a new message is received. Cancels any pending idle completion
    /// and publishes `Working`.
    fn on_new_message(&self);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_status_as_str_maps_correctly() {
        assert_eq!(AgentStatus::Starting.as_str(), "starting");
        assert_eq!(AgentStatus::Working.as_str(), "working");
        assert_eq!(AgentStatus::CompletedAwaiting.as_str(), "completed_awaiting");
    }
}
