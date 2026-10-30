use super::*;

pub(super) fn story(source: &str) -> Arc<LuauStoryBundle> {
    Arc::new(
        LuauStoryBundle::new(
            ParagraphId {
                module: "story.main".into(),
                entry: "start".into(),
            },
            BTreeMap::from([("story.main".into(), source.into())]),
        )
        .unwrap(),
    )
}

pub(super) fn config(target: &str) -> VnSessionConfig {
    VnSessionConfig {
        target_id: target.into(),
        seed: 23,
        package: None,
        integrity_mode: TickIntegrityMode::Shipping,
        worker_count: 1,
    }
}

pub(super) fn session(target: &str, source: &str) -> VnSession {
    VnSession::new(story(source), VnRunConfig::classic("en"), config(target)).unwrap()
}

pub(super) fn input(
    fixed_step: u64,
    command: NativeVnStepCommand,
    mode: roukaku_runtime::TickMode,
) -> NativeVnStepInput {
    NativeVnStepInput {
        timing: TickInput {
            fixed_step,
            delta_ns: 16_666_667,
            seed: 23,
        },
        mode,
        command,
    }
}

pub(super) fn launch(session: &mut VnSession) -> NativeVnStepOutput {
    session
        .step(input(
            1,
            NativeVnStepCommand::LaunchDefault,
            roukaku_runtime::TickMode::Live,
        ))
        .unwrap()
}

pub(super) fn execute(
    session: &mut VnSession,
    fixed_step: u64,
    command: VnPlayerCommand,
) -> Result<NativeVnStepOutput, VnError> {
    session.step(input(
        fixed_step,
        NativeVnStepCommand::Execute(command),
        roukaku_runtime::TickMode::Live,
    ))
}
