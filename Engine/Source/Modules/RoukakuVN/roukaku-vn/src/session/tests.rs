use super::test_support::*;
use super::*;

const BRANCHING: &str = r#"
return { paragraphs = {
  start = function()
    rk.call("roukaku.vn.dialog", "show", "intro", { key = "line.intro", speaker = "aoi", voice = "voice.intro" })
    local picked = rk.call("roukaku.vn.choice", "select", "pick", {
      { id = "left", text = "choice.left" },
      { id = "right", text = "choice.right" },
    }, { key = "choice.pick" })
    if picked == "left" then
      rk.call("roukaku.vn.dialog", "show", "left", { key = "line.left" })
    else
      rk.call("roukaku.vn.dialog", "show", "right", { key = "line.right" })
    end
    rk.enter("story.main", "ending")
  end,
  ending = function()
    rk.call("roukaku.vn.dialog", "show", "end", { key = "line.end" })
  end,
} }
"#;

const MENU: &str = r#"
return { paragraphs = {
  start = function()
    local selected = rk.call("roukaku.vn.menu", "show", "page.title", { kind = "title", policy = "roukaku.policy.standard" })
    if selected == "game.start" then
      rk.enter("story.main", "play")
    end
  end,
  play = function()
    rk.call("roukaku.vn.stage", "background", "bg.room", { asset = "asset:/bg/room", layer = "bg", duration_ms = 0, interrupt = "replace_from_current" })
    rk.call("roukaku.vn.stage", "wait", "bg.wait", "bg.ready")
    rk.call("roukaku.vn.dialog", "show", "line.play", { key = "line.play" })
  end,
} }
"#;

