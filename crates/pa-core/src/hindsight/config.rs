//! The Hindsight settings the bridge honors: the subset of
//! `~/.hindsight/coding-agent.json` (or `HINDSIGHT_CONFIG`) that affects the
//! two hooks, plus the documented environment overrides.
//!
//! Unknown keys are ignored: the file is shared with the JavaScript plugin,
//! which owns the rest of the surface. A missing or malformed file behaves
//! like an absent one (fail-open).

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::Deserialize;

/// The server the bridge talks to when the config names none.
pub(crate) const DEFAULT_API_URL: &str = "http://127.0.0.1:8888";
/// The per-repo bank template (`coding-agent::<repo>`); the JS default.
pub(crate) const DEFAULT_BANK_TEMPLATE: &str = "coding-agent::{gitProject}";
/// The bank used when `bankId` is fixed and the template is disabled.
pub(crate) const DEFAULT_BANK_NAME: &str = "coding";
/// Bounded reflect window for the automatic hook (the JS `HOOK_REFLECT_CAP_MS`).
pub(crate) const MAX_HOOK_REFLECT_MS: u64 = 20_000;
/// The reflection budget's default when the file names none.
pub(crate) const DEFAULT_REFLECT_TIMEOUT_MS: u64 = 120_000;
/// Roster refresh cadence default.
pub(crate) const DEFAULT_PAGE_REFRESH_EVERY_TURNS: u64 = 10;
/// The fallback tiers' shared budget (JS `HOOK_FALLBACK_BUDGET_MS`).
pub(crate) const FALLBACK_BUDGET_MS: u64 = 7_000;

/// The daemon-mode port default.
const DEFAULT_DAEMON_PORT: u16 = 9077;

/// The file's raw shape: every field optional, so a partial (or JS-owned)
/// file stays loadable.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct RawConfig {
    api_url: Option<String>,
    api_token: Option<String>,
    server_mode: Option<String>,
    api_port: Option<u16>,
    bank_id: Option<String>,
    dynamic_bank_id: Option<bool>,
    bank_id_template: Option<String>,
    map_path_to_bank: Option<BTreeMap<String, String>>,
    banks: Option<BTreeMap<String, RawBankOverride>>,
    disabled: Option<bool>,
    auto_reflect: Option<bool>,
    reflect_timeout_ms: Option<u64>,
    retain_sessions: Option<bool>,
    resolve_worktrees: Option<bool>,
    page_refresh_every_turns: Option<u64>,
    harness: Option<String>,
    opt_in_only: Option<bool>,
    opt_in_paths: Option<Vec<String>>,
}

/// One `banks.<id>` section. Only the fields the bridge acts on are read;
/// the JS plugin may carry more.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct RawBankOverride {
    /// A final bank id overriding the section's key.
    pub bank: Option<String>,
    pub disabled: Option<bool>,
    pub auto_reflect: Option<bool>,
    pub retain_sessions: Option<bool>,
    pub reflect_timeout_ms: Option<u64>,
    pub page_refresh_every_turns: Option<u64>,
}

/// Environment access, injected so tests never mutate the process env.
pub(crate) trait Env {
    fn var(&self, key: &str) -> Option<String>;
}

/// The real process environment.
pub(crate) struct ProcessEnv;

impl Env for ProcessEnv {
    fn var(&self, key: &str) -> Option<String> {
        std::env::var(key).ok()
    }
}

/// The resolved settings one hook invocation needs.
#[derive(Debug, Clone)]
#[allow(clippy::struct_excessive_bools)] // mirrors the settings file's independent switches
pub(crate) struct Config {
    pub api_url: String,
    pub api_token: Option<String>,
    pub bank_id: Option<String>,
    pub dynamic_bank_id: bool,
    pub bank_id_template: String,
    pub map_path_to_bank: BTreeMap<String, String>,
    pub banks: BTreeMap<String, RawBankOverride>,
    pub disabled: bool,
    pub auto_reflect: bool,
    pub reflect_timeout_ms: u64,
    pub retain_sessions: bool,
    pub resolve_worktrees: bool,
    pub page_refresh_every_turns: u64,
    pub opt_in_only: bool,
    pub opt_in_paths: Vec<String>,
    pub harness: String,
}

impl Config {
    /// Load the process configuration: `HINDSIGHT_CONFIG` (or the default
    /// file) plus the environment overrides.
    pub(crate) fn load() -> Config {
        Self::load_with(&ProcessEnv)
    }

    pub(crate) fn load_with(env: &dyn Env) -> Config {
        let raw = config_path(env)
            .map(|path| read_raw(&path))
            .unwrap_or_default();
        Self::resolve(raw, env)
    }

