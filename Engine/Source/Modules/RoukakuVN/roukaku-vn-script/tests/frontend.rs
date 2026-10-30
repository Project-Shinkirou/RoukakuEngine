use roukaku_ui_core::{UiThemeManifest, UiThemeValue};
use roukaku_vn_script::{
    CompileRoukakuProjectOptions, ExtensionCommandDescriptor, ExtensionFieldContract,
    ExtensionFieldKind, FormatOptions, RoukakuSource, SEMANTIC_PASS_ORDER, ScriptLanguageService,
    SemanticPass, SyntaxKind, compile_roukaku_project, format_roukaku_source, parse_roukaku_source,
};
use std::collections::BTreeMap;

const SOURCE: &str = "# leading\nstory main #@id story.main\n\nstate prologue #@id state.prologue\n  scene room #@id scene.room\n    text key:\"line hello\" speaker:hero #@id line.hello\n";

#[test]
fn lossless_parse_preserves_comments_blank_lines_quotes_and_spans() {
    let parsed = parse_roukaku_source("story.roukaku", SOURCE);
    assert_eq!(parsed.cst.text(), SOURCE);
    let kinds = parsed
        .cst
        .descendants()
        .map(|node| node.kind())
        .collect::<Vec<_>>();
    assert!(kinds.contains(&SyntaxKind::Story));
    assert!(kinds.contains(&SyntaxKind::State));
    assert!(kinds.contains(&SyntaxKind::Scene));
    assert!(kinds.contains(&SyntaxKind::Command));
    assert!(parsed.diagnostics.is_empty(), "{:#?}", parsed.diagnostics);
    let text = parsed
        .ast
        .commands()
        .find(|command| command.keyword() == "text")
        .unwrap();
    assert_eq!(text.attribute("key").unwrap().value(), "line hello");
    assert_eq!(text.source_id(), Some("line.hello"));
    assert_eq!(
        u32::from(text.keyword_span().start),
        SOURCE.find("text").unwrap() as u32
    );
}

#[test]
fn semantic_pass_order_is_stable_and_language_service_exposes_navigation() {
    assert_eq!(
        SEMANTIC_PASS_ORDER,
        [
            SemanticPass::Symbols,
            SemanticPass::Routes,
            SemanticPass::Variables,
            SemanticPass::Commands,
            SemanticPass::SystemStories,
            SemanticPass::CompiledStory,
        ]
    );
    let source = "story main #@id story.main\nstate start #@id state.start\n  scene room #@id scene.room\n    jump target:state.start #@id jump.loop\n";
    let service = ScriptLanguageService::new("service.roukaku", source);
    assert!(service.definition("state.start").is_some());
    assert_eq!(service.references("state.start").len(), 1);
    assert!(
        service
            .semantic_tokens()
            .iter()
            .any(|token| token.kind == "attribute")
    );
}

#[test]
fn semantic_hash_ignores_trivia_and_implicit_command_source_spans() {
    let compact = "story main\nstate start\n  scene room\n    text key:hello\n";
    let with_trivia =
        "# comment\nstory   main\n\nstate start\n  scene room\n    text   key:hello # comment\n";
    let left = compile_roukaku_project(
        [roukaku_vn_script::RoukakuSource::story(
            "story.roukaku",
            compact,
        )],
        Default::default(),
    )
    .unwrap();
    let right = compile_roukaku_project(
        [roukaku_vn_script::RoukakuSource::story(
            "story.roukaku",
            with_trivia,
        )],
        Default::default(),
    )
    .unwrap();
    assert_eq!(left.story_hash, right.story_hash);
    assert_ne!(left.source_map.hash, right.source_map.hash);
}

