//! Bank resolution: the repo-derived `coding-agent::<repo>` id, the
//! `mapPathToBank` overrides, and the template placeholders the JS plugin
//! documents.
//!
//! A git probe failure falls back to the directory name (never guesses a
//! different repository); an empty result means "skip this hook entirely".

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use super::config::{self, Config, ProcessEnv, DEFAULT_BANK_NAME};

/// Probe window for the two git calls.
const GIT_TIMEOUT: Duration = Duration::from_secs(2);

/// The bank id for this working directory, after the path map and the
/// `banks` override's `bank` rename.
pub(crate) async fn resolve(config: &Config, cwd: &Path) -> Option<String> {
    let project_root = if config.resolve_worktrees {
        git_main_worktree_root(cwd).await
    } else {
        None
    };
    let mut candidates: Vec<&Path> = vec![cwd];
    if let Some(root) = project_root.as_deref() {
        candidates.push(root);
    }
    if config.opt_in_only && !opted_in(config, &candidates) {
        return None;
    }
    if let Some(bank) = mapped_bank(config, &candidates) {
        return Some(bank);
    }
    if !config.dynamic_bank_id {
        return Some(
            config
                .bank_id
                .clone()
                .unwrap_or_else(|| DEFAULT_BANK_NAME.to_string()),
        );
    }
    let directory_name = basename(cwd);
    let project_name = project_root
        .as_deref()
        .map_or_else(|| directory_name.clone(), basename);
    let values = vec![
        ("gitProject", project_name),
        ("project", directory_name),
        ("harness", config.harness.clone()),
        (
            "channel",
            std::env::var("HINDSIGHT_CHANNEL_ID").unwrap_or_else(|_| "default".to_string()),
        ),
        (
            "user",
            std::env::var("HINDSIGHT_USER_ID").unwrap_or_else(|_| "anonymous".to_string()),
        ),
    ];
    Some(apply_template(&config.bank_id_template, &values))
}

/// Whether the opt-in gate approves this directory: an explicit path, or a
/// `mapPathToBank` entry, exactly like the JS `isOptedIn`.
fn opted_in(config: &Config, candidates: &[&Path]) -> bool {
    if !config.opt_in_only {
        return true;
    }
    let approved = candidates.iter().any(|candidate| {
        let candidate = normalize(&candidate.to_string_lossy());
        config.opt_in_paths.iter().any(|configured| {
            let configured = normalize(configured);
            is_within(&candidate, &configured)
        })
    });
    approved || mapped_bank(config, candidates).is_some()
}

/// The longest matching `mapPathToBank` entry, cwd first, then the main
/// worktree root.
fn mapped_bank(config: &Config, candidates: &[&Path]) -> Option<String> {
    for candidate in candidates {
        let candidate = normalize(&candidate.to_string_lossy());
        let best = config
            .map_path_to_bank
            .iter()
            .map(|(directory, bank)| (normalize(directory), bank))
            .filter(|(directory, _)| is_within(&candidate, directory))
            .max_by_key(|(directory, _)| directory.len());
        if let Some((_, bank)) = best {
            return Some(bank.clone());
        }
    }
    None
}

/// The main repository root for a worktree (or the repository root for a
/// plain checkout). `None` when git cannot answer.
async fn git_main_worktree_root(cwd: &Path) -> Option<PathBuf> {
    if let Some(common) = git_output(
        cwd,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    )
    .await
    {
        let common = PathBuf::from(common);
        let common = if common.is_absolute() {
            common
        } else {
            cwd.join(common)
        };
        if common.file_name().is_some_and(|name| name == ".git") {
            return common.parent().map(Path::to_path_buf);
        }
        return Some(common);
    }
    git_output(cwd, &["rev-parse", "--show-toplevel"])
        .await
        .map(PathBuf::from)
}

async fn git_output(cwd: &Path, args: &[&str]) -> Option<String> {
    let mut command = tokio::process::Command::new("git");
    command
        .arg("-C")
        .arg(cwd)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    crate::platform::process::set_no_window(command.as_std_mut());
    let output = tokio::time::timeout(GIT_TIMEOUT, command.output())
        .await
        .ok()?
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!text.is_empty()).then_some(text)
}

fn apply_template(template: &str, values: &[(&str, String)]) -> String {
    let mut out = String::new();
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let Some(end) = after.find('}') else {
            out.push_str(&rest[start..]);
            return out;
        };
        let name = &after[..end];
        match values.iter().find(|(key, _)| *key == name) {
            Some((_, value)) => out.push_str(value),
            None => out.push_str("unknown"),
        }
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}

fn basename(path: &Path) -> String {
    path.file_name().map_or_else(
        || "unknown".to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}

/// Compare paths the way the JS side does: separators unified, trailing
/// slashes dropped, and (on Windows) case folded.
fn normalize(path: &str) -> String {
    let expanded = expand_tilde(path);
    let mut out = expanded.replace('\\', "/");
    while out.len() > 1 && out.ends_with('/') {
        out.pop();
    }
    #[cfg(windows)]
    {
        out = out.to_ascii_lowercase();
    }
    out
}

fn expand_tilde(path: &str) -> String {
    let home = || config::home_dir(&ProcessEnv);
    if path == "~" {
        return home().map_or_else(
            || path.to_string(),
            |home| home.to_string_lossy().into_owned(),
        );
    }
    let rest = path.strip_prefix("~/").or_else(|| path.strip_prefix("~\\"));
    match (rest, home()) {
        (Some(rest), Some(home)) => home.join(rest).to_string_lossy().into_owned(),
        _ => path.to_string(),
    }
}

fn is_within(directory: &str, configured: &str) -> bool {
    if directory == configured {
        return true;
    }
    if configured == "/" {
        return directory.starts_with('/');
    }
    directory
        .strip_prefix(configured)
        .is_some_and(|rest| rest.starts_with('/'))
}
