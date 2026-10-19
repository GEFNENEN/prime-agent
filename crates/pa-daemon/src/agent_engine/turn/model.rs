//! The model-turn runner: the streaming run over the built session agent,
//! the retry/failover policy application, and the quota-park mid-run arm.
use super::{
    aborted_message, drop_trailing_assistant, json_round_trip, map_thinking_level,
    retry_event_to_engine_event, AgentSessionEngine, EngineEvent, ProviderTarget, StopReason,
    TurnAdmission, TurnOnce, TurnPrompt, TurnResult,
};

impl AgentSessionEngine {
    /// Drive one admitted prompt through the retry-driver model loop and
    /// emit the turn outcome (provider-failure retries + final-row
    /// surfacing). The trailing `Done` is owned by the caller.
    pub(super) fn run_model_turn(
        &self,
        admission: TurnAdmission,
        prompt: &TurnPrompt,
        boundary_passed: &std::sync::Arc<std::sync::atomic::AtomicBool>,
        aborted: &dyn Fn() -> bool,
        emit: &mut dyn FnMut(EngineEvent) -> bool,
    ) -> TurnResult {
        #[derive(Clone)]
        struct FailoverPrimary {
            model: pa_types::ai::Model,
            thinking_level: pa_agent::types::ThinkingLevel,
            api_key: Option<String>,
            headers: Option<std::collections::BTreeMap<String, String>>,
            session_restored: bool,
        }
        let model = match self.resolve_model() {
            Ok(model) => model,
            Err(error) => {
                return TurnResult::Error {
                    error: error.to_string(),
                    assistant: None,
                };
            }
        };
        // The preflight validates the model SERVING the run: a model whose
        // provider has no configured credential fails the run before the
        // provider request, with the login-guidance message — a routed
        // image-model episode authenticates its own image model.
        let preflight_model = self
            .armed_image_route()
            .map_or_else(|| model.clone(), |route| route.target.model);
        if self.config.faux_script.is_none() && self.current_selection().api_key.is_none() {
            let auth = pa_core::auth::AuthStorage::create(&self.config.agent_dir);
            let mut registry = pa_core::models::ModelRegistry::create(
                auth,
                self.config.agent_dir.join("models.json"),
            );
            registry.load_private_authorization_from_cache();
            if !registry.has_configured_auth(&preflight_model) {
                let uses_oauth = registry
                    .auth
                    .get_all()
                    .credential(&preflight_model.provider)
                    .is_some_and(|credential| {
                        matches!(credential, pa_core::auth::AuthCredential::Oauth { .. })
                    });
                let message = if uses_oauth {
                    format!(
                        "Authentication failed for \"{}\". Credentials may have expired or network is unavailable.\n\nRun /login to update credentials.",
                        preflight_model.provider
                    )
                } else {
                    let docs = pa_core::packages::docs_path();
                    format!(
                        "No API key found for {}.\n\nUse /login to log into a provider via OAuth or API key. See:\n  {}\n  {}",
                        preflight_model.provider,
                        docs.join("providers.md").display(),
                        docs.join("models.md").display()
                    )
                };
                return TurnResult::Error {
                    error: message,
                    assistant: None,
                };
            }
        }
        let agent = match self.session_agent(&model) {
            Ok(agent) => agent,
            Err(error) => {
                return TurnResult::Error {
                    error: format!("{error:#}"),
                    assistant: None,
                };
            }
        };
        // The delivery's cancel flag is consulted at the admission, before
        // the agent run registers: an abort landing in the [pickup,
        // registration] window is otherwise lost. Delivery-scoped by
        // construction.
        if aborted() {
            return TurnResult::Aborted;
        }
        // A routed image-model episode applies its override BEFORE the
        // first provider call: the serving target swaps to the image
        // model (the agent state itself never swaps — per-run, like TS).
        self.apply_armed_image_route(&agent);
        let policy = self.retry_policy();
        let failover_policy = self.failover_policy();
        // A routed episode serves (and fails over within) the ROUTED
        // model: the candidate chain derives from the serving model.
        let candidates = match self.armed_image_route() {
            Some(route) => self.failover_candidates(&route.target.model),
            None => self.failover_candidates(&model),
        };
        // The pa-core retry driver owns the attempt loop; this engine
        // owns one turn. The single `emit` reference is handed through
        // a RefCell slot to whichever closure is currently running.
        let emit_cell = std::cell::RefCell::new(emit);
        // A subagent has no operator to nudge a stalled provider reply, so the
        // stall falls back to a notice to its parent (see
        // `stalled_run_notice`); a top-level session just tells the user.
        let stalled_run_subagent = self.rlm_depth.load(std::sync::atomic::Ordering::Relaxed) > 0;
        // The overflow compact-and-retry re-issues the loop without a new
        // user message, so its turn starts as a continuation (TS
        // `agent.continue()`); ordinary turns start fresh.
        let first_attempt = std::cell::Cell::new(matches!(admission, TurnAdmission::FreshPrompt));
        // Failover switch/restore re-bind the live agent's model, append
        // the model-change row, and re-bind the turn-boundary model facts
        // (`model.info` follows the session model). The first successful
        // backup response restores session state before tools, like TS; the
        // captured run still serves backup traffic until it settles.
        let core = {
            let guard = self.session.blocking_lock();
            guard.as_ref().cloned()
        };
        let persistence = core.as_ref().map(|core| core.session.shared_persistence());
        // Retry/failover adoption telemetry (TS `auto_retry_start` counting):
        // retries increment `retry_count`, provider switches `failover_count`.
        // The semantic-edge recorder rides the same snapshot (TS
        // `prepareTurnRetry` / `clearTurnRetry` ride the retry events: the
        // parked id makes the retried call reuse the failed attempt's
        // idempotency key).
        let (telemetry, semantic_edges) = {
            let guard = self.session.blocking_lock();
            (
                guard.as_deref().and_then(|engine| engine.telemetry.clone()),
                guard
                    .as_deref()
                    .and_then(|engine| engine.session.semantic_edges()),
            )
        };
        // The failover-captured primary target state (TS `_backupModel`):
        // Session state restores at successful message_end; request auth
        // stays on the backup until the active run settles.
        let primary_state = std::sync::Arc::new(std::sync::Mutex::new(None::<FailoverPrimary>));
        let pending_model_changes = std::sync::Arc::new(std::sync::Mutex::new(Vec::<
            pa_types::session::ModelChangeEntry,
        >::new()));
        // The quota-park seam (TS #2375): the retry chain consults the
        // engine at its give-up; a quota failure whose reset exceeds
        // the wait cap parks the session. The weak self keeps the
        // callback `'static`.
        let quota_parked_flag = std::sync::Arc::clone(&self.quota_parked_this_run);
        let engine_weak = self
            .self_weak
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        let park: Option<pa_core::session_engine::provider_park::ParkDecisionCallback> =
            Some(&mut move |message, abort| {
                let engine_weak = engine_weak.clone();
                let abort = abort.to_string();
                let quota_parked_flag = std::sync::Arc::clone(&quota_parked_flag);
                Box::pin(async move {
                    let engine = engine_weak.as_ref().and_then(std::sync::Weak::upgrade)?;
                    let outcome = engine.park_for_quota_reset(&message, &abort).await;
                    if outcome.is_some() {
                        quota_parked_flag.store(true, std::sync::atomic::Ordering::SeqCst);
                    }
                    outcome
                })
            });
        let overflow_window = match self.armed_image_route() {
            Some(route) => route.target.model.context_window,
            None => model.context_window,
        };
        let restore_subscription = self.runtime.block_on(async {
            // Awaited before tool execution. The agent loop has already
            // captured its serving model, so restoring session state here
            // does not redirect the remaining backup provider requests.
            let primary_state = primary_state.clone();
            let pending_model_changes = pending_model_changes.clone();
            let agent_weak = std::sync::Arc::downgrade(&agent);
            let core = core.clone();
            let persistence = persistence.clone();
            agent
                .subscribe(move |event, _signal| {
                    let primary_state = primary_state.clone();
                    let pending_model_changes = pending_model_changes.clone();
                    let agent_weak = agent_weak.clone();
                    let core = core.clone();
                    let persistence = persistence.clone();
                    Box::pin(async move {
                        let pa_agent::types::AgentEvent::MessageEnd {
                            message:
                                pa_agent::types::AgentMessage::Standard(
                                    pa_agent::types::Message::Assistant(assistant),
                                ),
                        } = event
                        else {
                            return Ok(());
                        };
                        if matches!(
                            assistant.stop_reason,
                            StopReason::Error | StopReason::Aborted
                        ) {
                            return Ok(());
                        }
                        let primary = primary_state
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner)
                            .clone();
                        let Some(primary) = primary.filter(|primary| !primary.session_restored)
                        else {
                            return Ok(());
                        };
                        let Some(agent) = agent_weak.upgrade() else {
                            return Ok(());
                        };
                        let model = json_round_trip(&primary.model)
                            .ok_or_else(|| anyhow::anyhow!("model conversion failed"))?;
                        agent
                            .set_model_and_thinking_level(model, primary.thinking_level)
                            .await;
                        if let Some(core) = core {
                            core.update_model_facts(&primary.model);
                        }
                        if let Some(persistence) = persistence {
                            persistence
                                .lock()
                                .await
                                .append_model_change(&primary.model.provider, &primary.model.id)?;
                        }
                        // The later run_once subscriber forwards this after the
                        // assistant row and before tool execution, matching TS.
                        pending_model_changes
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner)
                            .push(pa_types::session::ModelChangeEntry {
                                provider: primary.model.provider.clone(),
                                model_id: primary.model.id.clone(),
                            });
                        if let Some(primary) = primary_state
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner)
                            .as_mut()
                        {
                            primary.session_restored = true;
                        }
                        Ok(())
                    })
                })
                .await
        });
        let restore_primary = || {
            let emit_cell = &emit_cell;
            let primary_state = primary_state.clone();
            let agent = agent.clone();
            let persistence = persistence.clone();
            let core = core.clone();
            let primary = primary_state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone();
            async move {
                let Some(FailoverPrimary {
                    model: primary_model,
                    thinking_level,
                    api_key: primary_api_key,
                    headers: primary_headers,
                    session_restored,
                }) = primary
                else {
                    return Ok(None);
                };
                let agent_model = json_round_trip(&primary_model)
                    .ok_or_else(|| anyhow::anyhow!("model conversion failed"))?;
                {
                    // The restore serializes with `/reload`'s
                    // live-input refresh: without this fence a
                    // reload landing between the restore's
                    // resolution and its slot write is clobbered
                    // by the restore's older pair — the lock
                    // makes whichever runs last leave the newest
                    // store standing. The whole turn (this
                    // closure included) runs on the blocking
                    // pool — the worker parks the turn there
                    // (`worker/turn.rs`) — so the synchronous
                    // credential resolution below, OAuth refresh
                    // and all, never touches an async executor
                    // worker; only a concurrently reloading
                    // session waits on this span.
                    let _reload_serialized = self
                        .reload_lock
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    // The armed-route install goes through the
                    // fenced helper (no provider-slot lock held
                    // across the route read).
                    if !self.install_armed_route_target() {
                        // The primary's request auth re-resolves at
                        // restore: a credential rotated (and
                        // reloaded) during the failover serves from
                        // the store, never the pre-failover
                        // capture.
                        let target = self.restored_primary_target(
                            &primary_model,
                            primary_api_key,
                            primary_headers,
                        );
                        *self.provider_target.write().expect("provider target lock") = Some(target);
                    }
                }
                agent.set_model(agent_model).await;
                agent.set_thinking_level(thinking_level).await;
                if let Some(core) = &core {
                    core.update_model_facts(&primary_model);
                }
                if let Some(persistence) = persistence.filter(|_| !session_restored) {
                    persistence
                        .lock()
                        .await
                        .append_model_change(&primary_model.provider, &primary_model.id)?;
                }
                if !session_restored
                    && !(emit_cell.borrow_mut())(EngineEvent::ModelChange(
                        pa_types::session::ModelChangeEntry {
                            provider: primary_model.provider.clone(),
                            model_id: primary_model.id.clone(),
                        },
                    ))
                {
                    anyhow::bail!("Model restoration persistence was cancelled");
                }
                if let Some(primary) = primary_state
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .as_mut()
                {
                    primary.session_restored = true;
                }
                Ok(Some(format!(
                    "{}/{}",
                    primary_model.provider, primary_model.id
                )))
            }
        };
        let result = self.runtime.block_on(
            pa_core::session_engine::provider_failover::run_turn_with_provider_failover(
                &policy,
                &failover_policy,
                &candidates,
                overflow_window,
                None,
                || {
                    let mut emit = emit_cell.borrow_mut();
                    let first = first_attempt.get();
                    first_attempt.set(false);
                    let agent = agent.clone();
                    let prompt = prompt.clone();
                    let model = model.clone();
                    let pending_model_changes = pending_model_changes.clone();
                    async move {
                        // The nudge slot is attempt-scoped: a retry that was
                        // queued but never issued (abort, worker shutdown)
                        // must not leak its nudge into this turn's request.
                        if let Some(core) = self.session.lock().await.clone() {
                            core.session.clear_retry_nudge();
                        }
                        // A retry re-issues the failed turn: the failed
                        // assistant message leaves the loop context first
                        // (TS `messages.slice(0, -1)`), then `continue`.
                        if !first {
                            drop_trailing_assistant(&agent).await;
                        }
                        let mut admit_prompt = first;
                        // Automatic retries for a finished turn the user
                        // cannot act on (no text, no tool call). Without
                        // them the run ends silently: the providers stop
                        // cleanly on thinking-only replies. A subagent gets
                        // one extra attempt (see `empty_reply_retries`).
                        let mut empty_reply_retries_left = 1;
                        loop {
                            let assistant = match self
                                .run_turn_once(
                                    &agent,
                                    &prompt,
                                    admit_prompt,
                                    boundary_passed,
                                    &pending_model_changes,
                                    aborted,
                                    &mut **emit,
                                )
                                .await
                            {
                                Ok(TurnOnce::Message { assistant }) => *assistant,
                                Ok(TurnOnce::None) => {
                                    return Err(anyhow::anyhow!("No response produced."));
                                }
                                Ok(TurnOnce::Aborted) => return Ok(aborted_message(&model)),
                                Err(error) => return Err(error),
                            };
                            if is_empty_reply(&assistant) {
                                if empty_reply_retries_left > 0 {
                                    empty_reply_retries_left -= 1;
                                    // Queue the request-only nudge, drop
                                    // the empty assistant, and re-issue
                                    // the same turn without re-admitting
                                    // the prompt.
                                    if let Some(core) = self.session.lock().await.clone() {
                                        core.session.queue_retry_nudge(EMPTY_REPLY_RETRY_NUDGE);
                                    }
                                    drop_trailing_assistant(&agent).await;
                                    admit_prompt = false;
                                    continue;
                                }
                                // Still empty: never end the turn
                                // silently �� surface a visible warning, and
                                // for a subagent tell the parent so the run is
                                // re-tasked instead of idling unseen.
                                emit(EngineEvent::CustomMessage(empty_reply_warning(
                                    stalled_run_subagent,
                                )));
                                if stalled_run_subagent {
                                    let notice = self
                                        .stalled_run_notice
                                        .lock()
                                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                                        .clone();
                                    if let Some(notice) = notice {
                                        notice(EMPTY_REPLY_STALLED_PARENT_NOTICE.to_string())
                                            .await
                                            .ok();
                                    }
                                }
                                return Ok(assistant);
                            }
                            if !self
                                .threshold_yielded
                                .swap(false, std::sync::atomic::Ordering::SeqCst)
                            {
                                return Ok(assistant);
                            }
                            // Requested boundaries stay owned by run_turns; consuming one here
                            // would lose its explicit stop and goal-continuation semantics.
                            let core = self.session.lock().await.clone();
                            let pending = match core {
                                Some(core) => {
                                    core.turn_boundary.compaction_scheduled().await
                                        || core.turn_boundary.refine_pending().await
                                }
                                None => false,
                            };
                            if aborted() {
                                return Ok(aborted_message(&model));
                            }
                            if pending
                                || self
                                    .config
                                    .queued_steering_probe
                                    .as_ref()
                                    .is_some_and(|probe| probe())
                            {
                                return Ok(assistant);
                            }
                            if aborted() {
                                return Ok(aborted_message(&model));
                            }
                            // Keep the retry/failover episode alive across this artificial
                            // yield. The engine owns a multi-thread runtime; the synchronous
                            // compaction boundary may re-enter it without holding async locks.
                            let outcome = tokio::task::block_in_place(|| {
                                self.run_auto_compaction(&mut **emit)
                            });
                            self.threshold_checks_enabled.store(
                                outcome.continues_threshold_checks(),
                                std::sync::atomic::Ordering::SeqCst,
                            );
                            if outcome == crate::auto_compaction::AutoCompactionRun::Cancelled
                                || aborted()
                            {
                                return Ok(aborted_message(&model));
                            }
                            if self
                                .config
                                .queued_steering_probe
                                .as_ref()
                                .is_some_and(|probe| probe())
                            {
                                return Ok(assistant);
                            }
                            // Continue the tool run without dropping its successful assistant
                            // or inserting another copy of the admitted user prompt.
                            admit_prompt = false;
                        }
                    }
                },
                |event| {
                    let mut emit = emit_cell.borrow_mut();
                    let telemetry = telemetry.clone();
                    let semantic_edges = semantic_edges.clone();
                    async move {
                        // TS parks the id at `auto_retry_start` and clears
                        // it at the settle (`_resolveRetry`), so a
                        // body-identical retry reuses the failed
                        // attempt's id; the parked id lives until the
                        // next turn mints over it otherwise.
                        if let Some(recorder) = &semantic_edges {
                            match &event {
                                pa_core::session_engine::auto_retry::AutoRetryEvent::Start {
                                    ..
                                } => recorder.prepare_turn_retry(),
                                pa_core::session_engine::auto_retry::AutoRetryEvent::End {
                                    ..
                                } => {
                                    recorder.clear_turn_retry();
                                }
                            }
                        }
                        if let Some(telemetry) = &telemetry {
                            // One retry event in, one telemetry seam out.
                            telemetry.note_auto_retry_event(&event);
                        }
                        let engine_event = retry_event_to_engine_event(event);
                        if !emit(engine_event) {
                            anyhow::bail!("emit cancelled");
                        }
                        Ok(())
                    }
                },
                |delay| {
                    async move {
                        // Abort-aware wait: the cancel flag stops the retry sleep early (TS
                        // `_retryAbortController`).
                        let deadline = tokio::time::Instant::now() + delay;
                        loop {
                            if aborted() {
                                return false;
                            }
                            if tokio::time::Instant::now() >= deadline {
                                return true;
                            }
                            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                        }
                    }
                },
                |next: &pa_types::ai::Model| {
                    let emit_cell = &emit_cell;
                    let agent = agent.clone();
                    let persistence = persistence.clone();
                    let core = core.clone();
                    {
                        let mut primary = primary_state
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner);
                        // Capture the primary state once (TS `_backupModel`), restored when the
                        // successful backup response arrives.
                        if primary.is_none() {
                            let (api_key, headers) = self.resolve_request_key_and_headers(&model);
                            *primary = Some(FailoverPrimary {
                                model: model.clone(),
                                thinking_level: map_thinking_level(self.effective_thinking()),
                                api_key,
                                headers,
                                session_restored: false,
                            });
                        } else if let Some(primary) = primary.as_mut() {
                            primary.session_restored = false;
                        }
                    }
                    let next = next.clone();
                    async move {
                        let agent_model = json_round_trip(&next)
                            .ok_or_else(|| anyhow::anyhow!("model conversion failed"))?;
                        // Clamp the level to the switched-to model (TS
                        // `clampThinkingLevel`); the primary restores with
                        // the primary.
                        let clamped =
                            pa_ai::models::clamp_thinking_level(&next, self.effective_thinking());
                        // The stream's provider target follows the switch (the same slot
                        // `set_model` swaps).
                        {
                            // The switch's resolve-and-install serializes
                            // with `/reload`'s live-input refresh — the
                            // restore path in the sibling callback takes
                            // the same lock: a reload landing between
                            // this resolution and its slot write must not
                            // be clobbered by the switch's older pair. The
                            // turn runs on the blocking pool, so the
                            // synchronous resolution never touches an
                            // async executor worker.
                            let _reload_serialized = self
                                .reload_lock
                                .lock()
                                .unwrap_or_else(std::sync::PoisonError::into_inner);
                            // A routed episode keeps serving the route's
                            // target across the failover switch — through
                            // the fenced install (the route lock covers
                            // the clone and the slot write together).
                            if !self.install_armed_route_target() {
                                let (api_key, headers) =
                                    self.resolve_request_key_and_headers(&next);
                                let mut target =
                                    self.provider_target.write().expect("provider target lock");
                                *target = Some(ProviderTarget {
                                    service_tier: *self
                                        .service_tier
                                        .read()
                                        .expect("service tier lock"),
                                    api_key,
                                    model: next.clone(),
                                    headers,
                                });
                            }
                        }
                        agent.set_model(agent_model).await;
                        agent.set_thinking_level(map_thinking_level(clamped)).await;
                        if let Some(core) = &core {
                            core.update_model_facts(&next);
                        }
                        if let Some(persistence) = persistence {
                            let mut session = persistence.lock().await;
                            session.append_model_change(&next.provider, &next.id)?;
                        }
                        if !(emit_cell.borrow_mut())(EngineEvent::ModelChange(
                            pa_types::session::ModelChangeEntry {
                                provider: next.provider.clone(),
                                model_id: next.id.clone(),
                            },
                        )) {
                            anyhow::bail!("Model switch persistence was cancelled");
                        }
                        Ok(())
                    }
                },
                &restore_primary,
                park,
            ),
        );
        self.runtime.block_on(restore_subscription.unsubscribe());
        // A failing awaited listener (including a failed model-change
        // append) exits the retry driver before its normal restore callback.
        // Always restore the transport as well, while preserving both errors.
        let result = match result {
            Err(error) => match self.runtime.block_on(restore_primary()) {
                Ok(_) => Err(error),
                Err(cleanup) => Err(anyhow::anyhow!(
                    "{error:#}; primary restoration also failed: {cleanup:#}"
                )),
            },
            Ok(message) => Ok(message),
        };
        match result {
            Ok(message) => match message.stop_reason {
                // The failure already reached the transcript as the final
                // assistant message; the turn error still travels to
                // headless callers through the turn result.
                StopReason::Error => TurnResult::Error {
                    error: message
                        .error_message
                        .clone()
                        .filter(|error| !error.is_empty())
                        .unwrap_or_else(|| "Assistant response failed".to_string()),
                    assistant: Some(Box::new(message)),
                },
                StopReason::Aborted => TurnResult::Aborted,
                _ => TurnResult::Message(Box::new(message)),
            },
            Err(error) => TurnResult::Error {
                error: error.to_string(),
                assistant: None,
            },
        }
    }
}