#[test]
fn select_items_are_a_typed_widget_property_not_a_repeat_binding() {
    let story = "story main #@id story.main\nstate start #@id state.start\n  scene room #@id scene.room\n    text key:line.one #@id line.one\n";
    let ui = concat!(
        "ui_bind surface:message view:ui.config controller:config policy:standard ",
        "theme:theme.classic #@id bind.config\n",
        "ui_view ui.config model:roukaku.vn.ui_model.config.v1 ",
        "theme:theme.classic #@id ui.config\n",
        "  screen id:root\n",
        "    select id:locale value:$model.locale items:$model.available_locales\n",
        "      on change -> vn.set_config key:display.language value:$event.value\n",
    );

    let compiled = compile_roukaku_project(
        [
            roukaku_vn_script::RoukakuSource::story("story.roukaku", story),
            roukaku_vn_script::RoukakuSource::ui("ui.roukaku", ui),
        ],
        CompileRoukakuProjectOptions::default()
            .with_ui_theme(test_theme())
            .with_ui_controller_source("config", test_controller_source()),
    )
    .expect("select items should compile without repeat item_key");
    let select = &compiled.ui.ui_blueprints.views["ui.config"].root.children[0];
    assert!(select.repeat.is_none());
    assert!(select.properties.contains_key("items"));
}

#[test]
fn quick_panel_system_page_and_actions_compile_as_typed_ui_contracts() {
    let story = concat!(
        "story system #@id story.system\n",
        "state quick #@id state.quick\n",
        "  scene quick #@id scene.quick\n",
        "    system_page kind:quick_panel #@id page.quick\n",
    );
    let ui = concat!(
        "ui_bind system_page:quick_panel view:ui.quick controller:quick policy:standard ",
        "theme:theme.classic #@id bind.quick\n",
        "ui_view ui.quick model:roukaku.vn.ui_model.quick_panel.v1 ",
        "theme:theme.classic #@id ui.quick\n",
        "  screen id:root\n",
        "    toggle id:auto checked:$model.auto_enabled\n",
        "      on change -> vn.set_auto enabled:$event.value\n",
        "    button id:skip\n",
        "      on activate -> vn.set_skip mode:\"read\"\n",
    );
    let controller = r#"
roukaku.ui.controller.register("quick", {
  schema = "roukaku.vn.ui_controller.v1",
  view = "ui.quick",
  model_schema = "roukaku.vn.ui_model.quick_panel.v1",
  snapshot = "session",
}, {
  on_action = function(_, _, action)
    return { roukaku.ui.effect.forward(action) }
  end,
})
"#;

    let compiled = compile_roukaku_project(
        [
            roukaku_vn_script::RoukakuSource::story("system.roukaku", story),
            roukaku_vn_script::RoukakuSource::ui("quick.roukaku", ui),
        ],
        CompileRoukakuProjectOptions::default()
            .with_ui_theme(test_theme())
            .with_ui_controller_source("quick", controller),
    )
    .expect("quick panel contracts should compile");

    assert!(compiled.ui.ui_blueprints.views.contains_key("ui.quick"));
    assert_eq!(
        compiled.ui.ui_bindings.system_page_bindings["quick_panel"].view_id,
        "ui.quick"
    );
}

