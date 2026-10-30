#![cfg(feature = "luau-runtime")]

use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    rc::Rc,
    sync::Arc,
};

use roukaku_core::Hash256;
use roukaku_policy::PolicyExecutionBudget;
use roukaku_runtime::TaskScope;
use roukaku_text::*;
use roukaku_vn_policy::authoring::*;
use roukaku_vn_policy::StageCommand;
use serde_json::json;

struct Presentation {
    text: CosmicTextLayoutProvider,
    displayed: RefCell<BTreeMap<String, Arc<TextLayoutResult>>>,
    properties: RefCell<BTreeMap<(String, String), f64>>,
    released: RefCell<BTreeSet<u64>>,
    presented: RefCell<Vec<(String, StageCommand)>>,
}

impl Presentation {
    fn new() -> Rc<Self> {
        let bytes =
            include_bytes!("../../../../../../Examples/NativeVN/Assets/Fonts/Poppins-Regular.ttf")
                .to_vec();
        Rc::new(Self {
            text: CosmicTextLayoutProvider::new(
                FontBindingContext {
                    target: "windows".into(),
                    profile: "classic".into(),
                    default_locale: "en-US".into(),
                },
                vec![PackagedFont {
                    asset_id: "asset:/font/poppins".into(),
                    family: "Poppins".into(),
                    face_index: 0,
                    hash: Hash256::from_sha256(&bytes),
                    license_id: "OFL-1.1".into(),
                    subset: None,
                    coverage: vec![UnicodeRange {
                        start: 0,
                        end: 0x036f,
                    }],
                    targets: vec!["windows".into()],
                    profiles: vec!["classic".into()],
                    bytes,
                }],
                TextLayoutConfig::production_defaults(),
            )
            .unwrap(),
            displayed: RefCell::new(BTreeMap::new()),
            properties: RefCell::new(BTreeMap::new()),
            presented: RefCell::new(Vec::new()),
            released: RefCell::new(BTreeSet::new()),
        })
    }

    fn property(&self, target: &str, property: &str) -> f64 {
        self.properties.borrow()[&(target.into(), property.into())]
    }
}

impl TextLayoutProvider for Presentation {
    fn identity(&self) -> Result<TextLayoutProviderIdentity, MediaError> {
        self.text.identity()
    }
    fn layout(&self, request: &TextLayoutRequest) -> Result<TextLayoutResult, MediaError> {
        self.text.layout(request)
    }
}

impl AuthoringHost for Presentation {
    fn display_text(&self, id: &str, layout: Arc<TextLayoutResult>) -> mlua::Result<()> {
        self.displayed.borrow_mut().insert(id.into(), layout);
        Ok(())
    }
    fn set_property(&self, target: &str, property: &str, value: f64) -> mlua::Result<()> {
        self.properties
            .borrow_mut()
            .insert((target.into(), property.into()), value);
        Ok(())
    }
    fn release_segment(&self, id: u64) -> mlua::Result<()> {
        assert!(
            self.released.borrow_mut().insert(id),
            "segment released twice"
        );
        Ok(())
    }
    fn present(&self, node: &str, command: &StageCommand) -> mlua::Result<()> {
        self.presented
            .borrow_mut()
            .push((node.into(), command.clone()));
        Ok(())
    }
}

fn paragraph() -> ParagraphId {
    ParagraphId {
        module: "story".into(),
        entry: "intro".into(),
    }
}
fn runtime(host: Rc<Presentation>) -> LuauAuthoringRuntime {
    LuauAuthoringRuntime::new(PolicyExecutionBudget::default(), TaskScope::new(), host).unwrap()
}
fn waiting(runtime: &LuauAuthoringRuntime) -> InputWait {
    match runtime.status() {
        StoryStatus::Waiting(wait) => wait,
        status => panic!("expected input wait: {status:?}"),
    }
}

