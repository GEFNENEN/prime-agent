//! The RPC mode's automatic threshold compaction: the daemon worker runs its
//! turn-boundary check through `AgentSessionEngine`; the in-process RPC engine
//! drives the agent directly, so this module owns the same check at the
//! points the RPC pump and prompt admission observe.

use std::sync::atomic::Ordering;
use std::sync::Arc;

use pa_core::session_engine::compact_session::CompactOutcome;

use super::commands::{compaction_frame_with_reason, resume_pump, RpcState};
use super::session::RpcEngineHandle;
use super::COMPACT_FRAME_FLUSH_BUDGET;

/// Run one threshold compaction when the live context crossed the effective
/// limit. Returns whether a compaction attempt ran (success, skip, or
/// failure); the caller's turn boundary continues either way.
pub(crate) async fn auto_compact_if_due(state: &Arc<RpcState>, handle: &RpcEngineHandle) -> bool {
    if state.compacting.load(Ordering::SeqCst) > 0
        || state.session.is_replacing()
        || state.session.shutdown_fired()
    {
        return false;
    }
    if !handle
        .engine
        .session
        .auto_compaction_due(&handle.model)
        .await
    {
        return false;
    }
    // Serialize the context rebuild with `/compact` and refine. A manual
    // compaction already holding the lane wins; this check yields to it.
    let Ok(_ops) = state.session_ops.try_lock() else {
        return false;
    };
    // The context may have been compacted while the lane was contested.
    if !handle
        .engine
        .session
        .auto_compaction_due(&handle.model)
        .await
    {
        return false;
    }
    state.compacting.fetch_add(1, Ordering::SeqCst);
    state
        .session
        .write_connection_output(compaction_frame_with_reason(
            "compaction_start",
            "threshold",
            None,
            None,
        ))
        .await;
    // The summarizer's pre-request CPU span does not yield; flush the start
    // frame first so the client's loader appears at the boundary.
    state.writer.drain_within(COMPACT_FRAME_FLUSH_BUDGET).await;
    let outcome = handle
        .engine
        .session
        .compact(None, &handle.model, handle.api_key.clone(), None)
        .await;
    match &outcome {
        Ok(CompactOutcome::Ran(run)) => {
            if let Some(telemetry) = handle.engine.telemetry.as_ref() {
                telemetry.note_compaction(Some(run.duration_ms));
            }
            // The post-compaction kernel notice rides between the start and
            // the settled end (the daemon's event order).
            if let Some(message) = &run.ipython_state {
                super::prompt_commands::write_command_row(state, message).await;
            }
            let result = crate::compaction::compaction_result_value(&run.result, &run.entry);
            state
                .session
                .write_connection_output(compaction_frame_with_reason(
                    "compaction_end",
                    "threshold",
                    None,
                    Some(&result),
                ))
                .await;
        }
        Ok(CompactOutcome::Skipped(message)) => {
            eprintln!("pa-daemon: RPC auto-compaction skipped: {message}");
            state
                .session
                .write_connection_output(compaction_frame_with_reason(
                    "compaction_end",
                    "threshold",
                    None,
                    None,
                ))
                .await;
        }
        Err(error) => {
            eprintln!("pa-daemon: RPC auto-compaction failed: {error:#}");
            state
                .session
                .write_connection_output(compaction_frame_with_reason(
                    "compaction_end",
                    "threshold",
                    None,
                    None,
                ))
                .await;
        }
    }
    state.compacting.fetch_sub(1, Ordering::SeqCst);
    // A prompt/steer parked behind the compaction gate may deliver now.
    resume_pump(state);
    true
}
