//! The empty-reply retry nudge: a request-only trailing user turn.
//!
//! The retry driver queues a nudge when a finished turn carries no
//! user-visible output; the request transform drains it into the next
//! assembled context. The nudge never enters the agent's persisted
//! history, so a retry leaves no trace and cannot loop on its own
//! injection.

use std::sync::{Arc, Mutex};

use pa_agent::abort::AbortSignal;
use pa_agent::agent_loop::TransformContextFn;
use pa_agent::types::{AgentMessage, Message, UserContent, UserMessage};

/// Wrap `inner` so the next request appends the queued nudge as the
/// trailing user turn and clears the slot.
pub fn wrap_transform(
    inner: TransformContextFn,
    slot: Arc<Mutex<Option<String>>>,
) -> TransformContextFn {
    Arc::new(move |messages: Vec<AgentMessage>, signal: AbortSignal| {
        let inner = Arc::clone(&inner);
        let slot = Arc::clone(&slot);
        Box::pin(async move {
            let mut messages = inner(messages, signal).await?;
            let nudge = slot
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take();
            if let Some(text) = nudge {
                messages.push(AgentMessage::Standard(Message::User(UserMessage {
                    content: UserContent::Text(text),
                    timestamp: now_millis(),
                })));
            }
            Ok(messages)
        })
    })
}

fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pass_through() -> TransformContextFn {
        Arc::new(|messages, _signal| Box::pin(async move { Ok(messages) }))
    }

    async fn run(transform: &TransformContextFn, messages: Vec<AgentMessage>) -> Vec<AgentMessage> {
        transform(messages, AbortSignal::never())
            .await
            .expect("transform")
    }

    fn user_message(text: &str) -> AgentMessage {
        AgentMessage::Standard(Message::User(UserMessage {
            content: UserContent::Text(text.to_string()),
            timestamp: 0,
        }))
    }

    fn user_text(message: &AgentMessage) -> String {
        match message {
            AgentMessage::Standard(Message::User(user)) => match &user.content {
                UserContent::Text(text) => text.clone(),
                _ => panic!("expected text content"),
            },
            _ => panic!("expected a standard user message"),
        }
    }

    #[tokio::test]
    async fn appends_the_queued_nudge_once() {
        let slot = Arc::new(Mutex::new(None));
        let transform = wrap_transform(pass_through(), Arc::clone(&slot));
        *slot.lock().unwrap() = Some("continue".to_string());

        let messages = run(&transform, vec![user_message("hello")]).await;
        assert_eq!(messages.len(), 2);
        assert_eq!(user_text(&messages[0]), "hello");
        assert_eq!(user_text(&messages[1]), "continue");
        assert!(slot.lock().unwrap().is_none(), "the nudge drains");

        let messages = run(&transform, vec![user_message("hello")]).await;
        assert_eq!(messages.len(), 1, "a drained slot appends nothing");
    }

    #[tokio::test]
    async fn appends_after_the_inner_transform_output() {
        let inner: TransformContextFn = Arc::new(|mut messages, _signal| {
            Box::pin(async move {
                messages.push(user_message("inner"));
                Ok(messages)
            })
        });
        let slot = Arc::new(Mutex::new(Some("nudge".to_string())));
        let transform = wrap_transform(inner, slot);

        let messages = run(&transform, vec![user_message("hello")]).await;
        let texts: Vec<String> = messages.iter().map(user_text).collect();
        assert_eq!(texts, vec!["hello", "inner", "nudge"]);
    }

    #[tokio::test]
    async fn an_empty_slot_leaves_the_messages_untouched() {
        let transform = wrap_transform(pass_through(), Arc::new(Mutex::new(None)));
        let messages = run(&transform, vec![user_message("hello")]).await;
        assert_eq!(messages.len(), 1);
    }
}
