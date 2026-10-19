use super::{
    auxiliary_model, compaction, compaction_exec, image_model_routing, ipython_state,
    provider_adapter, refine, semantic_edges, telemetry, turn_hook, AgentSession, PromptBatchRow,
};

impl AgentSession {
    /// Install the image-model routing host seam; `None` keeps image turns on the session model.
    pub fn set_image_model_router(
        &mut self,
        router: Option<image_model_routing::ImageModelRouter>,
    ) {
        self.image_model_router = router;
    }

    /// The dispatch-time routing decision for one admitted batch (TS `_imageModelOverrideForTurns`
    /// at commit): when the batch attaches image blocks the session model cannot serve, the host's
    /// route takes the stream target and the agent's per-run override. EVERY admitted batch
    /// re-evaluates the route, so the next image-free batch returns to the session model.
    pub(super) async fn apply_image_model_routing(
        &self,
        images: &[pa_agent::types::ImageContent],
        batch: &[PromptBatchRow],
    ) -> anyhow::Result<()> {
        let Some(router) = self.image_model_router.as_ref() else {
            return Ok(());
        };
        // The prior episode never outlives this admission (TS `_clearModelOverrideWhenIdle`): an
        // explicit selection wins over routing lingering from the last dispatched turn, and the
        // settle runs only while no run streams.
        if !self.agent.state().await.is_streaming {
            (router.swap_target)(None);
            self.agent.set_model_override(None);
        }
        let carries_images = !images.is_empty() || batch.iter().any(|row| !row.images.is_empty());
        // The live thinking level (the agent state's, matching `request_output_budget`'s
        // read) rides the decision: a mid-run `/effort` or model switch must not
        // route with the build-time level.
        let live_level =
            provider_adapter::model_thinking_level(self.agent.state().await.thinking_level);
        let route = (router.decide)(carries_images, live_level).map_err(anyhow::Error::msg)?;
        let Some(resolved) = route.as_ref() else {
            (router.swap_target)(None);
            self.agent.set_model_override(None);
            return Ok(());
        };
        (router.swap_target)(Some(resolved));
        // The conversion failure happens AFTER the swap armed the routed target:
        // unwind the route so the failed turn does not leave it serving the next batch.
        let Some(agent_model) =
            crate::session_engine::provider_adapter::json_round_trip(&resolved.model)
        else {
            (router.swap_target)(None);
            self.agent.set_model_override(None);
            return Err(anyhow::anyhow!("model conversion failed"));
        };
        self.agent
            .set_model_override(Some(pa_agent::agent::AgentModelOverride {
                model: agent_model,
                thinking_level: crate::session_engine::provider_adapter::map_thinking_level(
                    resolved.thinking_level,
                ),
            }));
        Ok(())
    }

    /// Override the compaction settings from the session's resolved settings so
    /// `/compact` honors `compaction.keepRecentTokens`/`reserveTokens`.
    pub fn set_compaction_settings(&self, settings: compaction::CompactionSettings) {
        *self
            .compaction
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = settings;
    }

    /// Toggle automatic compaction for this session: the live settings
    /// the auto-compaction arms and `/compact` read.
    pub fn set_auto_compaction_enabled(&self, enabled: bool) {
        let mut settings = self
            .compaction
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        settings.enabled = enabled;
    }

    /// Install the auxiliary-model routing context: compaction summaries resolve through the
    /// `auxiliaryModel` setting. Without it every summarizer stays on the session model.
    pub fn set_auxiliary_model_context(&mut self, context: auxiliary_model::AuxiliaryModelContext) {
        self.auxiliary_model = Some(context);
    }

    /// Install the skill inventory `/skill:<name>` submissions expand
    /// against (TS reads the resource loader at expansion time; the Rust
    /// session snapshots the engine's loaded list here).
    pub fn set_skills(&mut self, skills: Vec<crate::skills::Skill>) {
        self.skills = skills;
    }

    /// Bind the telemetry handle the `skill_use_count` counter counts
    /// through (the engine wiring owns the telemetry lifetime and
    /// installs it once the session telemetry is assembled).
    pub fn set_skill_telemetry(&mut self, telemetry: std::sync::Arc<telemetry::SessionTelemetry>) {
        self.skill_telemetry = Some(telemetry);
    }

    /// Bind the auto-refine surface for this session: whether the session may
    /// auto-refine (TS `_autoRefineAllowedForSession`: depth 0 with a local
    /// harness state dir) and the resolved gates.
    pub fn set_auto_refine(&mut self, allowed: bool, gates: refine::AutoRefineGates) {
        self.auto_refine_allowed = allowed;
        self.auto_refine = gates;
    }

    /// Bind the session's agent dir (the settings root). The refine flow
    /// reads the `factory.enabled` opt-in from the agent dir's
    /// settings.json on every run — the same live read the kernel-side
    /// factory gate performs — so a session without a wired agent dir
    /// keeps the fail-closed disabled default.
    pub fn set_agent_dir(&mut self, agent_dir: std::path::PathBuf) {
        self.agent_dir = Some(agent_dir);
    }

    /// Install the configured per-turn system-prompt hook (`turnHook`
    /// settings). The hook runs at each admitted user turn; `None` keeps
    /// the feature off.
    pub fn set_turn_hook(&mut self, hook: Option<turn_hook::TurnHook>) {
        self.turn_hook = hook;
    }