const TIMELINE: &str = r#"
    rk.register_state("variables", "results", 0)
    rk.register_state("variables", "cancelled", 0)
    local function result()
        rk.set_state("variables", "results", rk.get_state("variables", "results") + 1)
    end
    local function cancelled()
        rk.set_state("variables", "cancelled", rk.get_state("variables", "cancelled") + 1)
    end
    local function segment()
        return rk.segment({
            duration = 2,
            tracks = {
                {target="actor", property="x", keys={{time=0,value=0},{time=2,value=100}}},
                {target="camera", property="zoom", keys={{time=0,value=1},{time=2,value=3}}},
            },
            callbacks = {
                {time=1, call={module="story",name="result"}, complete={module="story",name="result"}},
                {time=2, call={module="story",name="result"}, complete={module="story",name="result"}},
            },
            on_complete = {module="story",name="result"},
            on_cancel = {module="story",name="cancelled"},
        })
    end
    return { result=result, cancelled=cancelled, paragraphs={ intro=function()
        local id = segment()
        rk.register_state("variables", "segment", id)
        rk.set_state("variables", "segment", id)
        rk.wait_input("advance")
    end } }
"#;

#[test]
fn paragraph_lines_choices_and_waits_advance_only_on_matching_input() {
    let host = Presentation::new();
    let mut runtime = runtime(host);
    runtime
        .replace_module(
            "story",
            "@story/intro.luau",
            r#"
        return {paragraphs={intro={run=function()
            for i=1,2 do
                rk.line("greeting", {text="Hello", iteration=i})
                rk.wait_input("advance")
            end
            local node = rk.choice("route", {{id="left",text="Left"},{id="right",text="Right"}})
            local result = rk.wait_input("choose")
            rk.choice_result(node, result)
            rk.line("ending", {text=result})
            rk.wait_input("dismiss")
        end}}}
    "#,
        )
        .unwrap();
    runtime.start_paragraph(paragraph()).unwrap();
    let first = waiting(&runtime);
    assert!(!runtime.input(first.token, "choose", json!("left")).unwrap());
    assert_eq!(runtime.records().len(), 1);
    assert!(runtime.input(first.token, "advance", json!(null)).unwrap());
    assert!(!runtime.input(first.token, "advance", json!(null)).unwrap());
    let second = waiting(&runtime);
    assert!(runtime.input(second.token, "advance", json!(null)).unwrap());
    let choice = waiting(&runtime);
    assert!(
        runtime
            .input(choice.token, "choose", json!("right"))
            .unwrap()
    );
    let records = runtime.records();
    assert_eq!(records[0].occurrence.node, records[1].occurrence.node);
    assert_eq!(records[0].occurrence.visit, 1);
    assert_eq!(records[1].occurrence.visit, 2);
    assert_eq!(records[0].occurrence.paragraph, paragraph());
    assert!(
        matches!(&records[2].content, NodeContent::Choice { selected: Some(value), .. } if value == "right")
    );
    assert!(matches!(&records[3].content, NodeContent::Line { data } if data["text"] == "right"));
    let dismiss = waiting(&runtime);
    runtime
        .input(dismiss.token, "dismiss", json!(null))
        .unwrap();
    assert_eq!(runtime.status(), StoryStatus::Completed);
    runtime.start_paragraph(paragraph()).unwrap();
    // Replaying an entry generates the same logical occurrence, but a fresh input token.
    assert_eq!(runtime.records()[0].occurrence, records[0].occurrence);
    assert_ne!(waiting(&runtime).token, first.token);
}

#[test]
fn paragraph_save_reenters_entry_and_replays_to_the_saved_line() {
    let source = r#"return {paragraphs={intro=function()
        rk.register_state("variables", "count", 0)
        rk.set_state("variables", "count", rk.get_state("variables", "count") + 1)
        local draw = rk.random_u64()
        rk.line("first", {value=rk.get_state("variables", "count"), random=draw})
        rk.wait_input("advance")
        rk.line("second", {value=rk.get_state("variables", "count")})
        rk.wait_input("advance")
    end}}"#;
    let host = Presentation::new();
    let mut live = runtime(Rc::clone(&host));
    live.replace_module("story", "@story/intro.luau", source)
        .unwrap();
    live.start_paragraph(paragraph()).unwrap();
    let wait = waiting(&live);
    live.input(wait.token, "advance", json!(null)).unwrap();
    let saved = live.save_paragraph().unwrap();
    assert_eq!(saved.entry_state.variables.get("count"), None);
    assert_eq!(saved.replay.len(), 2);

    let mut loaded = runtime(host);
    loaded
        .replace_module("story", "@story/intro.luau", source)
        .unwrap();
    assert_eq!(
        loaded.load_paragraph(saved).unwrap(),
        ParagraphLoadOutcome::Replayed
    );
    assert_eq!(loaded.records(), live.records());
    assert_eq!(loaded.recoverable_state().variables["count"], json!(1));
    assert_eq!(waiting(&loaded).action, "advance");
}

