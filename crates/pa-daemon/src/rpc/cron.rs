//! The RPC embedding's cron/heartbeat scheduler: the engine build wires the
//! session-artifact store, and this mode fires its due jobs into the live
//! agent's queue. The daemon worker's scheduler owns the same contract; the
//! RPC connection owns no supervisor, so the hooks deliver locally.

use std::sync::Arc;

use pa_agent::types::{AgentMessage, CustomAgentMessage};
use pa_core::cron::scheduler::{AgentCronScheduler, AgentCronSchedulerHooks};
use pa_core::cron::{should_defer_heartbeat_cron_job, DeliveryMode, HeartbeatSessionActivity};
use pa_core::session_engine::messages::create_heartbeat_prompt_message;

use super::commands::RpcState;

/// The scheduler hooks over the live RPC connection. The weak back-reference
/// keeps the scheduler (owned by `RpcState`) from forming a cycle.
pub(crate) struct RpcCronHooks {
    state: std::sync::Weak<RpcState>,
}

impl AgentCronSchedulerHooks for RpcCronHooks {
    async fn run_job(
        &self,
        job: &pa_core::cron::AgentCronJob,
    ) -> anyhow::Result<Option<&'static str>> {
        let Some(state) = self.state.upgrade() else {
            return Ok(Some("skipped"));
        };
        let handle = state.session.handle().await;
        // A replacement swaps the handle before the scheduler restart:
        // never deliver the retired session's job into the new engine.
        let Some(active) = handle.cron_active_session_id.as_deref() else {
            return Ok(Some("skipped"));
        };
        if job.session_id != active && job.active_session_id != active {
            return Ok(Some("skipped"));
        }
        let engine = handle.engine.clone();
        let agent = engine.session.agent();
        let snapshot = agent.state().await;
        let activity = HeartbeatSessionActivity {
            is_streaming: snapshot.is_streaming,
            is_compacting: state.compacting.load(std::sync::atomic::Ordering::SeqCst) > 0,
            // The RPC transport has no retry/bash slots distinct from the
            // agent's own state; queued work is the deferral signal it has.
            is_retrying: false,
            is_bash_running: false,
            has_pending_session_work: agent.has_queued_messages(),
            unfinished_action_count: 0,
        };
        if should_defer_heartbeat_cron_job(job, &activity) {
            return Ok(Some("skipped"));
        }
        // The same durable heartbeat row the daemon worker enqueues: the
        // kernel skill's prompt rides the heartbeat custom message.
        let row = create_heartbeat_prompt_message(job, crate::util::now_ms());
        let custom = CustomAgentMessage {
            role: "custom".to_string(),
            payload: serde_json::to_value(&row)?,
        };
        let message = AgentMessage::Custom(custom);
        if matches!(job.delivery_mode, Some(DeliveryMode::FollowUp)) {
            agent.follow_up(message);
        } else {
            agent.steer(message);
        }
        drop(handle);
        super::commands::resume_pump(&state);
        super::commands::kick_queue_pump(&state, &engine);
        Ok(None)
    }
}

/// Rebuild the scheduler for the live engine: a whole-session replacement
/// swaps both the store and the binding, so the old timer must stop.
pub(crate) async fn restart_cron_scheduler(state: &Arc<RpcState>) {
    let store = {
        let handle = state.session.handle().await;
        handle.cron_store.clone()
    };
    let mut slot = state.cron_scheduler.lock().await;
    if let Some(old) = slot.take() {
        old.stop().await;
    }
    let Some(store) = store else {
        return;
    };
    let hooks = Arc::new(RpcCronHooks {
        state: Arc::downgrade(state),
    });
    let scheduler = Arc::new(AgentCronScheduler::new(store, hooks));
    scheduler.start().await;
    *slot = Some(scheduler);
}

/// Re-arm the scheduler after a catalog read/poll: `wake` reschedules the
/// timer from the store's next due job.
pub(crate) async fn wake_cron_scheduler(state: &Arc<RpcState>) {
    let scheduler = { state.cron_scheduler.lock().await.clone() };
    if let Some(scheduler) = scheduler {
        scheduler.wake().await;
    }
}

/// Poll the store so a heartbeat created/updated after the scheduler's last
/// timer arm is picked up without a mutation hook (the RPC build has no
/// daemon supervisor to signal it).
pub(crate) fn serve_cron_scheduler(state: Arc<RpcState>) {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(15)).await;
            wake_cron_scheduler(&state).await;
        }
    });
}
