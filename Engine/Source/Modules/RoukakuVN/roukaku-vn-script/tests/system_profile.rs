use roukaku_vn_script::{
    ReadingMode, RoukakuSource, SaveCompletionPolicy, SystemPageKind, SystemStoryManifest,
    SystemStoryValidationStatus, SystemUiProfilePolicy, VnSystemUiProfileManifest,
    compile_roukaku_project,
};
use std::collections::BTreeSet;

const SYSTEM_STORY: &str = r#"
story main #@id story.main
state prologue #@id state.prologue
  scene room #@id scene.room
    text key:line.start #@id line.start

story system #@id story.system
state title #@id state.system.title
  scene title #@id scene.system.title
    system_page kind:title policy:roukaku.policy.standard #@id page.title
state save #@id state.system.save
  scene save #@id scene.system.save
    system_page kind:save policy:roukaku.policy.standard #@id page.save
state load #@id state.system.load
  scene load #@id scene.system.load
    system_page kind:load policy:roukaku.policy.standard #@id page.load
state config #@id state.system.config
  scene config #@id scene.system.config
    system_page kind:config policy:roukaku.policy.standard #@id page.config
state gallery #@id state.system.gallery
  scene gallery #@id scene.system.gallery
    system_page kind:gallery policy:roukaku.policy.standard #@id page.gallery
state replay #@id state.system.replay
  scene replay #@id scene.system.replay
    system_page kind:replay policy:roukaku.policy.standard #@id page.replay
state voice_replay #@id state.system.voice_replay
  scene voice_replay #@id scene.system.voice_replay
    system_page kind:voice_replay policy:roukaku.policy.standard #@id page.voice_replay
state route_chart #@id state.system.route_chart
  scene route_chart #@id scene.system.route_chart
    system_page kind:route_chart policy:roukaku.policy.standard #@id page.route_chart
state backlog #@id state.system.backlog
  scene backlog #@id scene.system.backlog
    system_page kind:backlog policy:roukaku.policy.standard #@id page.backlog
state localization_preview #@id state.system.localization_preview
  scene localization_preview #@id scene.system.localization_preview
    system_page kind:localization_preview policy:roukaku.policy.standard #@id page.localization_preview
"#;

#[test]
fn system_story_manifest_validates_required_entries_and_sources() {
    let compiled = compile_roukaku_project(
        [RoukakuSource::story("system.roukaku", SYSTEM_STORY)],
        Default::default(),
    )
    .unwrap();

    let manifest = SystemStoryManifest::from_compiled(&compiled).unwrap();
    let required = SystemStoryManifest::commercial_required_pages();
    let report = manifest.validate_required(&required);

    assert_eq!(report.status, SystemStoryValidationStatus::Pass);
    assert_eq!(manifest.entries.len(), required.len());
    assert_eq!(
        manifest
            .entries
            .get(&SystemPageKind::Title)
            .unwrap()
            .source_id,
        "page.title"
    );

    let mut missing = manifest.clone();
    missing.entries.remove(&SystemPageKind::Gallery);
    let report = missing.validate_required(&required);
    assert_eq!(report.status, SystemStoryValidationStatus::Blocked);
    assert!(
        report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "ROUKAKU_VN_SYSTEM_ENTRY_MISSING")
    );
}

#[test]
fn system_ui_profile_manifest_validates_migration_unlock_and_localization() {
    let compiled = compile_roukaku_project(
        [RoukakuSource::story("system.roukaku", SYSTEM_STORY)],
        Default::default(),
    )
    .unwrap();
    let mut manifest =
        VnSystemUiProfileManifest::from_ui(&compiled.ui, vec!["zh-Hans".to_string()]);
    manifest.profiles.insert(
        "classic".into(),
        SystemUiProfilePolicy {
            profile_id: "classic".into(),
            save_slot_ids: (1..=8).map(|index| format!("slot.{index:02}")).collect(),
            quick_slot_id: None,
            allowed_pages: BTreeSet::from([
                SystemPageKind::Title,
                SystemPageKind::Save,
                SystemPageKind::Load,
            ]),
            reading_modes: BTreeSet::from([ReadingMode::Manual]),
            audio_toggle: true,
            save_completion: SaveCompletionPolicy::ReturnSystem,
            custom_action_ids: BTreeSet::new(),
        },
    );

    let report = manifest.validate();
    assert_eq!(report.status, SystemStoryValidationStatus::Pass);

    let mut missing_unlock = manifest.clone();
    missing_unlock.unlock_sources.clear();
    assert!(
        missing_unlock
            .validate()
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "ROUKAKU_VN_UNLOCK_SOURCE_POLICY")
    );

    let mut missing_localization = manifest;
    missing_localization.localization.locales.clear();
    assert_eq!(
        missing_localization.validate().diagnostics[0].code,
        "ROUKAKU_VN_LOCALIZATION_COVERAGE"
    );
}