#[test]
fn unsupported_cursor_save_schema_is_rejected() {
    let host = Presentation::new();
    let mut runtime = runtime(host);
    runtime
        .replace_module(
            "story",
            "@story/intro.luau",
            "return {paragraphs={intro=function() rk.wait_input('next') end}}",
        )
        .unwrap();
    assert!(runtime.save_paragraph().is_err());
    runtime.start_paragraph(paragraph()).unwrap();
    let mut save = runtime.save_paragraph().unwrap();
    save.schema = "roukaku.vn.mid_cursor_save.v1".into();
    assert!(
        runtime
            .load_paragraph(save)
            .unwrap_err()
            .to_string()
            .contains("ROUKAKU_VN_SAVE_FORMAT_UNSUPPORTED")
    );
}
#[test]
fn revision_mismatch_restarts_from_entry_and_reports_diagnostic() {
    let host = Presentation::new();
    let mut runtime = runtime(host);
    runtime
        .replace_module(
            "story",
            "@story/intro.luau",
            "return {paragraphs={intro=function() rk.line('a', {}) rk.wait_input('next') end}}",
        )
        .unwrap();
    runtime.start_paragraph(paragraph()).unwrap();
    let saved = runtime.save_paragraph().unwrap();
    runtime
        .replace_module(
            "story",
            "@story/intro.luau",
            "return {paragraphs={intro=function() rk.line('b', {}) rk.wait_input('next') end}}",
        )
        .unwrap();
    assert!(matches!(
        runtime.load_paragraph(saved).unwrap(),
        ParagraphLoadOutcome::RevisionMismatchRestarted { diagnostic }
            if diagnostic.contains("ROUKAKU_VN_SOURCE_REVISION_MISMATCH")
    ));
    assert_eq!(runtime.records()[0].occurrence.node, "b");
}

#[test]
fn paragraph_replay_deduplicates_guarded_one_shot_effects() {
    let source = r#"return {paragraphs={intro=function()
        if rk.once("voice:intro") then
            rk.set_property("audio", "voice_started", 1)
        end
        rk.line("intro", {})
        rk.wait_input("advance")
    end}}"#;
    let original_host = Presentation::new();
    let mut original = runtime(Rc::clone(&original_host));
    original
        .replace_module("story", "@story/intro.luau", source)
        .unwrap();
    original.start_paragraph(paragraph()).unwrap();
    assert_eq!(original_host.property("audio", "voice_started"), 1.0);
    let save = original.save_paragraph().unwrap();

    let replay_host = Presentation::new();
    let mut replayed = runtime(Rc::clone(&replay_host));
    replayed
        .replace_module("story", "@story/intro.luau", source)
        .unwrap();
    replayed.load_paragraph(save).unwrap();
    assert!(
        !replay_host
            .properties
            .borrow()
            .contains_key(&("audio".to_owned(), "voice_started".to_owned(),))
    );
    assert_eq!(replayed.records()[0].occurrence.node, "intro");
}
#[test]
fn rollback_replays_choice_inputs_to_selected_occurrence() {
    let host = Presentation::new();
    let mut runtime = runtime(host);
    runtime
        .replace_module(
            "story",
            "@story/intro.luau",
            r#"return {paragraphs={intro=function()
                rk.line("line", {})
                rk.wait_input("advance")
                local choice = rk.choice("route", {{id="left",text="Left"},{id="right",text="Right"}})
                local selected = rk.wait_input("choose")
                rk.choice_result(choice, selected)
                rk.line("end", {selected=selected})
                rk.wait_input("advance")
            end}}"#,
        )
        .unwrap();
    runtime.start_paragraph(paragraph()).unwrap();
    let wait = waiting(&runtime);
    runtime.input(wait.token, "advance", json!(null)).unwrap();
    let choice_occurrence = runtime.records()[1].occurrence.clone();
    let wait = waiting(&runtime);
    runtime.input(wait.token, "choose", json!("right")).unwrap();
    let wait = waiting(&runtime);
    runtime.input(wait.token, "advance", json!(null)).unwrap();
    runtime.rollback_to(&choice_occurrence).unwrap();
    assert!(matches!(
        &runtime.records()[1].content,
        NodeContent::Choice { selected: None, .. }
    ));
    assert_eq!(waiting(&runtime).action, "choose");
}