fn dialogue_keys(output: &NativeVnStepOutput) -> Vec<String> {
    output
        .presentations
        .iter()
        .filter_map(|command| match command {
            PresentationCommand::Dialogue { key, .. } => Some(key.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn callback_fence_runs_while_story_waits_on_another_fence() {
    let source = r#"
return {
  paragraphs = {
    start = function()
      rk.on_fence("voice.ready", "story.main", "on_ready", {})
      rk.call("roukaku.vn.stage", "wait", "advance.wait", "advance")
    end,
  },
  on_ready = function()
    rk.set_property("camera.main", "zoom", 1.15)
  end,
}
"#;
    let mut session = session("callback-fence", source);
    launch(&mut session);
    assert_eq!(session.state.pending_wait.as_ref().unwrap().fence, "advance");

    let completed = execute(
        &mut session,
        2,
        VnPlayerCommand::CompleteWait {
            fence: "voice.ready".into(),
        },
    )
    .unwrap();

    assert_eq!(session.state.pending_wait.as_ref().unwrap().fence, "advance");
    assert!(!session.callback_fences().contains("voice.ready"));
    assert_eq!(completed.properties.last().unwrap().value, 1.15);
}

#[test]
fn choice_branches_and_paragraph_entry_reach_the_next_paragraph() {
    let mut session = session("branch", BRANCHING);
    let first = launch(&mut session);
    assert_eq!(dialogue_keys(&first), ["line.intro"]);
    assert_eq!(
        session.state.pending_wait.as_ref().unwrap().kind,
        VnWaitKind::Dialogue
    );
    assert!(
        first
            .coverage_reached
            .contains(&"story.main/start".to_string())
    );

    let choice = execute(&mut session, 2, VnPlayerCommand::Advance).unwrap();
    assert!(choice.presentations.iter().any(|command| matches!(
        command,
        PresentationCommand::Choice { key, options } if key == "choice.pick" && options.len() == 2
    )));
    assert_eq!(
        session.state.pending_choice.as_ref().unwrap().choice_id,
        "pick"
    );

    let chosen = execute(
        &mut session,
        3,
        VnPlayerCommand::Choose {
            option_id: "right".into(),
        },
    )
    .unwrap();
    assert_eq!(dialogue_keys(&chosen), ["line.right"]);
    assert!(session.state.pending_choice.is_none());
    let ending = execute(&mut session, 4, VnPlayerCommand::Advance).unwrap();
    assert_eq!(dialogue_keys(&ending), ["line.end"]);
    assert!(
        ending
            .coverage_reached
            .contains(&"story.main/ending".to_string())
    );
    let keys = session
        .state
        .backlog
        .iter()
        .map(|entry| entry.key.as_str())
        .collect::<Vec<_>>();
    assert_eq!(keys, ["line.intro", "line.right", "line.end"]);
    assert!(session.state.voice_replay.contains_key("voice.intro"));
    execute(&mut session, 5, VnPlayerCommand::Advance).unwrap();
    assert!(session.state.cursor.is_none());
    assert!(session.state.pending_wait.is_none());
}

#[test]
fn unavailable_choice_option_fails_the_step() {
    let mut session = session("unknown", BRANCHING);
    launch(&mut session);
    execute(&mut session, 2, VnPlayerCommand::Advance).unwrap();
    let error = execute(
        &mut session,
        3,
        VnPlayerCommand::Choose {
            option_id: "middle".into(),
        },
    )
    .unwrap_err();
    assert_eq!(error.code(), "ROUKAKU_VN_CHOICE_OPTION");
}

#[test]
fn story_menu_is_a_system_page_resolved_by_profile_actions_and_fences() {
    let mut session = session("menu", MENU);
    session.bind_system_actions(BTreeSet::from(["game.start".to_string()]));
    let title = launch(&mut session);
    assert!(
        title
            .presentations
            .contains(&PresentationCommand::SystemPage {
                page: SystemPageKind::Title
            })
    );
    assert_eq!(session.state.system_stack.len(), 1);
    assert_eq!(session.state.system_stack[0].page, SystemPageKind::Title);
    assert!(session.state.pending_choice.is_none());

    // A user-opened page stacks above the story menu and returns to it.
    execute(
        &mut session,
        2,
        VnPlayerCommand::OpenSystem {
            page: SystemPageKind::Config,
        },
    )
    .unwrap();
    assert_eq!(session.state.system_stack.len(), 2);
    execute(&mut session, 3, VnPlayerCommand::ReturnSystem).unwrap();
    assert_eq!(session.state.system_stack.len(), 1);
    assert_eq!(
        session.state.pending_wait.as_ref().unwrap().kind,
        VnWaitKind::SystemPage
    );

    let play = execute(
        &mut session,
        4,
        VnPlayerCommand::InvokeSystemAction {
            action_id: "game.start".into(),
        },
    )
    .unwrap();
    assert!(session.state.system_stack.is_empty());
    assert!(play.presentations.iter().any(|command| matches!(
        command,
        PresentationCommand::Stage(StageCommand::Background { .. })
    )));
    let wait = session.state.pending_wait.clone().unwrap();
    assert_eq!(
        (wait.kind, wait.fence.as_str()),
        (VnWaitKind::Fence, "bg.ready")
    );
    // Input never completes a fence wait; only its fence does.
    execute(&mut session, 5, VnPlayerCommand::Advance).unwrap();
    assert_eq!(
        session.state.pending_wait.as_ref().unwrap().fence,
        "bg.ready"
    );
    let line = execute(
        &mut session,
        6,
        VnPlayerCommand::CompleteWait {
            fence: "bg.ready".into(),
        },
    )
    .unwrap();
    assert_eq!(dialogue_keys(&line), ["line.play"]);
}

#[test]
fn returning_from_a_story_menu_selects_return() {
    let mut session = session("menu.return", MENU);
    launch(&mut session);
    execute(&mut session, 2, VnPlayerCommand::ReturnSystem).unwrap();
    assert!(session.state.system_stack.is_empty());
    assert!(session.state.cursor.is_none());
}

#[test]
fn invalid_engine_step_cancels_the_session_scope() {
    for (step, seed, mode) in [
        (2, 23, roukaku_runtime::TickMode::Live),
        (1, 99, roukaku_runtime::TickMode::Live),
        (1, 23, roukaku_runtime::TickMode::RestoreContinuation),
    ] {
        let mut session = session("invalid-step", BRANCHING);
        let mut input = input(step, NativeVnStepCommand::LaunchDefault, mode);
        input.timing.seed = seed;
        assert!(session.step(input).is_err());
        assert!(session.state.backlog.is_empty());
        assert!(session.engine.world().task_scope().is_cancelled());
        assert!(
            launch_result(&mut session)
                .unwrap_err()
                .code()
                .contains("SESSION_FAILED")
        );
    }
}

fn launch_result(session: &mut VnSession) -> Result<NativeVnStepOutput, VnError> {
    session.step(input(
        1,
        NativeVnStepCommand::LaunchDefault,
        roukaku_runtime::TickMode::Live,
    ))
}

#[test]
fn closing_one_owned_session_does_not_cancel_another() {
    let mut first = session("one", BRANCHING);
    let mut second = session("two", BRANCHING);
    launch(&mut first);
    let first_scope = first.engine.world().task_scope();
    let second_scope = second.engine.world().task_scope();
    first.close();
    assert!(first_scope.is_cancelled());
    assert!(!second_scope.is_cancelled());
    launch(&mut second);
    assert!(second.state.pending_wait.is_some());
}

#[test]
fn fast_forward_skips_dialogue_until_a_choice() {
    let mut session = session("skip", BRANCHING);
    launch(&mut session);
    execute(
        &mut session,
        2,
        VnPlayerCommand::SetReadingMode {
            mode: ReadingMode::FastForward,
        },
    )
    .unwrap();
    assert!(session.state.pending_choice.is_some());
}

#[test]
fn luau_segments_emit_property_samples_over_presentation_time() {
    let mut session = session(
        "segment",
        r#"return { paragraphs = { start = function()
            rk.segment({ duration = 1, tracks = {
              { target = "hero", property = "opacity", keys = { { time = 0, value = 0 }, { time = 1, value = 1 } } },
            } })
            rk.call("roukaku.vn.dialog", "show", "line", { key = "line" })
        end } }"#,
    );
    let first = launch(&mut session);
    assert_eq!(first.properties[0].value, 0.0);
    let samples = session.advance_presentation(0.5).unwrap();
    let sample = samples.last().unwrap();
    assert_eq!(
        (sample.target.as_str(), sample.property.as_str()),
        ("hero", "opacity")
    );
    assert!((sample.value - 0.5).abs() < 1e-9);
}
