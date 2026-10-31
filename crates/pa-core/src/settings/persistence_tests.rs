use std::sync::Arc;

use serde_json::{json, Value};

use super::{
    InMemorySettingsStorage, ModelAudience, SettingsManager, SettingsScope, SettingsStorage,
};

fn fixture(content: &str) -> (Arc<InMemorySettingsStorage>, SettingsManager) {
    let storage = Arc::new(InMemorySettingsStorage::default());
    storage
        .with_lock(SettingsScope::Global, &mut |_| Some(content.to_string()))
        .unwrap();
    let manager = SettingsManager::from_storage(storage.clone());
    (storage, manager)
}

fn document(storage: &dyn SettingsStorage) -> Value {
    serde_json::from_str(&storage.read(SettingsScope::Global).unwrap().unwrap()).unwrap()
}

#[test]
fn project_settings_cannot_select_executable_configuration() {
    let (storage, mut manager) = fixture(
        r#"{"theme":"global","shellPath":"/trusted/shell","shellCommandPrefix":"trusted", "npmCommand":["trusted-npm"],"packages":["trusted-package"],"mcpServers":{"trusted":{"command":"trusted"}},"mcpCatalogSources":["trusted.json"]}"#,
    );
    let project = json!({
        "theme": "project",
        "shellPath": "/attacker/shell",
        "shellCommandPrefix": "attacker",
        "npmCommand": ["attacker-npm"],
        "packages": ["npm:attacker"],
        "mcpServers": {"attacker": {"command": "attacker"}},
        "mcpCatalogSources": ["attacker.json"]
    });
    storage
        .with_lock(SettingsScope::Project, &mut |_| Some(project.to_string()))
        .unwrap();
    manager.reload().unwrap();
    let reopened = manager.reopen();
    for effective in [manager.settings(), reopened.settings()] {
        assert_eq!(effective.theme.as_deref(), Some("project"));
        assert_eq!(effective.shell_path.as_deref(), Some("/trusted/shell"));
        assert_eq!(effective.shell_command_prefix.as_deref(), Some("trusted"));
        assert_eq!(effective.npm_command, Some(vec!["trusted-npm".into()]));
        assert_eq!(effective.packages, Some(vec![json!("trusted-package")]));
        assert!(effective
            .mcp_servers
            .as_ref()
            .unwrap()
            .contains_key("trusted"));
        assert!(!effective
            .mcp_servers
            .as_ref()
            .unwrap()
            .contains_key("attacker"));
        assert_eq!(
            effective.mcp_catalog_sources,
            Some(vec!["trusted.json".into()])
        );
    }
    let project_settings = manager.project_settings();
    assert_eq!(project_settings.shell_path, None);
    assert_eq!(project_settings.shell_command_prefix, None);
    assert_eq!(project_settings.npm_command, None);
    assert_eq!(project_settings.packages, None);
    assert_eq!(project_settings.mcp_servers, None);
    assert_eq!(project_settings.mcp_catalog_sources, None);
    assert_eq!(
        serde_json::from_str::<Value>(&storage.read(SettingsScope::Project).unwrap().unwrap())
            .unwrap(),
        project
    );
}

#[test]
fn project_packages_stay_inert_after_a_project_write() {
    let (storage, mut manager) = fixture("{}");
    manager.set_project_packages(vec![json!("npm:attacker")]);
    assert_eq!(manager.settings().packages, None);
    assert_eq!(manager.project_settings().packages, None);
    assert_eq!(
        serde_json::from_str::<Value>(&storage.read(SettingsScope::Project).unwrap().unwrap())
            .unwrap(),
        json!({"packages":["npm:attacker"]})
    );
}

#[test]
fn model_switch_preserves_a_document_that_failed_to_load() {
    for content in ["{\"theme\":\"light\",}", "\u{feff}{\"theme\":\"light\"}"] {
        let (storage, mut manager) = fixture(content);
        manager
            .set_default_model_and_provider("openai", "model")
            .unwrap();
        assert_eq!(
            storage.read(SettingsScope::Global).unwrap().as_deref(),
            Some(content)
        );
        assert!(manager
            .errors()
            .iter()
            .any(|error| error.message.contains("not saved")));
        assert_eq!(manager.get_default_model(), Some("model"));
    }
}

