//! The terminal window title: `prime-agent · <session> · <state>`.
//!
//! Written through OSC 0 (icon name and title) on change only. Unlike the
//! OSC 7501 program-status reports, the title needs no feature detection:
//! every terminal that renders a title understands OSC 0. The caller gates
//! on a real terminal, so a piped run writes nothing.

use std::io::Write;

use crate::program_status::Status;

const APP: &str = "prime-agent";

#[derive(Debug, Default)]
struct Reporter {
    /// The sanitized session label (`None` until a session reports one).
    session: Option<String>,
    status: Option<Status>,
    /// The title the terminal already has.
    written: Option<String>,
}

static REPORTER: std::sync::Mutex<Reporter> = std::sync::Mutex::new(Reporter {
    session: None,
    status: None,
    written: None,
});

fn reporter() -> std::sync::MutexGuard<'static, Reporter> {
    REPORTER
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Record the session label and the surface's status, writing the title when
/// the composed string changed.
pub(crate) fn report(status: Status, session: &str) {
    let mut reporter = reporter();
    reporter.session = Some(sanitize(session));
    reporter.status = Some(status);
    reporter.flush();
}

impl Reporter {
    /// The title owed to the terminal, if any. An unnamed session still gets
    /// the app name and the state, so a fresh chat's title is never blank.
    fn owed(&self) -> Option<String> {
        let state = match self.status? {
            Status::Working => "working",
            Status::Done => "done",
            Status::Error => "error",
            Status::Idle => "idle",
        };
        let session = self.session.as_deref().unwrap_or_default();
        let title = if session.is_empty() {
            format!("{APP} · {state}")
        } else {
            format!("{APP} · {session} · {state}")
        };
        (self.written.as_deref() != Some(title.as_str())).then_some(title)
    }

    fn flush(&mut self) {
        let Some(title) = self.owed() else {
            return;
        };
        let mut out = std::io::stdout();
        if out
            .write_all(format!("\x1b]0;{title}\x07").as_bytes())
            .and_then(|()| out.flush())
            .is_ok()
        {
            self.written = Some(title);
        }
    }
}

/// Collapse control characters and whitespace runs: a session name is user
/// text and must not smuggle a second escape sequence into the title.
fn sanitize(text: &str) -> String {
    text.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reporter(session: &str, status: Status) -> Reporter {
        Reporter {
            session: Some(sanitize(session)),
            status: Some(status),
            written: None,
        }
    }

    #[test]
    fn composes_the_app_session_and_state() {
        assert_eq!(
            reporter("my session", Status::Working).owed().as_deref(),
            Some("prime-agent · my session · working")
        );
        assert_eq!(
            reporter("my session", Status::Done).owed().as_deref(),
            Some("prime-agent · my session · done")
        );
    }

    #[test]
    fn an_unchanged_title_is_not_owed() {
        let mut reporter = reporter("chat", Status::Working);
        reporter.written = Some("prime-agent · chat · working".to_string());
        assert_eq!(reporter.owed(), None);
        reporter.status = Some(Status::Done);
        assert_eq!(
            reporter.owed().as_deref(),
            Some("prime-agent · chat · done"),
            "a state change rewrites the title"
        );
    }

    #[test]
    fn an_empty_session_falls_back_to_the_app_name() {
        assert_eq!(
            reporter("   ", Status::Idle).owed().as_deref(),
            Some("prime-agent · idle")
        );
    }

    #[test]
    fn control_characters_and_whitespace_runs_are_sanitized() {
        assert_eq!(sanitize("a\u{1b}]0;evil\u{7}  b\tc"), "a ]0;evil b c");
    }
}