#[test]
fn stale_history_checkpoint_restarts_instead_of_resuming_changed_source() {
    let mut runtime = runtime(Presentation::new());
    runtime
        .replace_module(
            "story",
            "@story/intro.luau",
            "return {paragraphs={intro=function() rk.line('old', {}) rk.wait_input('advance') end}}",
        )
        .unwrap();
    runtime.start_paragraph(paragraph()).unwrap();
    let occurrence = runtime.records()[0].occurrence.clone();
    runtime
        .replace_module(
            "story",
            "@story/intro.luau",
            "return {paragraphs={intro=function() rk.line('new', {}) rk.wait_input('advance') end}}",
        )
        .unwrap();
    assert!(matches!(
        runtime.rollback_to(&occurrence).unwrap(),
        ParagraphLoadOutcome::RevisionMismatchRestarted { .. }
    ));
    assert_eq!(runtime.records()[0].occurrence.node, "new");
}
#[test]
fn multitrack_waits_keep_animating_and_complete_results_once() {
    let host = Presentation::new();
    let mut runtime = runtime(Rc::clone(&host));
    runtime
        .replace_module("story", "@story/stage.luau", TIMELINE)
        .unwrap();
    runtime.start_paragraph(paragraph()).unwrap();
    let id = runtime.recoverable_state().variables["segment"]
        .as_u64()
        .unwrap();
    let wait = waiting(&runtime);
    runtime.advance_presentation(0.5).unwrap();
    assert_eq!(host.property("actor", "x"), 25.0);
    assert_eq!(host.property("camera", "zoom"), 1.5);
    assert_eq!(waiting(&runtime), wait);
    runtime.advance_presentation(0.5).unwrap();
    assert_eq!(runtime.recoverable_state().variables["results"], 1);
    assert!(runtime.complete_segment(id).unwrap());
    assert_eq!(host.property("actor", "x"), 100.0);
    assert_eq!(host.property("camera", "zoom"), 3.0);
    assert_eq!(runtime.recoverable_state().variables["results"], 3);
    assert_eq!(runtime.recoverable_state().scene["actor"]["x"], 100.0);
    assert!(!runtime.complete_segment(id).unwrap());
    assert!(!runtime.cancel_segment(id).unwrap());
    runtime.advance_presentation(100.0).unwrap();
    assert_eq!(runtime.recoverable_state().variables["results"], 3);
    assert_eq!(*host.released.borrow(), BTreeSet::from([id]));
}

#[test]
fn cancelled_segments_release_without_applying_end_state_or_required_results() {
    let host = Presentation::new();
    let mut runtime = runtime(Rc::clone(&host));
    runtime
        .replace_module("story", "@story/stage.luau", TIMELINE)
        .unwrap();
    runtime.start_paragraph(paragraph()).unwrap();
    let id = runtime.recoverable_state().variables["segment"]
        .as_u64()
        .unwrap();
    runtime.advance_presentation(0.5).unwrap();
    assert!(runtime.cancel_segment(id).unwrap());
    assert!(!runtime.cancel_segment(id).unwrap());
    assert!(!runtime.complete_segment(id).unwrap());
    runtime.advance_presentation(10.0).unwrap();
    assert_eq!(host.property("actor", "x"), 25.0);
    assert_eq!(runtime.recoverable_state().variables["results"], 0);
    assert_eq!(runtime.recoverable_state().variables["cancelled"], 1);
    assert_eq!(runtime.segment_status(id), Some(SegmentStatus::Cancelled));
}

