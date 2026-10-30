use roukaku_core::Diagnostic;
use roukaku_ui_core::{UiValue, UiValueExpr};
use roukaku_vn_script::{
    CompileRoukakuProjectOptions, CompiledVnProject, RoukakuSource, VnError,
    compile_roukaku_project, compile_ui_style,
};
use serde_json::{Value, json};

const STORY: &str = "story main #@id story.main\nstate start #@id state.start\n  scene room #@id scene.room\n    text key:hello #@id line.hello\n";
const STYLE: &str = r##"{
  "$schema": "ui-style.v1.json",
  "$id": "theme.test",
  ":root": {"surface": {"color": [1, 2, 3, 255]}},
  "button": {"min_height": 40},
  ".primary": {"min_height": 50},
  "#start": {"min_height": 60}
}"##;

fn screen(root: Value) -> Value {
    json!({"$schema":"ui-screen.v1.json", "views":[{"id":"test", "model":"roukaku.vn.ui_model.system.v1", "theme":"theme.test", "root":root}]})
}

fn compile(text: &str) -> Result<CompiledVnProject, VnError> {
    compile_roukaku_project(
        [
            RoukakuSource::story("story.roukaku", STORY),
            RoukakuSource::ui("UI/test.screen.jsonc", text),
        ],
        CompileRoukakuProjectOptions::default()
            .with_ui_style(compile_ui_style("Themes/test.style.jsonc", STYLE)?),
    )
}

fn diagnostic(error: VnError) -> Diagnostic {
    let VnError::Diagnostic(diagnostic) = error else {
        panic!("expected located diagnostic, got {error}")
    };
    diagnostic
}

#[test]
fn schema_failure_points_to_nested_value_with_unicode_line_and_column() {
    let text = "{\n  \"$schema\": \"ui-screen.v1.json\",\n  \"views\": [{\"id\":\"test\",\"model\":\"roukaku.vn.ui_model.system.v1\",\"theme\":\"theme.test\",\"root\": {\n    \"id\": \"根\", \"kind\": \"button\", \"min_height\": false\n  }}]\n}";
    let error = diagnostic(compile(text).unwrap_err());
    assert_eq!(error.code, "ROUKAKU_UI_SCHEMA");
    let source = error.source.unwrap();
    assert_eq!(source.source, "UI/test.screen.jsonc");
    assert_eq!(source.line, 4);
    assert_eq!(
        source.column as usize,
        text.lines()
            .nth(3)
            .unwrap()
            .split("false")
            .next()
            .unwrap()
            .chars()
            .count()
            + 1
    );
    assert_eq!(source.length, 5);
}

#[test]
fn missing_control_identity_is_a_located_schema_error() {
    for node in [json!({"kind":"screen"}), json!({"id":"root"})] {
        let error = diagnostic(compile(&screen(node).to_string()).unwrap_err());
        assert_eq!(error.code, "ROUKAKU_UI_SCHEMA");
        assert_eq!(error.source.unwrap().source, "UI/test.screen.jsonc");
    }
}

#[test]
fn conflicting_position_and_anchor_are_rejected_before_rendering() {
    for key in [
        "position_x",
        "position_y",
        "position_right",
        "position_bottom",
    ] {
        let mut node = json!({"id":"root", "kind":"panel", "anchor":"bottom"});
        node[key] = json!(0);
        let error = diagnostic(compile(&screen(node).to_string()).unwrap_err());
        assert_eq!(error.code, "ROUKAKU_UI_POSITION_CONFLICT");
        assert!(error.source.unwrap().column > 1);
    }
}

#[test]
fn jsonc_trivia_and_member_order_do_not_change_runtime_blueprint() {
    let plain = screen(json!({"id":"root","kind":"screen","children":[{"id":"start","kind":"button","text":"Start"}]})).to_string();
    let commented = format!(
        "/* opening */\n{}\n// trailing\n",
        plain
            .replacen(
                "\"kind\":\"button\"",
                "\"kind\": /* author comment */ \"button\"",
                1
            )
            .replacen("\"text\":\"Start\"}", "\"text\":\"Start\",}", 1)
    );
    let left = compile(&plain).unwrap();
    let right = compile(&commented).unwrap();
    assert_eq!(left.ui_blueprints, right.ui_blueprints);
    assert_eq!(left.project_hash, right.project_hash);
}

#[test]
fn selectors_cascade_by_specificity_then_inline_values() {
    let doc = screen(
        json!({"id":"root", "kind":"column", "children":[{"id":"plain","kind":"button"},{"id":"classed","kind":"button","style":"primary"},{"id":"start","kind":"button","style":"primary"},{"id":"inline","kind":"button","style":"primary","min_height":70}]}),
    );
    let compiled = compile(&doc.to_string()).unwrap();
    for (node, expected) in compiled.ui_blueprints.views["test"]
        .root
        .children
        .iter()
        .zip([40, 50, 60, 70])
    {
        assert_eq!(
            node.properties["min_height"],
            UiValueExpr::Literal {
                value: UiValue::Integer(expected)
            }
        );
    }
    assert_eq!(compiled.themes["theme.test"].tokens.len(), 1);
}

#[test]
fn duplicate_keys_and_invalid_jsonc_are_located() {
    for text in [
        r#"{"$schema":"ui-screen.v1.json", "$schema":"ui-screen.v1.json"}"#,
        "{\n  bare: 1\n}",
        "{\n  \"$schema\": \"ui-screen.v1.json\"\n  \"views\": []\n}",
    ] {
        let error = diagnostic(compile(text).unwrap_err());
        let source = error.source.unwrap();
        assert_eq!(source.source, "UI/test.screen.jsonc");
        assert!(source.line >= 1 && source.column >= 1);
    }
}

#[test]
fn style_schema_errors_preserve_style_file_location() {
    let text = STYLE.replace("[1, 2, 3, 255]", "[1, 2, 3, 999]");
    let error = diagnostic(compile_ui_style("Themes/test.style.jsonc", &text).unwrap_err());
    assert_eq!(error.code, "ROUKAKU_UI_SCHEMA");
    let source = error.source.unwrap();
    assert_eq!(source.source, "Themes/test.style.jsonc");
    assert_eq!(source.line, 4);
}

#[test]
fn stylesheet_position_conflict_is_not_hidden_by_inline_properties() {
    let style = STYLE.replace(
        "\"button\": {\"min_height\": 40}",
        "\"button\": {\"anchor\":\"bottom\"}",
    );
    let error = compile_roukaku_project(
        [
            RoukakuSource::story("story.roukaku", STORY),
            RoukakuSource::ui(
                "test.screen.jsonc",
                screen(json!({"id":"start","kind":"button","position_x":1})).to_string(),
            ),
        ],
        CompileRoukakuProjectOptions::default()
            .with_ui_style(compile_ui_style("test.style.jsonc", &style).unwrap()),
    )
    .unwrap_err();
    assert_eq!(diagnostic(error).code, "ROUKAKU_UI_POSITION_CONFLICT");
}