    fn resolve(raw: RawConfig, env: &dyn Env) -> Config {
        let server_mode = env
            .var("HINDSIGHT_SERVER_MODE")
            .or_else(|| raw.server_mode.clone())
            .unwrap_or_else(|| "self-hosted".to_string());
        let api_port = number(env, "HINDSIGHT_API_PORT")
            .and_then(|value| u16::try_from(value).ok())
            .or(raw.api_port)
            .unwrap_or(DEFAULT_DAEMON_PORT);
        let api_url = env
            .var("HINDSIGHT_API_URL")
            .or_else(|| raw.api_url.clone())
            .unwrap_or_else(|| {
                if server_mode == "daemon" {
                    format!("http://127.0.0.1:{api_port}")
                } else {
                    DEFAULT_API_URL.to_string()
                }
            });
        let bank_id = env
            .var("HINDSIGHT_BANK_ID")
            .filter(|value| !value.trim().is_empty())
            .or_else(|| raw.bank_id.clone());
        let dynamic_bank_id = boolean(env, "HINDSIGHT_DYNAMIC_BANK_ID")
            .or(raw.dynamic_bank_id)
            .unwrap_or(bank_id.is_none());
        let hooks_disabled = env
            .var("HINDSIGHT_DISABLE_HOOKS")
            .is_some_and(|value| !value.is_empty());
        Config {
            api_url: api_url.trim_end_matches('/').to_string(),
            api_token: env
                .var("HINDSIGHT_API_TOKEN")
                .filter(|value| !value.trim().is_empty())
                .or_else(|| {
                    raw.api_token
                        .clone()
                        .filter(|value| !value.trim().is_empty())
                }),
            bank_id,
            dynamic_bank_id,
            bank_id_template: env
                .var("HINDSIGHT_BANK_ID_TEMPLATE")
                .or_else(|| raw.bank_id_template.clone())
                .unwrap_or_else(|| DEFAULT_BANK_TEMPLATE.to_string()),
            map_path_to_bank: raw.map_path_to_bank.clone().unwrap_or_default(),
            banks: raw.banks.clone().unwrap_or_default(),
            disabled: hooks_disabled
                || boolean(env, "HINDSIGHT_DISABLED")
                    .or(raw.disabled)
                    .unwrap_or(false),
            auto_reflect: boolean(env, "HINDSIGHT_AUTO_REFLECT")
                .or(raw.auto_reflect)
                .unwrap_or(true),
            reflect_timeout_ms: number(env, "HINDSIGHT_REFLECT_TIMEOUT_MS")
                .or(raw.reflect_timeout_ms)
                .unwrap_or(DEFAULT_REFLECT_TIMEOUT_MS),
            retain_sessions: boolean(env, "HINDSIGHT_RETAIN_SESSIONS")
                .or(raw.retain_sessions)
                .unwrap_or(true),
            resolve_worktrees: boolean(env, "HINDSIGHT_RESOLVE_WORKTREES")
                .or(raw.resolve_worktrees)
                .unwrap_or(true),
            page_refresh_every_turns: number(env, "HINDSIGHT_PAGE_REFRESH_EVERY_TURNS")
                .or(raw.page_refresh_every_turns)
                .unwrap_or(DEFAULT_PAGE_REFRESH_EVERY_TURNS),
            opt_in_only: raw.opt_in_only.unwrap_or(false),
            opt_in_paths: raw
                .opt_in_paths
                .unwrap_or_default()
                .into_iter()
                .filter(|path| !path.trim().is_empty())
                .collect(),
            harness: env
                .var("HINDSIGHT_HARNESS")
                .or_else(|| raw.harness.clone())
                .unwrap_or_else(|| "prime-agent".to_string()),
        }
    }

    /// Apply one `banks.<resolved id>` override; returns the final bank id
    /// (a section may rename it through `bank`).
    pub(crate) fn for_bank(&self, bank: &str) -> (Config, String) {
        let Some(section) = self.banks.get(bank) else {
            return (self.clone(), bank.to_string());
        };
        let mut config = self.clone();
        if let Some(value) = section.disabled {
            config.disabled = value;
        }
        if let Some(value) = section.auto_reflect {
            config.auto_reflect = value;
        }
        if let Some(value) = section.retain_sessions {
            config.retain_sessions = value;
        }
        if let Some(value) = section.reflect_timeout_ms {
            config.reflect_timeout_ms = value;
        }
        if let Some(value) = section.page_refresh_every_turns {
            config.page_refresh_every_turns = value;
        }
        let id = section.bank.clone().unwrap_or_else(|| bank.to_string());
        (config, id)
    }
}

/// The config file the JS plugin reads, unless overridden.
fn config_path(env: &dyn Env) -> Option<PathBuf> {
    if let Some(path) = env
        .var("HINDSIGHT_CONFIG")
        .filter(|value| !value.is_empty())
    {
        return Some(PathBuf::from(path));
    }
    home_dir(env).map(|home| home.join(".hindsight").join("coding-agent.json"))
}

/// The user's home directory: `USERPROFILE` first on Windows, then `HOME`.
pub(crate) fn home_dir(env: &dyn Env) -> Option<PathBuf> {
    env.var("USERPROFILE")
        .filter(|value| !value.is_empty())
        .or_else(|| env.var("HOME").filter(|value| !value.is_empty()))
        .map(PathBuf::from)
}

fn read_raw(path: &std::path::Path) -> RawConfig {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn boolean(env: &dyn Env, key: &str) -> Option<bool> {
    env.var(key).map(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        )
    })
}

fn number(env: &dyn Env, key: &str) -> Option<u64> {
    env.var(key).and_then(|value| value.trim().parse().ok())
}
