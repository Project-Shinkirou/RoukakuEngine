use roukaku_vn_script::{compile_roukaku_project, RoukakuSource};

#[test]
fn grammar_negative_cases_block_compile() {
    let cases = [
        (
            "unterminated_quote.roukaku",
            r#"
story main #@id story.main
state prologue #@id state.prologue
  scene room #@id scene.room
    text key:"line.open #@id line.open
"#,
            "ROUKAKU_VN_PARSE_QUOTE",
        ),
        (
            "missing_arrow_target.roukaku",
            r#"
story main #@id story.main
state prologue #@id state.prologue
  scene room #@id scene.room
    choice key:where #@id choice.where
      option key:go -> #@id option.go
"#,
            "ROUKAKU_VN_PARSE_ARROW",
        ),
        (
            "tab_indent.roukaku",
            "story main #@id story.main\n\tstate prologue #@id state.prologue\n",
            "ROUKAKU_VN_PARSE_INDENT",
        ),
        (
            "odd_indent.roukaku",
            "story main #@id story.main\n state prologue #@id state.prologue\n",
            "ROUKAKU_VN_PARSE_INDENT",
        ),
        (
            "duplicate_attr.roukaku",
            r#"
story main #@id story.main
state prologue #@id state.prologue
  scene room #@id scene.room
    text key:line.one key:line.two #@id line.dup_attr
"#,
            "ROUKAKU_VN_ATTR_DUPLICATE",
        ),
        (
            "empty_source_id.roukaku",
            r#"
story main #@id
"#,
            "ROUKAKU_VN_SOURCE_ID",
        ),
        (
            "option_without_choice.roukaku",
            r#"
story main #@id story.main
state prologue #@id state.prologue
  scene room #@id scene.room
    option key:go -> state.prologue #@id option.orphan
"#,
            "ROUKAKU_VN_OPTION_CONTEXT",
        ),
        (
            "text_missing_key.roukaku",
            r#"
story main #@id story.main
state prologue #@id state.prologue
  scene room #@id scene.room
    text speaker:hero #@id line.no_key
"#,
            "ROUKAKU_VN_ATTR_MISSING",
        ),
        (
            "unknown_system_page.roukaku",
            r#"
story system.bad #@id story.system.bad
  scene menu #@id scene.system.bad
    system_page kind:not_real policy:roukaku.policy.standard #@id page.bad
"#,
            "ROUKAKU_VN_SYSTEM_PAGE_UNKNOWN",
        ),
    ];

    for (path, source, expected) in cases {
        let err = compile_roukaku_project([RoukakuSource::story(path, source)], Default::default())
            .unwrap_err();
        assert_eq!(err.code(), expected, "{path}");
    }
}

#[test]
fn duplicate_explicit_source_ids_block_compile() {
    let err = compile_roukaku_project(
        [RoukakuSource::story(
            "duplicate.roukaku",
            r#"
story main #@id story.main
state prologue #@id state.prologue
  scene room #@id scene.room
    text key:line.one #@id line.dup
    text key:line.two #@id line.dup
"#,
        )],
        Default::default(),
    )
    .unwrap_err();

    assert_eq!(err.code(), "ROUKAKU_VN_DUPLICATE_ID");
}

#[test]
fn undefined_jump_targets_block_compile() {
    let err = compile_roukaku_project(
        [RoukakuSource::story(
            "missing_jump.roukaku",
            r#"
story main #@id story.main
state prologue #@id state.prologue
  scene room #@id scene.room
    text key:line.one #@id line.one
    jump missing_state #@id jump.missing
"#,
        )],
        Default::default(),
    )
    .unwrap_err();

    assert_eq!(err.code(), "ROUKAKU_VN_TARGET_UNDEFINED");
}

#[test]
fn undefined_choice_targets_block_compile() {
    let err = compile_roukaku_project(
        [RoukakuSource::story(
            "missing_choice.roukaku",
            r#"
story main #@id story.main
state prologue #@id state.prologue
  scene room #@id scene.room
    choice key:where #@id choice.where
      option key:go -> missing_state #@id option.missing
"#,
        )],
        Default::default(),
    )
    .unwrap_err();

    assert_eq!(err.code(), "ROUKAKU_VN_TARGET_UNDEFINED");
}

#[test]
fn unreachable_main_states_block_compile() {
    let err = compile_roukaku_project(
        [RoukakuSource::story(
            "unreachable.roukaku",
            r#"
story main #@id story.main
state prologue #@id state.prologue
  scene room #@id scene.room
    text key:line.one #@id line.one
    jump ending.good #@id jump.good

state orphan #@id state.orphan
  scene orphan #@id scene.orphan
    text key:line.orphan #@id line.orphan
"#,
        )],
        Default::default(),
    )
    .unwrap_err();

    assert_eq!(err.code(), "ROUKAKU_VN_UNREACHABLE_STATE");
}

