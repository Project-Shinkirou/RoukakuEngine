use roukaku_vn_policy::{
    LuauPolicy, PolicyExecutionBudget, PolicyQueryContext, PolicySnapshotValue, VnPolicyState,
};

#[test]
fn luau_policy_records_authorized_mutation_trace() {
    let mut policy = LuauPolicy::new().unwrap();
    let mut state = VnPolicyState::default();

    let result = policy
        .eval_bool(
            r#"
            roukaku.mutate.set_var("project", "affinity", 3)
            return roukaku.var.get("project", "affinity") == 3
            "#,
            &mut state,
        )
        .unwrap();

    assert!(result);
    assert_eq!(state.var("project", "affinity"), Some(3));
    assert_eq!(state.mutation_trace.len(), 1);
    let entry = &state.mutation_trace[0];
    assert_eq!(entry.api, "roukaku.mutate.set_var");
    assert_eq!(entry.dirty_scope, "project");
    assert_eq!(entry.rollback_scope, "project");
    assert_eq!(entry.replay_event, "vn.mutation.set_var");
    assert_eq!(entry.previous_value, None);
}

#[test]
fn luau_policy_rolls_back_and_replays_mutation_trace() {
    let mut policy = LuauPolicy::new().unwrap();
    let mut state = VnPolicyState::default();
    state
        .variables
        .entry("project".to_string())
        .or_default()
        .insert("affinity".to_string(), 1);

    let result = policy
        .eval_bool(
            r#"
            roukaku.mutate.set_var("project", "affinity", 7)
            roukaku.mutate.set_var("temp", "flag", 1)
            roukaku.mutate.set_var("project", "unlocked", 1)
            return roukaku.var.get("project", "affinity") == 7
                and roukaku.var.get("temp", "flag") == 1
                and roukaku.var.get("project", "unlocked") == 1
            "#,
            &mut state,
        )
        .unwrap();

    assert!(result);
    let trace = state.mutation_trace.clone();
    assert_eq!(trace.len(), 3);
    assert_eq!(trace[0].previous_value, Some(1));
    assert_eq!(trace[1].previous_value, None);
    assert_eq!(trace[2].previous_value, None);

    let rolled_back = state.rollback_scope("project");
    assert_eq!(rolled_back.len(), 2);
    assert_eq!(state.var("project", "affinity"), Some(1));
    assert_eq!(state.var("project", "unlocked"), None);
    assert_eq!(state.var("temp", "flag"), Some(1));
    assert_eq!(state.mutation_trace.len(), 1);
    assert_eq!(state.mutation_trace[0].scope, "temp");

    let mut replayed = VnPolicyState::default();
    replayed
        .variables
        .entry("project".to_string())
        .or_default()
        .insert("affinity".to_string(), 1);
    replayed.replay_mutation_trace(&trace);

    assert_eq!(replayed.var("project", "affinity"), Some(7));
    assert_eq!(replayed.var("project", "unlocked"), Some(1));
    assert_eq!(replayed.var("temp", "flag"), Some(1));
    assert_eq!(replayed.mutation_trace, trace);
}