#[test]
fn registry_replacement_changes_subsequent_calls_not_the_suspended_frame() {
    let host = Presentation::new();
    let mut runtime = runtime(host);
    runtime
        .replace_module(
            "logic",
            "@logic.luau",
            "return { value=function() return 10 end }",
        )
        .unwrap();
    runtime
        .replace_module(
            "story",
            "@story.luau",
            r#"
        rk.register_state("variables", "values", {})
        return {paragraphs={intro=function()
            rk.set_state("variables", "values", {rk.call("logic", "value")})
            rk.wait_input("advance")
            rk.set_state("variables", "values", {rk.call("logic", "value"), "old frame"})
        end}}
    "#,
        )
        .unwrap();
    runtime.start_paragraph(paragraph()).unwrap();
    assert_eq!(runtime.recoverable_state().variables["values"], json!([10]));
    runtime
        .replace_module(
            "logic",
            "@logic.luau",
            "return { value=function() return 20 end }",
        )
        .unwrap();
    runtime
        .replace_module(
            "story",
            "@new-story.luau",
            "return {paragraphs={intro=function() error('new paragraph') end}}",
        )
        .unwrap();
    let wait = waiting(&runtime);
    runtime.input(wait.token, "advance", json!(null)).unwrap();
    assert_eq!(
        runtime.recoverable_state().variables["values"],
        json!([20, "old frame"])
    );
    assert_eq!(runtime.status(), StoryStatus::Completed);
    assert!(
        runtime
            .start_paragraph(paragraph())
            .unwrap_err()
            .to_string()
            .contains("new paragraph")
    );
}