/// The trailing user nudge the empty reply is retried with. Request-only:
/// the engine's transform drains it after assembling the request, so it
/// never enters the persisted history.
const EMPTY_REPLY_RETRY_NUDGE: &str = "Your previous reply was empty: no user-visible text and no tool calls. Continue the task now - answer the user or call the tools you need. Never reply with an empty message.";

/// The message a stalled subagent sends its parent. It is warm and actionable:
/// the parent re-tasks the child (or takes the work over) instead of waiting
/// for a reply that will never arrive.
const EMPTY_REPLY_STALLED_PARENT_NOTICE: &str = "I stopped without producing a reply: the model returned an empty turn (no text, no tool calls) twice in a row, so this run has ended. Nothing in my context was lost - re-task me with the next step and I will continue.";

/// True when a finished assistant message carries nothing the user can act
/// on: no non-blank text and no tool call. Thinking-only replies count as
/// empty (providers stop cleanly on them and the run would end silently).
fn is_empty_reply(message: &pa_agent::types::AssistantMessage) -> bool {
    is_empty_content(&message.content)
}

/// The content-level predicate behind [`is_empty_reply`].
fn is_empty_content(content: &[pa_agent::types::AssistantContent]) -> bool {
    !content.iter().any(|part| match part {
        pa_agent::types::AssistantContent::Text(text) => !text.text.trim().is_empty(),
        pa_agent::types::AssistantContent::ToolCall(_) => true,
        _ => false,
    })
}

