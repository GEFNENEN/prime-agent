//! The optional terminal bell when a turn settles.
//!
//! `terminal.bellOnTurnEnd` (default off): one BEL after a turn ends with
//! `Done` or `Error`. An aborted turn settles `Idle`, and a repeated status
//! report never rings twice. The caller gates on a real terminal, so a piped
//! run writes nothing.

use std::io::Write;

use crate::program_status::Status;

#[derive(Debug, Default)]
struct Reporter {
    last: Option<Status>,
}

static REPORTER: std::sync::Mutex<Reporter> = std::sync::Mutex::new(Reporter { last: None });

fn reporter() -> std::sync::MutexGuard<'static, Reporter> {
    REPORTER
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Record the surface's status and ring when it newly settles on `Done` or
/// `Error` while the setting is enabled.
pub(crate) fn report(status: Status, enabled: bool) {
    let ring = reporter().observe(status, enabled);
    if ring {
        let mut out = std::io::stdout();
        let _ = out.write_all(b"\x07").and_then(|()| out.flush());
    }
}

impl Reporter {
    fn observe(&mut self, status: Status, enabled: bool) -> bool {
        let previous = self.last.replace(status);
        enabled && previous != Some(status) && matches!(status, Status::Done | Status::Error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rings_once_when_a_turn_settles_done_or_error() {
        let mut reporter = Reporter::default();
        assert!(!reporter.observe(Status::Idle, true), "startup is silent");
        assert!(!reporter.observe(Status::Working, true));
        assert!(reporter.observe(Status::Done, true));
        assert!(
            !reporter.observe(Status::Done, true),
            "a repeated report does not ring again"
        );
        assert!(!reporter.observe(Status::Working, true));
        assert!(reporter.observe(Status::Error, true));
    }

    #[test]
    fn a_cancelled_turn_and_a_disabled_setting_stay_silent() {
        let mut reporter = Reporter::default();
        assert!(!reporter.observe(Status::Working, true));
        assert!(
            !reporter.observe(Status::Idle, true),
            "an aborted turn settles idle"
        );
        assert!(!reporter.observe(Status::Done, false));
        assert!(!reporter.observe(Status::Error, false));
    }
}