#[test]
fn text_measurement_and_display_share_the_real_text_contract() {
    let host = Presentation::new();
    let mut runtime = runtime(Rc::clone(&host));
    runtime.replace_module("story", "@text.luau", r#"
        return {paragraphs={intro=function()
            local request = {
                key="authoring-line", runs={{text="Hello world",language="en-US",direction="auto",ruby={}}},
                constraint={max_width=400,font_size=24,line_height=30,wrap="word",overflow="visible"},
                font_families={"Poppins"}, features={},
            }
            local measured = rk.measure_text(request)
            local displayed = rk.display_text("dialogue", request)
            assert(measured.width > 0 and measured.height > 0)
            assert(measured.width == displayed.width and measured.revision == displayed.revision)
            rk.register_state("variables", "width", measured.width)
        end}}
    "#).unwrap();
    runtime.start_paragraph(paragraph()).unwrap();
    let displayed = host.displayed.borrow();
    let layout = &displayed["dialogue"];
    assert_eq!(
        runtime.recoverable_state().variables["width"],
        json!(layout.width)
    );
    assert!(!layout.shaped_runs.is_empty());
    assert!(!layout.glyph_resources.is_empty());
    assert!(
        layout
            .shaped_runs
            .iter()
            .flat_map(|run| &run.glyphs)
            .all(|glyph| glyph.source.end <= "Hello world".len())
    );
}

#[test]
fn parent_scope_cancellation_rejects_late_input_and_releases_segments() {
    let host = Presentation::new();
    let scope = TaskScope::new();
    let mut runtime = LuauAuthoringRuntime::new(
        PolicyExecutionBudget::default(),
        scope.clone(),
        host.clone(),
    )
    .unwrap();
    runtime
        .replace_module("story", "@stage.luau", TIMELINE)
        .unwrap();
    runtime.start_paragraph(paragraph()).unwrap();
    let id = runtime.recoverable_state().variables["segment"]
        .as_u64()
        .unwrap();
    let wait = waiting(&runtime);
    scope.cancel();
    assert!(!runtime.input(wait.token, "advance", json!(null)).unwrap());
    assert_eq!(runtime.status(), StoryStatus::Cancelled);
    assert_eq!(*host.released.borrow(), BTreeSet::from([id]));
    assert_eq!(runtime.recoverable_state().variables["results"], 0);
}

#[test]
fn state_rejects_stacks_and_restores_plain_data_without_a_coroutine() {
    let mut runtime = runtime(Presentation::new());
    runtime
        .replace_module(
            "story",
            "@state.luau",
            r#"
        rk.register_state("extensions", "plugin", {revision=1})
        return {paragraphs={intro=function()
            rk.set_state("extensions", "plugin", {revision=2,opacity=0.5})
            rk.wait_input("advance")
        end}}
    "#,
        )
        .unwrap();
    runtime.start_paragraph(paragraph()).unwrap();
    let snapshot = runtime.recoverable_state();
    assert!(runtime.restore_registered_state(snapshot.clone()).is_err());
    runtime.cancel().unwrap();
    // State installation remains available after cancellation; it cannot resume a stack.
    runtime.restore_registered_state(snapshot.clone()).unwrap();
    assert_eq!(runtime.recoverable_state(), snapshot);
    assert!(
        runtime
            .replace_module(
                "bad",
                "@bad.luau",
                "rk.register_state('variables','bad',function() end); return {}",
            )
            .is_err()
    );
    assert_eq!(runtime.recoverable_state(), snapshot);
    assert!(!runtime.recoverable_state().variables.contains_key("bad"));
}

#[test]
fn instruction_and_output_limits_stop_real_scripts() {
    let limits = PolicyExecutionBudget {
        interrupt_limit: 50,
        output_limit: 10,
        ..PolicyExecutionBudget::default()
    };
    let mut runtime =
        LuauAuthoringRuntime::new(limits, TaskScope::new(), Presentation::new()).unwrap();
    runtime
        .replace_module(
            "story",
            "@budget.luau",
            "return {paragraphs={intro=function() while true do end end}}",
        )
        .unwrap();
    assert!(
        runtime
            .start_paragraph(paragraph())
            .unwrap_err()
            .to_string()
            .contains("BUDGET")
    );
    assert_eq!(runtime.status(), StoryStatus::Failed);
    let mut runtime =
        LuauAuthoringRuntime::new(limits, TaskScope::new(), Presentation::new()).unwrap();
    runtime
        .replace_module(
            "story",
            "@output.luau",
            "return {paragraphs={intro=function() for i=1,100 do rk.line('loop',i) end end}}",
        )
        .unwrap();
    assert!(
        runtime
            .start_paragraph(paragraph())
            .unwrap_err()
            .to_string()
            .contains("BUDGET")
    );
    assert!(runtime.records().len() < 100);
}

#[test]
fn runtime_replacement_invalidates_old_wait_tokens_and_indirect_calls_can_yield() {
    let host = Presentation::new();
    let source = r#"return {paragraphs={intro=function()
        rk.call("logic", "wait")
        rk.line("resumed", {text="done"})
    end}}"#;
    let mut old = runtime(host.clone());
    old.replace_module(
        "logic",
        "@logic.luau",
        "return {wait=function() rk.wait_input('advance') end}",
    )
    .unwrap();
    old.replace_module("story", "@story.luau", source).unwrap();
    old.start_paragraph(paragraph()).unwrap();
    let stale = waiting(&old);
    old.cancel().unwrap();
    let mut fresh = runtime(host);
    fresh
        .replace_module(
            "logic",
            "@logic.luau",
            "return {wait=function() rk.wait_input('advance') end}",
        )
        .unwrap();
    fresh
        .replace_module("story", "@story.luau", source)
        .unwrap();
    fresh.start_paragraph(paragraph()).unwrap();
    let live = waiting(&fresh);
    assert_ne!(stale.token, live.token);
    assert!(!fresh.input(stale.token, "advance", json!(null)).unwrap());
    assert!(fresh.records().is_empty());
    assert!(fresh.input(live.token, "advance", json!(null)).unwrap());
    assert_eq!(fresh.status(), StoryStatus::Completed);
    assert_eq!(fresh.records().len(), 1);
}

