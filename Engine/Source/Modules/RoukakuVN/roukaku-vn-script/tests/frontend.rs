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
    let ui = r#"{
  "$schema": "https://project-shinkirou.github.io/schemas/ui-screen.v1.json",
  "views": [
    {
      "id": "ui.config",
      "source_id": "ui.config",
      "model": "roukaku.vn.ui_model.config.v1",
      "theme": "theme.classic",
      "root": {
        "id": "root",
        "kind": "screen",
        "source_id": "ui.screen.jsonc/root",
        "children": [
          {
            "id": "locale",
            "kind": "select",
            "source_id": "ui.screen.jsonc/locale",
            "value": "$model.locale",
            "items": "$model.available_locales",
            "events": [
              {
                "event": "change",
                "action": "vn.set_config",
                "arguments": {
                  "key": "display.language",
                  "value": "$event.value"
                }
              }
            ]
          }
        ]
      }
    }
  ],
  "bindings": [
    {
      "id": "bind.config",
      "surface": "message",
      "view": "ui.config",
      "controller": "config",
      "policy": "standard",
      "theme": "theme.classic"
    }
  ],
  "policies": []
}"#;

    let compiled = compile_roukaku_project(
        [
            roukaku_vn_script::RoukakuSource::story("story.roukaku", story),
            roukaku_vn_script::RoukakuSource::ui("ui.screen.jsonc", ui),
        ],
        CompileRoukakuProjectOptions::default()
            .with_ui_theme(test_theme())
            .with_ui_controller_source("config", test_controller_source()),
    )
    .expect("select items should compile without repeat item_key");
    let select = &compiled.ui_blueprints.views["ui.config"].root.children[0];
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
    let ui = r#"{
  "$schema": "https://project-shinkirou.github.io/schemas/ui-screen.v1.json",
  "views": [
    {
      "id": "ui.quick",
      "source_id": "ui.quick",
      "model": "roukaku.vn.ui_model.quick_panel.v1",
      "theme": "theme.classic",
      "root": {
        "id": "root",
        "kind": "screen",
        "source_id": "ui.screen.jsonc/root",
        "children": [
          {
            "id": "auto",
            "kind": "toggle",
            "source_id": "ui.screen.jsonc/auto",
            "checked": "$model.auto_enabled",
            "events": [
              {
                "event": "change",
                "action": "vn.set_auto",
                "arguments": {
                  "enabled": "$event.value"
                }
              }
            ]
          },
          {
            "id": "skip",
            "kind": "button",
            "source_id": "ui.screen.jsonc/skip",
            "events": [
              {
                "event": "activate",
                "action": "vn.set_skip",
                "arguments": {
                  "mode": "read"
                }
              }
            ]
          }
        ]
      }
    }
  ],
  "bindings": [
    {
      "id": "bind.quick",
      "system_page": "quick_panel",
      "view": "ui.quick",
      "controller": "quick",
      "policy": "standard",
      "theme": "theme.classic"
    }
  ],
  "policies": []
}"#;
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
            roukaku_vn_script::RoukakuSource::ui("quick.screen.jsonc", ui),
        ],
        CompileRoukakuProjectOptions::default()
            .with_ui_theme(test_theme())
            .with_ui_controller_source("quick", controller),
    )
    .expect("quick panel contracts should compile");

    assert!(compiled.ui_blueprints.views.contains_key("ui.quick"));
    assert_eq!(
        compiled.ui_bindings.system_page_bindings["quick_panel"].view_id,
        "ui.quick"
    );
}