#[test]
fn invalid_variable_scope_blocks_compile() {
    let err = compile_roukaku_project(
        [RoukakuSource::story(
            "bad_scope.roukaku",
            r#"
story main #@id story.main
state prologue #@id state.prologue
  scene room #@id scene.room
    mutate unsafe.affinity += 1 #@id var.bad
"#,
        )],
        Default::default(),
    )
    .unwrap_err();

    assert_eq!(err.code(), "ROUKAKU_VN_VARIABLE_SCOPE");
}

#[test]
fn duplicate_text_keys_block_compile() {
    let err = compile_roukaku_project(
        [RoukakuSource::story(
            "duplicate_key.roukaku",
            r#"
story main #@id story.main
state prologue #@id state.prologue
  scene room #@id scene.room
    text key:line.same #@id line.one
    text key:line.same #@id line.two
"#,
        )],
        Default::default(),
    )
    .unwrap_err();

    assert_eq!(err.code(), "ROUKAKU_VN_TEXT_KEY_DUPLICATE");
}

#[test]
fn invalid_mutation_number_blocks_compile() {
    let err = compile_roukaku_project(
        [RoukakuSource::story(
            "bad_number.roukaku",
            r#"
story main #@id story.main
state prologue #@id state.prologue
  scene room #@id scene.room
    mutate project.affinity += many #@id var.bad_number
"#,
        )],
        Default::default(),
    )
    .unwrap_err();

    assert_eq!(err.code(), "ROUKAKU_VN_MUTATE_VALUE");
}

#[test]
fn presentation_command_outside_scene_blocks_compile() {
    let err = compile_roukaku_project(
        [RoukakuSource::story(
            "outside_scene.roukaku",
            r#"
story main #@id story.main
voice asset:voice.hero.0001 #@id voice.outside
"#,
        )],
        Default::default(),
    )
    .unwrap_err();

    assert_eq!(err.code(), "ROUKAKU_VN_SCENE_CONTEXT");
}

#[test]
fn option_cannot_bind_across_an_intervening_command() {
    let err = compile_roukaku_project(
        [RoukakuSource::story(
            "detached_option.roukaku",
            r#"
story main #@id story.main
state prologue #@id state.prologue
  scene room #@id scene.room
    choice key:route.pick #@id choice.route
    text key:route.after #@id line.after
      option key:route.left -> left #@id option.left
"#,
        )],
        Default::default(),
    )
    .unwrap_err();

    assert_eq!(err.code(), "ROUKAKU_VN_OPTION_CONTEXT");
}

#[test]
fn structural_indentation_is_part_of_the_canonical_ast() {
    let command_error = compile_roukaku_project(
        [RoukakuSource::story(
            "bad_command_indent.roukaku",
            r#"
story main #@id story.main
state prologue #@id state.prologue
  scene room #@id scene.room
  text key:route.bad #@id line.bad
"#,
        )],
        Default::default(),
    )
    .unwrap_err();
    assert_eq!(command_error.code(), "ROUKAKU_VN_STRUCTURE_INDENT");

    let option_error = compile_roukaku_project(
        [RoukakuSource::story(
            "bad_option_indent.roukaku",
            r#"
story main #@id story.main
state prologue #@id state.prologue
  scene room #@id scene.room
    choice key:route.pick #@id choice.route
    option key:route.left -> left #@id option.left
"#,
        )],
        Default::default(),
    )
    .unwrap_err();
    assert_eq!(option_error.code(), "ROUKAKU_VN_OPTION_CONTEXT");
}

#[test]
fn system_story_definitions_are_authoritative_over_gameplay_page_triggers() {
    let compiled = compile_roukaku_project(
        [RoukakuSource::story(
            "system_authority.roukaku",
            r#"
story main #@id story.main
state checkpoint #@id state.checkpoint
  scene checkpoint #@id scene.checkpoint
    system_page kind:save #@id gameplay.save

story system #@id story.system
state save #@id state.system.save
  scene save #@id scene.system.save
    system_page kind:save policy:roukaku.policy.standard #@id page.save
"#,
        )],
        Default::default(),
    )
    .unwrap();

    let entry = compiled
        .story
        .system_story_manifest
        .entries
        .get(&roukaku_vn_script::SystemPageKind::Save)
        .expect("save system story entry");
    assert_eq!(entry.story_id, "story.system");
    assert_eq!(entry.state_id, "state.system.save");
    assert_eq!(entry.source_id, "page.save");
}

#[test]
fn duplicate_system_story_page_definitions_are_blocking() {
    let error = compile_roukaku_project(
        [RoukakuSource::story(
            "duplicate_system_page.roukaku",
            r#"
story system #@id story.system
state save_one #@id state.system.save_one
  scene save_one #@id scene.system.save_one
    system_page kind:save #@id page.save.one
state save_two #@id state.system.save_two
  scene save_two #@id scene.system.save_two
    system_page kind:save #@id page.save.two
"#,
        )],
        Default::default(),
    )
    .unwrap_err();
    assert_eq!(error.code(), "ROUKAKU_VN_SYSTEM_PAGE_DUPLICATE");
}