#[test]
fn pure_sampling_runs_no_callbacks_and_natural_completion_uses_timed_logic() {
    let declaration = TimelineDeclaration {
        duration: 2.0,
        tracks: vec![PropertyTrack {
            target: "actor".into(),
            property: "x".into(),
            keys: vec![
                Keyframe {
                    time: 0.0,
                    value: 0.0,
                },
                Keyframe {
                    time: 2.0,
                    value: 100.0,
                },
            ],
        }],
        callbacks: vec![],
        on_complete: None,
        on_cancel: None,
    };
    let mut sample = None;
    declaration
        .evaluate(0.5, |target, property, value| {
            sample = Some((target.to_owned(), property.to_owned(), value));
        })
        .unwrap();
    assert_eq!(sample, Some(("actor".into(), "x".into(), 25.0)));
    let host = Presentation::new();
    let mut runtime = runtime(host.clone());
    runtime.replace_module("story", "@natural.luau", r#"
        rk.register_state("variables", "timed", 0)
        rk.register_state("variables", "skipped", 0)
        return {
            timed=function() rk.set_state("variables","timed",rk.get_state("variables","timed")+1) end,
            skipped=function() rk.set_state("variables","skipped",rk.get_state("variables","skipped")+1) end,
            paragraphs={intro=function()
                rk.segment({duration=1, tracks={}, callbacks={
                    {time=1,call={module="story",name="timed"},complete={module="story",name="skipped"}}
                }})
                rk.wait_input("advance")
            end},
        }
    "#).unwrap();
    runtime.start_paragraph(paragraph()).unwrap();
    runtime.advance_presentation(2.0).unwrap();
    runtime.advance_presentation(2.0).unwrap();
    assert_eq!(runtime.recoverable_state().variables["timed"], 1);
    assert_eq!(runtime.recoverable_state().variables["skipped"], 0);
    assert_eq!(host.released.borrow().len(), 1);
}

#[test]
fn failed_completion_is_not_retried_and_still_releases_its_resources() {
    let host = Presentation::new();
    let mut runtime = runtime(host.clone());
    runtime.replace_module("story", "@failed-complete.luau", r#"
        rk.register_state("variables","result",0)
        return {
            finish=function()
                rk.set_state("variables","result",rk.get_state("variables","result")+1)
                error("completion failed")
            end,
            paragraphs={intro=function()
                local id=rk.segment({duration=1,tracks={},on_complete={module="story",name="finish"}})
                rk.register_state("variables","segment",id)
                rk.wait_input("advance")
            end},
        }
    "#).unwrap();
    runtime.start_paragraph(paragraph()).unwrap();
    let id = runtime.recoverable_state().variables["segment"]
        .as_u64()
        .unwrap();
    assert!(runtime.complete_segment(id).is_err());
    assert!(!runtime.complete_segment(id).unwrap());
    assert_eq!(runtime.recoverable_state().variables["result"], 1);
    assert_eq!(*host.released.borrow(), BTreeSet::from([id]));
}

#[test]
fn many_presentation_ticks_use_dispatch_budgets_without_accumulating_property_copies() {
    let host = Presentation::new();
    let limits = PolicyExecutionBudget {
        output_limit: 32,
        ..PolicyExecutionBudget::default()
    };
    let mut runtime = LuauAuthoringRuntime::new(limits, TaskScope::new(), host.clone()).unwrap();
    runtime
        .replace_module("story", "@ticks.luau", TIMELINE)
        .unwrap();
    runtime.start_paragraph(paragraph()).unwrap();
    for _ in 0..5000 {
        runtime.advance_presentation(0.0001).unwrap();
    }
    assert!((host.property("actor", "x") - 25.0).abs() < 0.0001);
    assert!(matches!(runtime.status(), StoryStatus::Waiting(_)));
    assert_eq!(runtime.recoverable_state().variables["results"], 0);
}

#[test]
fn paragraph_entry_transition_starts_a_new_save_boundary() {
    let mut runtime = runtime(Presentation::new());
    runtime
        .replace_module(
            "story",
            "@transition.luau",
            r#"
return { paragraphs = {
    intro = function() rk.enter("story", "finish") end,
    finish = function() rk.line("finish", { key = "finish" }); rk.wait_input("advance") end,
} }
"#,
        )
        .unwrap();

    assert!(matches!(
        runtime.start_paragraph(paragraph()).unwrap(),
        StoryStatus::Waiting(_)
    ));
    let save = runtime.save_paragraph().unwrap();
    assert_eq!(save.paragraph.entry, "finish");
    assert_eq!(runtime.records()[0].occurrence.paragraph.entry, "finish");
}

#[test]
fn host_fence_callback_is_one_shot_and_replays_from_the_entry() {
    let source = r#"
return {
    paragraphs = { intro = function()
        rk.on_fence("voice.ready", "story", "camera_pulse", {})
        rk.line("line", { key = "line" })
        rk.wait_input("advance")
    end },
    camera_pulse = function()
        rk.set_property("camera.main", "zoom", 1.15)
    end,
}
"#;
    let host = Presentation::new();
    let mut live = runtime(host.clone());
    live.replace_module("story", "@fence.luau", source).unwrap();
    live.start_paragraph(paragraph()).unwrap();
    assert_eq!(live.signal_fence("voice.ready").unwrap(), 1);
    assert_eq!(host.property("camera.main", "zoom"), 1.15);
    assert_eq!(live.signal_fence("voice.ready").unwrap(), 0);
    let save = live.save_paragraph().unwrap();

    let restored_host = Presentation::new();
    let mut restored = runtime(restored_host.clone());
    restored
        .replace_module("story", "@fence.luau", source)
        .unwrap();
    assert_eq!(
        restored.load_paragraph(save).unwrap(),
        ParagraphLoadOutcome::Replayed
    );
    assert_eq!(restored_host.property("camera.main", "zoom"), 1.15);
    assert!(matches!(restored.status(), StoryStatus::Waiting(_)));
}

#[test]
fn replacing_default_dialog_module_keeps_records_and_rollback_checkpoints() {
    let mut runtime = runtime(Presentation::new());
    runtime
        .replace_module(
            "roukaku.vn.dialog",
            "@replacement/dialog.luau",
            r#"return { show = function(node, data)
                local occurrence = rk.line(node, data)
                rk.wait_input("advance")
                return occurrence
            end }"#,
        )
        .unwrap();
    runtime
        .replace_module(
            "story",
            "@replacement/story.luau",
            r#"return { paragraphs = { intro = function()
                rk.call("roukaku.vn.dialog", "show", "line.hello", { key = "line.hello" })
            end } }"#,
        )
        .unwrap();
    runtime.start_paragraph(paragraph()).unwrap();
    let occurrence = runtime.records()[0].occurrence.clone();
    assert!(matches!(
        &runtime.records()[0].content,
        NodeContent::Line { data } if data["key"] == "line.hello"
    ));
    let wait = waiting(&runtime);
    assert!(runtime.input(wait.token, "advance", serde_json::Value::Null).unwrap());

    assert_eq!(
        runtime.rollback_to(&occurrence).unwrap(),
        ParagraphLoadOutcome::Replayed
    );
    assert_eq!(runtime.records()[0].occurrence, occurrence);
    assert!(matches!(runtime.status(), StoryStatus::Waiting(_)));
}

#[test]
fn luau_stage_action_is_typed_and_saved_in_recoverable_scene_state() {
    let host = Presentation::new();
    let mut runtime = runtime(host.clone());
    runtime
        .replace_module(
            "story",
            "@stage-action.luau",
            r#"return { paragraphs = { intro = function()
                rk.present("opening.backdrop", "backdrop", { color = { 0, 0, 0, 255 } })
                rk.wait_input("advance")
            end } }"#,
        )
        .unwrap();
    runtime.start_paragraph(paragraph()).unwrap();

    assert!(matches!(
        &host.presented.borrow()[0],
        (node, StageCommand::Backdrop { color })
            if node == "opening.backdrop" && color == &[0, 0, 0, 255]
    ));
    assert!(runtime
        .recoverable_state()
        .scene
        .contains_key("opening.backdrop"));
}