#[test]
fn luau_policy_records_command_query_and_trace_capabilities() {
    let mut policy = LuauPolicy::new().unwrap();
    let mut state = VnPolicyState::default();

    let object = |values: &[(&str, PolicySnapshotValue)]| {
        PolicySnapshotValue::Object(
            values
                .iter()
                .map(|(key, value)| ((*key).to_string(), value.clone()))
                .collect(),
        )
    };
    let queries = PolicyQueryContext {
        text: [(
            "zh-Hans:line.001".to_string(),
            object(&[
                ("key", PolicySnapshotValue::String("line.001".to_string())),
                ("locale", PolicySnapshotValue::String("zh-Hans".to_string())),
            ]),
        )]
        .into_iter()
        .collect(),
        assets: [(
            "bg.school".to_string(),
            object(&[("id", PolicySnapshotValue::String("bg.school".to_string()))]),
        )]
        .into_iter()
        .collect(),
        backlog: object(&[("count", PolicySnapshotValue::Integer(0))]),
        savepoint: object(&[("available", PolicySnapshotValue::Bool(true))]),
        layouts: [(
            "message".to_string(),
            object(&[("target", PolicySnapshotValue::String("message".to_string()))]),
        )]
        .into_iter()
        .collect(),
    };
    let result = policy
        .eval_bool_with_context(
            r#"
            roukaku.command.register("show.hero", { schema = "roukaku.command.show.v2" }, function() return true end)
            roukaku.command.emit("show.hero", { layer = "hero", x = 320 })
            roukaku.command.enqueue("voice.line", { asset = "voice.001" })
            roukaku.command.filter("show.hero", function(command) return command end)
            local text = roukaku.query.text("line.001", "zh-Hans")
            local asset = roukaku.query.asset("bg.school")
            local backlog = roukaku.query.backlog()
            local savepoint = roukaku.query.savepoint()
            local layout = roukaku.query.layout("message")
            roukaku.trace.event("system.open", { page = "save" })
            roukaku.trace.performance_scope("title.boot")
            return text.key == "line.001"
                and text.locale == "zh-Hans"
                and asset.id == "bg.school"
                and backlog.count == 0
                and savepoint.available == true
                and layout.target == "message"
            "#,
            &mut state,
            &queries,
            PolicyExecutionBudget::default(),
        )
        .unwrap();

    assert!(result);
    assert_eq!(state.command_trace.len(), 4);
    assert_eq!(state.command_trace[0].api, "roukaku.command.register");
    assert_eq!(state.command_trace[1].api, "roukaku.command.emit");
    assert_eq!(state.command_trace[1].name, "show.hero");
    assert_eq!(state.command_trace[2].api, "roukaku.command.enqueue");
    assert_eq!(state.command_trace[3].api, "roukaku.command.filter");
    assert_eq!(state.query_trace.len(), 5);
    assert_eq!(state.query_trace[0].api, "roukaku.query.text");
    assert_eq!(state.query_trace[0].target, "line.001");
    assert_eq!(state.query_trace[4].api, "roukaku.query.layout");
    assert_eq!(state.query_trace[4].target, "message");
    assert_eq!(state.trace_events.len(), 2);
    assert_eq!(state.trace_events[0].kind, "system.open");
    assert_eq!(state.trace_events[1].kind, "title.boot");
}

#[test]
fn luau_policy_blocks_unserializable_command_and_trace_payloads() {
    let mut policy = LuauPolicy::new().unwrap();
    let mut state = VnPolicyState::default();

    let err = policy
        .eval_bool(
            r#"
            roukaku.command.emit("bad", { callback = function() return true end })
            return true
            "#,
            &mut state,
        )
        .unwrap_err();

    assert_eq!(err.code(), "ROUKAKU_VN_LUAU_SNAPSHOT_UNSERIALIZABLE");
    assert!(state.command_trace.is_empty());

    let err = policy
        .eval_bool(
            r#"
            roukaku.trace.event("bad", { callback = function() return true end })
            return true
            "#,
            &mut state,
        )
        .unwrap_err();

    assert_eq!(err.code(), "ROUKAKU_VN_LUAU_SNAPSHOT_UNSERIALIZABLE");
    assert!(state.trace_events.is_empty());
}

#[test]
fn luau_policy_ignores_direct_table_writes_outside_capability_api() {
    let mut policy = LuauPolicy::new().unwrap();
    let mut state = VnPolicyState::default();

    let result = policy
        .eval_bool(
            r#"
            roukaku.var.cache = { project = { affinity = 99 } }
            return roukaku.var.get("project", "affinity") == 0
            "#,
            &mut state,
        )
        .unwrap();

    assert!(result);
    assert_eq!(state.var("project", "affinity"), None);
    assert!(state.mutation_trace.is_empty());
}

#[test]
fn luau_policy_records_serializable_snapshot_values() {
    let mut policy = LuauPolicy::new().unwrap();
    let mut state = VnPolicyState::default();

    let result = policy
        .eval_bool(
            r#"
            roukaku.snapshot.set("ui.save", { page = "save", slot = 3 })
            return roukaku.snapshot.get("ui.save").slot == 3
            "#,
            &mut state,
        )
        .unwrap();

    assert!(result);
    let snapshot = state.snapshot("ui.save").unwrap();
    assert!(matches!(snapshot, PolicySnapshotValue::Object(values)
        if values.get("page") == Some(&PolicySnapshotValue::String("save".to_string()))
            && values.get("slot") == Some(&PolicySnapshotValue::Integer(3))));
}

#[test]
fn luau_policy_blocks_unserializable_snapshot_values() {
    let mut policy = LuauPolicy::new().unwrap();
    let mut state = VnPolicyState::default();

    let err = policy
        .eval_bool(
            r#"
            roukaku.snapshot.set("bad", function() return 1 end)
            return true
            "#,
            &mut state,
        )
        .unwrap_err();

    assert_eq!(err.code(), "ROUKAKU_VN_LUAU_SNAPSHOT_UNSERIALIZABLE");
    assert!(state.snapshots.is_empty());
}