#[test]
fn tsuinosora_classic_and_modern_ui_template_compiles_as_one_project() {
    const CLASSIC_UI: &str =
        include_str!("../../../../../../Examples/TsuiNoSora/ProjectTemplate/UI/classic.roukaku");
    const MODERN_UI: &str =
        include_str!("../../../../../../Examples/TsuiNoSora/ProjectTemplate/UI/modern.roukaku");
    const CONTROLLERS: &str = include_str!(
        "../../../../../../Examples/TsuiNoSora/ProjectTemplate/Controllers/tsui_ui.luau"
    );
    const CLASSIC_THEME: &str =
        include_str!("../../../../../../Examples/TsuiNoSora/ProjectTemplate/Themes/classic.json");
    const MODERN_THEME: &str =
        include_str!("../../../../../../Examples/TsuiNoSora/ProjectTemplate/Themes/modern.json");

    let mut options = CompileRoukakuProjectOptions::default()
        .with_ui_theme(theme_from_source(CLASSIC_THEME))
        .with_ui_theme(theme_from_source(MODERN_THEME));
    for id in [
        "tsui.message.classic",
        "tsui.monologue.classic",
        "tsui.opening.staggered.classic",
        "tsui.opening.centered.classic",
        "tsui.choice.classic",
        "tsui.system.title.classic",
        "tsui.system.popup.classic",
        "tsui.system.save.classic",
        "tsui.system.load.classic",
        "tsui.system.config.classic",
        "tsui.system.test.classic",
        "tsui.message.modern",
        "tsui.monologue.modern",
        "tsui.choice.modern",
        "tsui.system.title.modern",
        "tsui.system.quick_panel.modern",
        "tsui.system.save.modern",
        "tsui.system.load.modern",
        "tsui.system.config.modern",
        "tsui.system.backlog.modern",
    ] {
        options = options.with_ui_controller_source(id, CONTROLLERS);
    }

    let compiled = roukaku_vn_script::compile_roukaku_ui(
        [
            roukaku_vn_script::RoukakuSource::ui("classic.roukaku", CLASSIC_UI),
            roukaku_vn_script::RoukakuSource::ui("modern.roukaku", MODERN_UI),
        ],
        options,
    )
    .expect("the checked-in TsuiNoSora UI template must compile");

    assert_eq!(compiled.ui_blueprints.views.len(), 21);
    assert!(
        compiled
            .ui_blueprints
            .views
            .contains_key("ui.tsui.modern.quick_panel")
    );
    for view_id in ["ui.tsui.classic.monologue", "ui.tsui.modern.monologue"] {
        let view = compiled
            .ui_blueprints
            .views
            .get(view_id)
            .expect("typed monologue view");
        assert_eq!(view.model_schema, "roukaku.vn.ui_model.message.v2");
    }
    for profile in ["classic", "modern"] {
        assert!(
            compiled
                .ui_bindings
                .profile_scoped_bindings
                .get(profile)
                .and_then(|bindings| bindings.surface_bindings.get("tsui.surface.monologue"))
                .is_some()
        );
    }
    let classic = compiled
        .system_ui_profiles
        .get("classic")
        .expect("classic system UI policy");
    assert_eq!(classic.save_slot_ids.len(), 8);
    assert_eq!(classic.save_slot_ids.first().unwrap(), "slot.01");
    assert_eq!(classic.save_slot_ids.last().unwrap(), "slot.08");
    assert!(classic.quick_slot_id.is_none());
    assert_eq!(classic.custom_action_ids.len(), 31);
    let modern = compiled
        .system_ui_profiles
        .get("modern")
        .expect("modern system UI policy");
    assert_eq!(modern.save_slot_ids.len(), 25);
    assert_eq!(modern.quick_slot_id.as_deref(), Some("slot.quick"));
}

fn theme_from_source(source: &str) -> UiThemeManifest {
    let value: serde_json::Value = serde_json::from_str(source).expect("theme JSON");
    let object = value.as_object().expect("theme object");
    UiThemeManifest {
        schema: object["schema"].as_str().unwrap().to_string(),
        id: object["id"].as_str().unwrap().to_string(),
        parent: object
            .get("parent")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        tokens: serde_json::from_value(object["tokens"].clone()).expect("theme tokens"),
        high_contrast_tokens: serde_json::from_value(
            object
                .get("high_contrast_tokens")
                .cloned()
                .unwrap_or_else(|| serde_json::json!({})),
        )
        .expect("high contrast tokens"),
        revision: 1,
    }
}

fn test_theme() -> UiThemeManifest {
    UiThemeManifest {
        schema: "roukaku.ui_theme_manifest.v1".into(),
        id: "theme.classic".into(),
        parent: None,
        tokens: BTreeMap::from([("surface".into(), UiThemeValue::Color([0, 0, 0, 255]))]),
        high_contrast_tokens: BTreeMap::new(),
        revision: 1,
    }
}

fn test_controller_source() -> &'static str {
    r#"