/// The visible warning emitted when the retried reply is still empty. It
/// reuses the model-prompt-error row type: the client renders it as a
/// warning status line instead of ending the turn with no feedback.
fn empty_reply_warning(stalled_subagent: bool) -> serde_json::Value {
    let content = if stalled_subagent {
        "The model returned an empty reply (no text and no tool calls) after an automatic retry. This subagent run stopped and its parent was notified so it can re-task the work."
    } else {
        "The model returned an empty reply (no text and no tool calls) after an automatic retry. Nothing was lost - send a message to continue."
    };
    let message = pa_types::session::CustomMessage {
        custom_type: pa_core::prompts::model_prompts::MODEL_PROMPT_ERROR_CUSTOM_TYPE.to_string(),
        content: pa_types::ai::UserContent::Text(content.to_string()),
        display: true,
        details: None,
        timestamp: crate::util::now_ms(),
        rest: serde_json::Map::default(),
    };
    crate::session_commands::custom_message_value(&message)
}

#[cfg(test)]
mod tests {
    use pa_agent::types::AssistantContent;

    fn content(value: serde_json::Value) -> AssistantContent {
        serde_json::from_value(value).expect("assistant content")
    }

    #[test]
    fn text_and_tool_calls_are_not_empty() {
        assert!(!super::is_empty_content(&[content(
            serde_json::json!({"type": "text", "text": "hello"})
        )]));
        assert!(!super::is_empty_content(&[content(serde_json::json!({
            "type": "toolCall",
            "id": "call-1",
            "name": "bash",
            "arguments": {}
        }))]));
    }

    #[test]
    fn blank_text_and_thinking_only_are_empty() {
        assert!(super::is_empty_content(&[]));
        assert!(super::is_empty_content(&[content(
            serde_json::json!({"type": "text", "text": "   \n"})
        )]));
        assert!(super::is_empty_content(&[content(
            serde_json::json!({"type": "thinking", "thinking": "a long internal monologue"})
        )]));
        assert!(super::is_empty_content(&[
            content(serde_json::json!({"type": "thinking", "thinking": "monologue"})),
            content(serde_json::json!({"type": "text", "text": ""})),
        ]));
    }
}