#[test]
fn chat_detail_save_changes_only_the_requested_setting() {
    let (storage, mut manager) = fixture(r#"{"theme":"light","pluginSetting":{"keep":true}}"#);
    manager.set_chat_detail("details").unwrap();
    assert_eq!(
        document(storage.as_ref()),
        json!({
            "theme":"light", "pluginSetting":{"keep":true}, "chatDetail":"details"
        })
    );
}

/// `/speed on` is a preference, not a per-session probe: the write lands in the
/// global document so the footer row is back on the next launch. It must also
/// survive a reload (the read path `/speed` was missing before this existed).
#[test]
fn speed_display_persists_and_reads_back() {
    let (storage, mut manager) = fixture(r#"{"theme":"light","pluginSetting":{"keep":true}}"#);
    assert!(!manager.get_speed_display(), "unset reads as off");

    manager.set_speed_display(true).unwrap();
    assert_eq!(
        document(storage.as_ref()),
        json!({
            "theme":"light", "pluginSetting":{"keep":true}, "speedDisplay":true
        })
    );
    assert!(manager.get_speed_display());

    // A fresh manager over the same store sees the persisted value: this is
    // what makes the setting survive a restart rather than resetting at
    // construction.
    let reloaded = SettingsManager::from_storage(storage.clone());
    assert!(reloaded.get_speed_display(), "reload keeps the row on");

    manager.set_speed_display(false).unwrap();
    assert!(!SettingsManager::from_storage(storage).get_speed_display());
}

#[test]
fn stale_managers_preserve_other_writers_and_explicit_same_value_choices() {
    let (storage, mut first) =
        fixture(r#"{"theme":"light","telemetry":{"enabled":false,"futureSetting":"keep"}}"#);
    let mut second = SettingsManager::from_storage(storage.clone());
    second.set_theme("dark".into()).unwrap();
    second.set_telemetry_notice_shown(true).unwrap();
    first.set_theme("light".into()).unwrap();
    first.set_telemetry_enabled(true).unwrap();
    second.set_quiet_startup(true).unwrap();
    assert_eq!(
        document(storage.as_ref()),
        json!({
            "theme":"light", "quietStartup":true,
            "telemetry":{"enabled":true,"noticeShown":true,"futureSetting":"keep"}
        })
    );
}

#[test]
fn saves_preserve_a_document_corrupted_after_load() {
    let (storage, mut manager) = fixture(r#"{"theme":"light"}"#);
    for content in ["{bad", "[]", "null"] {
        storage
            .with_lock(SettingsScope::Global, &mut |_| Some(content.into()))
            .unwrap();
        assert!(manager.set_theme("dark".into()).is_err());
        assert_eq!(
            storage.read(SettingsScope::Global).unwrap().as_deref(),
            Some(content)
        );
    }
    storage
        .with_lock(SettingsScope::Global, &mut |_| {
            Some(r#"{"external":"keep"}"#.into())
        })
        .unwrap();
    manager.set_quiet_startup(true).unwrap();
    assert_eq!(
        document(storage.as_ref()),
        json!({
            "external":"keep", "theme":"dark", "quietStartup":true
        })
    );
}

#[test]
fn resource_save_preserves_a_document_corrupted_after_load() {
    let (storage, mut manager) = fixture("{}");
    storage
        .with_lock(SettingsScope::Global, &mut |_| Some("{bad".into()))
        .unwrap();
    manager.set_global_resource_array("skills", vec!["new".into()]);
    assert_eq!(
        storage.read(SettingsScope::Global).unwrap().as_deref(),
        Some("{bad")
    );
    assert!(manager
        .errors()
        .iter()
        .any(|error| error.message.contains("not saved")));
}

#[test]
fn clearing_a_setting_removes_only_that_key() {
    let (storage, mut manager) = fixture(r#"{"enabledModels":["old"],"theme":"light"}"#);
    manager.set_enabled_models(None).unwrap();
    assert_eq!(document(storage.as_ref()), json!({"theme":"light"}));
}

#[test]
fn resource_save_survives_an_unrelated_later_save() {
    let (storage, mut manager) = fixture("{}");
    manager.set_packages(vec![json!("first")]);
    let mut other = SettingsManager::from_storage(storage.clone());
    other.set_packages(vec![json!("second")]);
    manager.set_theme("dark".into()).unwrap();
    assert_eq!(
        document(storage.as_ref()),
        json!({"packages":["second"],"theme":"dark"})
    );
}

#[test]
fn harness_refinements_persist_without_clobbering_other_settings() {
    let (storage, mut manager) =
        fixture(r#"{"theme":"light","autoRefine":{"turnInterval":3},"future":{"keep":true}}"#);
    assert!(manager.get_harness_refinements());
    manager.set_harness_refinements(false).unwrap();
    assert!(!manager.reopen().get_harness_refinements());
    assert_eq!(
        document(storage.as_ref()),
        json!({
            "theme":"light", "autoRefine":{"turnInterval":3}, "future":{"keep":true}, "harnessRefinements":false
        })
    );
    manager.set_harness_refinements(true).unwrap();
    assert!(manager.reopen().get_harness_refinements());
}

#[test]
fn harness_refinements_ignore_project_overrides_and_resolve_global_wrong_types() {
    let dir = tempfile::tempdir().unwrap();
    let agent_dir = dir.path().join("agent");
    let project_dir = dir.path().join(".prime/agent");
    std::fs::create_dir_all(&agent_dir).unwrap();
    std::fs::create_dir_all(&project_dir).unwrap();
    std::fs::write(
        agent_dir.join("settings.json"),
        r#"{"harnessRefinements":false}"#,
    )
    .unwrap();
    std::fs::write(
        project_dir.join("settings.json"),
        r#"{"harnessRefinements":true}"#,
    )
    .unwrap();
    let mut manager = SettingsManager::create(dir.path(), &agent_dir);
    assert!(!manager.get_harness_refinements());
    manager.set_harness_refinements(true).unwrap();
    std::fs::write(
        project_dir.join("settings.json"),
        r#"{"harnessRefinements":false}"#,
    )
    .unwrap();
    assert!(manager.reopen().get_harness_refinements());
    std::fs::write(
        agent_dir.join("settings.json"),
        r#"{"harnessRefinements":"false"}"#,
    )
    .unwrap();
    assert!(manager.reopen().get_harness_refinements());
}

#[test]
fn compaction_percentage_preserves_siblings_and_survives_reload() {
    let (storage, mut manager) =
        fixture(r#"{"compaction":{"enabled":false,"reserveTokens":1234,"future":"keep"}}"#);
    assert_eq!(manager.get_compaction_threshold_percent(), 95.0);
    for percent in [1.0, 62.5, 95.0, 100.0] {
        manager.set_compaction_threshold_percent(percent).unwrap();
        manager.reload().unwrap();
        assert_eq!(manager.get_compaction_threshold_percent(), percent);
        assert_eq!(
            document(storage.as_ref()),
            json!({"compaction":{
                "enabled":false,"reserveTokens":1234,"future":"keep","thresholdPercent":percent
            }})
        );
    }
}

#[test]
fn invalid_compaction_percentage_never_mutates_or_saves() {
    let (storage, mut manager) = fixture(r#"{"compaction":{"thresholdPercent":80.0}}"#);
    for percent in [
        0.0,
        -1.0,
        100.01,
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
    ] {
        assert!(manager.set_compaction_threshold_percent(percent).is_err());
        assert_eq!(manager.get_compaction_threshold_percent(), 80.0);
        assert_eq!(
            document(storage.as_ref()),
            json!({"compaction":{"thresholdPercent":80.0}})
        );
    }
}

#[test]
fn invalid_loaded_percentage_keeps_other_compaction_settings() {
    for invalid in [
        json!(null),
        json!("80"),
        json!(false),
        json!(-1),
        json!(0),
        json!(101),
        json!({}),
        json!([]),
    ] {
        let settings = super::load::from_value_lenient(&json!({"compaction":{
            "enabled":false,"reserveTokens":1234,"thresholdPercent":invalid
        }}));
        let manager = SettingsManager::in_memory(&settings);
        assert_eq!(manager.get_compaction_threshold_percent(), 95.0);
        assert!(!manager.get_compaction_enabled());
        assert_eq!(
            manager
                .settings()
                .compaction
                .as_ref()
                .unwrap()
                .reserve_tokens,
            Some(1234)
        );
    }
}

#[test]
fn project_percentage_overrides_global_and_stale_writes_preserve_external_settings() {
    let (storage, mut first) = fixture(r#"{"compaction":{"thresholdPercent":80.0}}"#);
    let mut second = SettingsManager::from_storage(storage.clone());
    second.set_compaction_enabled(false).unwrap();
    first.set_compaction_threshold_percent(70.0).unwrap();
    assert_eq!(
        document(storage.as_ref()),
        json!({"compaction":{"enabled":false,"thresholdPercent":70.0}})
    );
    storage
        .with_lock(SettingsScope::Project, &mut |_| {
            Some(r#"{"compaction":{"thresholdPercent":60.5}}"#.into())
        })
        .unwrap();
    first.reload().unwrap();
    assert_eq!(first.get_compaction_threshold_percent(), 60.5);
    first.set_compaction_threshold_percent(90.0).unwrap();
    first.reload().unwrap();
    assert_eq!(first.get_compaction_threshold_percent(), 60.5);
}

/// `visibleModels` and `thinkingLevels` read the MERGED scopes: a project file
/// overrides the box-level value (unlike `allowedModels`, which stays global).
#[test]
fn audience_limits_are_project_overridable() {
    let global = r#"{
        "visibleModels": { "enabled": true, "human": ["yes/*"], "agent": ["yes/a"] },
        "thinkingLevels": { "enabled": true, "human": ["high"], "agent": ["high"] }
    }"#;
    let (storage, manager) = fixture(global);
    assert_eq!(
        manager.get_visible_models(ModelAudience::Human),
        Some(vec!["yes/*".to_string()])
    );
    // `agent` was named, so it does not inherit.
    assert_eq!(
        manager.get_visible_models(ModelAudience::Agent),
        Some(vec!["yes/a".to_string()])
    );

    storage
        .with_lock(SettingsScope::Project, &mut |_| {
            Some(r#"{ "visibleModels": { "enabled": true, "human": ["yes/b"] } }"#.to_string())
        })
        .unwrap();
    let narrowed = SettingsManager::from_storage(storage);
    assert_eq!(
        narrowed.get_visible_models(ModelAudience::Human),
        Some(vec!["yes/b".to_string()]),
        "the project overrides the box value"
    );
    // `thinkingLevels` was not overridden, so the box-level bound survives.
    assert_eq!(
        narrowed.get_thinking_levels(ModelAudience::Human),
        Some(vec![pa_types::ai::ModelThinkingLevel::High])
    );
}

/// An enabled limit with an empty list hides everything (fail-closed), and an
/// absent or disabled limit is `None` (unrestricted).
#[test]
fn an_empty_audience_list_hides_everything() {
    let (_, manager) =
        fixture(r#"{ "visibleModels": { "enabled": true, "human": [], "agent": [] } }"#);
    assert_eq!(
        manager.get_visible_models(ModelAudience::Human),
        Some(Vec::new()),
        "a configured empty list is a limit that hides everything, never no limit"
    );
    assert_eq!(
        manager.get_thinking_levels(ModelAudience::Human),
        None,
        "an absent thinkingLevels stays unrestricted"
    );

    let (_, off) = fixture(r#"{ "visibleModels": { "enabled": false, "human": [] } }"#);
    assert_eq!(off.get_visible_models(ModelAudience::Human), None);
}

/// The settings ladder bound steers a request onto the allowed levels, reports
/// the unmeetable case instead of failing the turn, and treats an empty list as
/// "no reasoning".
#[test]
fn the_thinking_levels_bound_steers_and_reports() {
    let reasoning: pa_types::ai::Model = serde_json::from_value(json!({
        "id": "m", "name": "m", "api": "openai-completions", "provider": "yes",
        "baseUrl": "https://example.com", "reasoning": true, "input": ["text"],
        "cost": { "input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0 },
        "contextWindow": 1000, "maxTokens": 100
    }))
    .unwrap();
    let plain: pa_types::ai::Model = serde_json::from_value(json!({
        "id": "plain", "name": "plain", "api": "openai-completions", "provider": "yes",
        "baseUrl": "https://example.com", "reasoning": false, "input": ["text"],
        "cost": { "input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0 },
        "contextWindow": 1000, "maxTokens": 100
    }))
    .unwrap();

    let (_, manager) =
        fixture(r#"{ "thinkingLevels": { "enabled": true, "human": ["high"], "agent": [] } }"#);
    // A weaker request is raised onto the only allowed level.
    assert_eq!(
        manager.clamp_thinking_level(
            ModelAudience::Human,
            &reasoning,
            pa_types::ai::ModelThinkingLevel::Low
        ),
        (pa_types::ai::ModelThinkingLevel::High, None)
    );
    // The picker offers the intersection, not the whole ladder.
    assert_eq!(
        manager.pickable_thinking_levels(ModelAudience::Human, &reasoning),
        vec![pa_types::ai::ModelThinkingLevel::High]
    );
    // An empty agent list means no reasoning at all.
    assert_eq!(
        manager
            .clamp_thinking_level(
                ModelAudience::Agent,
                &reasoning,
                pa_types::ai::ModelThinkingLevel::High
            )
            .0,
        pa_types::ai::ModelThinkingLevel::Off
    );
    // A model that cannot meet the bound runs on its own clamp and reports it.
    let (level, warning) = manager.clamp_thinking_level(
        ModelAudience::Human,
        &plain,
        pa_types::ai::ModelThinkingLevel::High,
    );
    assert_eq!(level, pa_types::ai::ModelThinkingLevel::Off);
    let warning = warning.expect("the unmeetable bound is reported");
    assert!(warning.contains("thinkingLevels"), "{warning}");
    assert!(warning.contains("supports none"), "{warning}");
}