roukaku.ui.controller.register("config", {
  schema = "roukaku.vn.ui_controller.v1",
  view = "ui.config",
  model_schema = "roukaku.vn.ui_model.config.v1",
  snapshot = "none",
}, {
  on_action = function(_, _, action)
    return { roukaku.ui.effect.forward(action) }
  end,
})
"#
}

#[test]
fn unknown_command_is_editable_but_requires_explicit_compile_binding() {
    let source = "story main\nstate start\n  scene room\n    studio_fx intensity:2 #@id fx.1\n";
    let parsed = parse_roukaku_source("unknown.roukaku", source);
    assert!(
        parsed
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "ROUKAKU_VN_UNKNOWN_COMMAND")
    );

    let error = compile_roukaku_project(
        [roukaku_vn_script::RoukakuSource::story(
            "unknown.roukaku",
            source,
        )],
        CompileRoukakuProjectOptions::default(),
    )
    .unwrap_err();
    assert_eq!(error.code(), "ROUKAKU_VN_COMMAND_UNBOUND");

    let compiled = compile_roukaku_project(
        [roukaku_vn_script::RoukakuSource::story(
            "unknown.roukaku",
            source,
        )],
        CompileRoukakuProjectOptions::default().bind_extension(ExtensionCommandDescriptor {
            command: "studio_fx".to_string(),
            provider_id: "studio.presentation".to_string(),
            schema: "studio.presentation.fx.v1".to_string(),
            fields: BTreeMap::from([(
                "intensity".to_string(),
                ExtensionFieldContract {
                    kind: ExtensionFieldKind::Integer,
                    required: true,
                },
            )]),
        }),
    )
    .unwrap();
    assert_eq!(compiled.schema, "roukaku.vn.compiled_project.v5");
}

#[test]
fn standard_audio_control_is_bound_without_an_extension_bypass() {
    let source = "story main\nstate start\n  scene room\n    audio action:pause target:bgm.main #@id audio.pause\n";
    let parsed = parse_roukaku_source("audio.roukaku", source);
    assert!(
        !parsed
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "ROUKAKU_VN_UNKNOWN_COMMAND")
    );

    let compiled = compile_roukaku_project(
        [roukaku_vn_script::RoukakuSource::story(
            "audio.roukaku",
            source,
        )],
        Default::default(),
    )
    .unwrap();
    assert_eq!(compiled.schema, "roukaku.vn.compiled_project.v5");
}

#[test]
fn formatter_is_idempotent_and_preserves_semantics() {
    let formatted =
        format_roukaku_source("story.roukaku", SOURCE, FormatOptions::default()).unwrap();
    assert_eq!(
        format_roukaku_source("story.roukaku", &formatted, FormatOptions::default()).unwrap(),
        formatted
    );
    assert_eq!(
        format_roukaku_source(
            "story.roukaku",
            &(SOURCE.to_string() + "\n"),
            FormatOptions::default()
        )
        .unwrap(),
        formatted
    );
    let service = ScriptLanguageService::new("story.roukaku", &formatted);
    assert!(
        service
            .symbols()
            .iter()
            .any(|symbol| symbol.id == "line.hello")
    );
    assert!(
        service
            .hover(formatted.find("text").unwrap() as u32 + 1)
            .is_some()
    );
}

#[test]
fn dynamic_ui_component_authoring_is_rejected() {
    for ui in [
        "ui_component obsolete.widget #@id obsolete.widget\n",
        "ui_view ui.test model:roukaku.vn.ui_model.config.v1 theme:theme.classic #@id ui.test\n  component_slot id:root component:obsolete.widget\n",
        "ui_view ui.test model:roukaku.vn.ui_model.config.v1 theme:theme.classic #@id ui.test\n  screen id:root component:obsolete.widget\n",
    ] {
        assert!(
            compile_roukaku_project(
                [
                    RoukakuSource::story("story.roukaku", SOURCE),
                    RoukakuSource::ui("ui.roukaku", ui)
                ],
                CompileRoukakuProjectOptions::default().with_ui_theme(test_theme()),
            )
            .is_err()
        );
    }
}
