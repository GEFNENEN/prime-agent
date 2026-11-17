//! The hidden `prime-agent hindsight-hook prompt|stop` entry.
//!
//! The session's `turnHook` / `stopHook` settings spawn this command, so it
//! owns the process contract: read all of stdin, print only the hook JSON
//! on stdout, and exit 0 on every runtime failure (fail-open). The command
//! is intentionally absent from the public command registry and help.

use std::io::{Read, Write};

use pa_core::hindsight::{self, HookKind};

pub(crate) fn run(args: &[String]) -> i32 {
    let kind = match args.first().map(String::as_str) {
        Some("prompt") => HookKind::Prompt,
        Some("stop") => HookKind::Stop,
        _ => return 2,
    };
    let mut stdin = String::new();
    if std::io::stdin().read_to_string(&mut stdin).is_err() {
        return 0;
    }
    let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    else {
        return 0;
    };
    if let Some(output) = runtime.block_on(hindsight::handle(kind, &stdin)) {
        let mut stdout = std::io::stdout().lock();
        if stdout.write_all(output.as_bytes()).is_ok() {
            let _ = stdout.flush();
        }
    }
    0
}