#[test]
fn tsuinosora_classic_and_modern_ui_template_compiles_as_one_project() {
    const CLASSIC_UI: &str = include_str!(
        "../../../../../../Examples/TsuiNoSora/ProjectTemplate/UI/classic.screen.jsonc"
    );
    const MODERN_UI: &str = include_str!(
        "../../../../../../Examples/TsuiNoSora/ProjectTemplate/UI/modern.screen.jsonc"
    );
    const SYSTEM_STORY: &str = include_str!(
        "../../../../../../Examples/TsuiNoSora/ProjectTemplate/Scripts/system.roukaku"
    );
    const CONTROLLERS: &str = include_str!(
        "../../../../../../Examples/TsuiNoSora/ProjectTemplate/Controllers/tsui_ui.luau"
    );
    const CLASSIC_TEST_TARGETS: &str = r#"
story k12 #@id story.fixture.k12
state k12 #@id director.k.0015
story k13 #@id story.fixture.k13
state k13 #@id director.k.0126
story k14 #@id story.fixture.k14
state k14 #@id director.k.0249
story k15 #@id story.fixture.k15
state k15 #@id director.k.0312
story k16 #@id story.fixture.k16
state k16 #@id director.k.0387
story k1701 #@id story.fixture.k1701
state k1701 #@id director.k.0456
story k1801 #@id story.fixture.k1801
state k1801 #@id director.k.0543
story t13 #@id story.fixture.t13
state t13 #@id director.t.0018
story t14 #@id story.fixture.t14
state t14 #@id director.t.0111
story t15 #@id story.fixture.t15
state t15 #@id director.t.0216
story t16 #@id story.fixture.t16
state t16 #@id director.t.0309
story t17 #@id story.fixture.t17
state t17 #@id director.t.0408
story t17_18 #@id story.fixture.t17_18
state t17_18 #@id director.t.0453
story t19 #@id story.fixture.t19
state t19 #@id director.t.0498
story y13 #@id story.fixture.y13
state y13 #@id director.y.0020
story y14 #@id story.fixture.y14
state y14 #@id director.y.0246
story y15 #@id story.fixture.y15
state y15 #@id director.y.0405
story y16 #@id story.fixture.y16
state y16 #@id director.y.0524
story y17 #@id story.fixture.y17
state y17 #@id director.y.0667
story y17_18 #@id story.fixture.y17_18
state y17_18 #@id director.y.0775
story y18_19 #@id story.fixture.y18_19
state y18_19 #@id director.y.0922
story y20 #@id story.fixture.y20
state y20 #@id director.y.1030
story z09 #@id story.fixture.z09
state z09 #@id director.z.0015
story z10 #@id story.fixture.z10
state z10 #@id director.z.0060
story z11 #@id story.fixture.z11
state z11 #@id director.z.0087
story z12 #@id story.fixture.z12
state z12 #@id director.z.0126
"#;
    const CLASSIC_THEME: &str = include_str!(
        "../../../../../../Examples/TsuiNoSora/ProjectTemplate/Themes/classic.style.jsonc"
    );
    const MODERN_THEME: &str = include_str!(
        "../../../../../../Examples/TsuiNoSora/ProjectTemplate/Themes/modern.style.jsonc"
    );

    let mut options = CompileRoukakuProjectOptions::default()
        .with_ui_style(
            roukaku_vn_script::compile_ui_style("classic.style.jsonc", CLASSIC_THEME).unwrap(),
        )
        .with_ui_style(
            roukaku_vn_script::compile_ui_style("modern.style.jsonc", MODERN_THEME).unwrap(),
        );
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

    let compiled = compile_roukaku_project(
        [
            roukaku_vn_script::RoukakuSource::story("system.roukaku", SYSTEM_STORY),
            roukaku_vn_script::RoukakuSource::story(
                "classic-test-targets.roukaku",
                CLASSIC_TEST_TARGETS,
            ),
            roukaku_vn_script::RoukakuSource::ui("classic.screen.jsonc", CLASSIC_UI),
            roukaku_vn_script::RoukakuSource::ui("modern.screen.jsonc", MODERN_UI),
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
fn unknown_widgets_and_properties_are_rejected_by_schema() {
    for root in [
        serde_json::json!({"id":"root", "kind":"component_slot"}),
        serde_json::json!({"id":"root", "kind":"screen", "component":"obsolete.widget"}),
    ] {
        let ui = serde_json::json!({"$schema":"ui-screen.v1.json", "views":[{"id":"test", "model":"roukaku.vn.ui_model.config.v1", "theme":"theme.classic", "root":root}]}).to_string();
        let error = compile_roukaku_project(
            [
                RoukakuSource::story("story.roukaku", SOURCE),
                RoukakuSource::ui("ui.screen.jsonc", ui),
            ],
            CompileRoukakuProjectOptions::default().with_ui_theme(test_theme()),
        )
        .unwrap_err();
        let roukaku_vn_script::VnError::Diagnostic(diagnostic) = error else {
            panic!("expected schema diagnostic")
        };
        assert_eq!(diagnostic.code, "ROUKAKU_UI_SCHEMA");
        assert!(diagnostic.source.is_some());
    }
}