    /// Share the slot the request transform drains: the engine builds the
    /// transform before the session exists, so both must hold the same
    /// `Arc`. A session without the wiring keeps its own inert slot.
    pub fn set_retry_nudge_slot(&mut self, slot: std::sync::Arc<std::sync::Mutex<Option<String>>>) {
        self.retry_nudge = slot;
    }

    /// Queue a one-shot nudge for the next assembled request. The
    /// transform drains it into a trailing user turn without touching the
    /// persisted history (the empty-reply retry path).
    pub fn queue_retry_nudge(&self, text: impl Into<String>) {
        *self
            .retry_nudge
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(text.into());
    }

    /// Install the configured agent-end write-back hook (`stopHook`
    /// settings): subscribes the loop listener that renders each finished
    /// run and runs the hook command. The listener renders and spawns the
    /// command inline (the loop awaits listeners before it settles, so the
    /// child always lands while the host process is alive); only the wait
    /// is detached, so neither a slow hook nor a failed one can fail a
    /// run. `None` keeps the feature off.
    pub async fn set_stop_hook(&self, hook: Option<std::sync::Arc<super::stop_hook::StopHook>>) {
        let Some(hook) = hook else {
            return;
        };
        let session = std::sync::Arc::clone(&self.session);
        self.agent
            .subscribe(move |event, _signal| {
                let hook = std::sync::Arc::clone(&hook);
                let session = std::sync::Arc::clone(&session);
                Box::pin(async move {
                    if let pa_agent::types::AgentEvent::AgentEnd { messages } = event {
                        let (session_id, cwd) = {
                            let session = session.lock().await;
                            (
                                session.get_session_id().to_string(),
                                session.get_cwd().display().to_string(),
                            )
                        };
                        hook.note_agent_end(&session_id, &cwd, &messages);
                    }
                    Ok(())
                })
            })
            .await;
    }

    /// Bind the kernel-state probe behind the post-compaction
    /// `ipython_state` notice (the engine wiring hands over the session's
    /// kernel provisioner, TS `AgentSession._ipythonKernelProvisioner`).
    /// Without a probe no notice lands: sessions without a kernel keep
    /// the pre-notice compaction flow.
    pub fn set_kernel_state_probe(
        &mut self,
        probe: Option<std::sync::Arc<dyn ipython_state::CompactionKernelProbe>>,
    ) {
        self.kernel_state = probe;
    }

    /// Install the live compaction summary-delta sink (the daemon's
    /// `compaction_summary_delta` broadcast seam): every summarizer delta reaches the
    /// sink while the summary generates, in arrival order.
    ///
    /// # Panics
    ///
    /// Panics when the sink slot's mutex is poisoned.
    pub fn set_compaction_summary_sink(&self, sink: compaction_exec::SummaryDeltaSink) {
        *self
            .compaction_summary_sink
            .lock()
            .expect("compaction summary sink lock") = Some(sink);
    }

    /// Whether the session may run auto-refinement.
    pub fn auto_refine_allowed(&self) -> bool {
        self.auto_refine_allowed && self.harness_refinements_enabled()
    }

    /// The resolved auto-refine gates.
    pub fn auto_refine_gates(&self) -> refine::AutoRefineGates {
        self.auto_refine
    }

    /// Whether automatic compaction is enabled for this session: the
    /// gate the automatic arms check before any trigger.
    pub fn auto_compaction_enabled(&self) -> bool {
        self.compaction
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .enabled
    }

    /// The resolved compaction settings: the in-run continuation consult reads the threshold
    /// headroom without owning the session (a compaction in flight owns it across its model turn).
    pub fn compaction_settings(&self) -> compaction::CompactionSettings {
        let mut resolved = *self
            .compaction
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        // Read only the percentage at decision boundaries, off the paint path.
        // Like auxiliary-model routing, another client's settings save applies
        // to an already-running session without changing manual summary budgets.
        if let Some(context) = &self.auxiliary_model {
            resolved.threshold_percent =
                crate::settings::SettingsManager::create(&context.cwd, &context.agent_dir)
                    .get_compaction_threshold_percent();
        }
        resolved
    }

    /// Install the session's semantic-edge recorder (the engine build's
    /// handoff; the daemon's child registry and retry park read it back
    /// through [`AgentSession::semantic_edges`]).
    pub fn set_semantic_edges(
        &self,
        recorder: Option<std::sync::Arc<semantic_edges::SemanticEdgeRecorder>>,
    ) {
        *self
            .semantic_edges
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = recorder;
    }

    /// The session's semantic-edge recorder, when the engine built the
    /// session with a semantic identity.
    #[must_use]
    pub fn semantic_edges(&self) -> Option<std::sync::Arc<semantic_edges::SemanticEdgeRecorder>> {
        self.semantic_edges
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    /// Install the pre-semantic stream fn calls outside session history run
    /// on (TS `unwrapSemanticEdgeStreamFn`; the engine wires the
    /// timing-instrumented fn it wrapped semantic edges around).
    pub fn set_side_question_stream_fn(&self, stream_fn: pa_agent::stream::StreamFn) {
        *self
            .side_question_stream_fn
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(stream_fn);
    }

    /// The pre-semantic stream fn for calls outside session history (side
    /// questions carry no request id).
    #[must_use]
    pub fn side_question_stream_fn(&self) -> Option<pa_agent::stream::StreamFn> {
        self.side_question_stream_fn
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}
