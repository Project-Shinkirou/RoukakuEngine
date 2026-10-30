use super::*;

/// In-process NativeVN work.
#[derive(Debug, Clone, PartialEq)]
pub enum NativeVnStepCommand {
    LaunchDefault,
    PollPolicy,
    Execute(VnPlayerCommand),
}

#[derive(Debug, Clone, PartialEq)]
pub struct NativeVnStepInput {
    pub timing: TickInput,
    pub mode: roukaku_runtime::TickMode,
    pub command: NativeVnStepCommand,
}

/// One sample of a Luau timeline segment property track.
#[derive(Debug, Clone, PartialEq)]
pub struct NativeVnPropertySample {
    pub target: String,
    pub property: String,
    pub value: f64,
}

/// Story output accumulated since the last step result, including entry replay.
#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct PendingOutput {
    pub presentations: Vec<PresentationCommand>,
    pub audio: Vec<VnAudioCommand>,
    pub timeline: Vec<VnTimelineTask>,
    pub properties: Vec<NativeVnPropertySample>,
    pub coverage_reached: BTreeSet<String>,
}

impl PendingOutput {
    pub(super) fn present(&mut self, node: &str, command: StageCommand) {
        match &command {
            StageCommand::Audio(cue) => self.audio.push(VnAudioCommand {
                command_id: node.to_owned(),
                cue: cue.clone(),
            }),
            StageCommand::Timeline(timeline) => self.timeline.push(VnTimelineTask {
                command_id: match timeline {
                    TimelineCommand::Start(spec) => spec.id.clone(),
                    TimelineCommand::Cancel { id, .. } => id.clone(),
                },
                command: timeline.clone(),
            }),
            _ => {}
        }
        self.presentations.push(PresentationCommand::Stage(command));
    }
}

impl VnSession {
    pub fn step(&mut self, input: NativeVnStepInput) -> Result<NativeVnStepOutput, VnError> {
        if self.failed {
            return Err(VnError::diagnostic(
                "ROUKAKU_NATIVE_VN_SESSION_FAILED",
                "failed session requires a successful restore before another step",
            ));
        }
        // Set before any fallible execution, including panic unwinding.
        self.failed = true;
        let mut failure_scope = StepFailureScope(Some(self.engine.world().task_scope()));
        tracing::trace!(
            event = "vn.provider.session.step",
            fixed_step = input.timing.fixed_step,
            "RoukakuVN runtime session step started"
        );
        self.engine
            .validate_step(input.timing, input.mode)
            .map_err(|error| VnError::message(error.to_string()))?;
        let previous_backlog_count = self.state.backlog.len();
        match input.command {
            NativeVnStepCommand::PollPolicy => {
                if let Some((node, writes)) = self.poll_policy_action()? {
                    self.complete_policy_action(&node, writes)?;
                }
            }
            NativeVnStepCommand::Execute(command) => {
                if matches!(command, VnPlayerCommand::CompleteWait { .. })
                    && self.policy_action_pending()?
                {
                    return Err(VnError::diagnostic(
                        "ROUKAKU_VN_POLICY_ACTION_EXTERNAL_COMPLETION",
                        "input cannot complete a policy action",
                    ));
                }
                self.apply_command(command)?;
            }
            NativeVnStepCommand::LaunchDefault => {
                let entry = self.story.entry.clone();
                self.launch(entry)?;
            }
        }
        let mut ingress = Vec::new();
        let policy_completion = self.take_policy_completion();
        if let Some(completion) = &policy_completion {
            ingress.push(OrderedTickIngress {
                sequence: 1,
                payload: TickIngress::AwaitCompletion(completion.handle.complete(
                    1,
                    input.timing.fixed_step,
                    EventPayload::new("vn.policy.io.completed"),
                )),
            });
        }
        let tick = self
            .engine
            .tick(TickRequest {
                timing: input.timing,
                mode: input.mode,
                ingress,
            })
            .map_err(|err| VnError::message(err.to_string()))?;
        if let Some(diagnostic) = tick.diagnostics.first() {
            return Err(VnError::diagnostic(
                diagnostic.code.clone(),
                diagnostic.message.clone(),
            ));
        }
        if let Some(completion) = policy_completion {
            self.commit_policy_completion(completion)?;
        }
        let output = std::mem::take(&mut self.pending);
        for id in &output.coverage_reached {
            self.engine
                .world_mut()
                .emit_event(
                    roukaku_runtime::EventSource::Runtime,
                    EventPayload {
                        kind: "vn.route.reached".into(),
                        data: [(
                            "id".into(),
                            roukaku_runtime::BlackboardValue::String(id.clone()),
                        )]
                        .into_iter()
                        .collect(),
                    },
                )
                .map_err(|error| VnError::message(error.to_string()))?;
        }
        self.state.revision = self.state.revision.saturating_add(1);
        self.step_complexity = Some(VnStepComplexityMetrics {
            schema: "roukaku.vn.step_complexity_metrics.v4".to_string(),
            previous_backlog_count,
            appended_backlog_entries: self
                .state
                .backlog
                .len()
                .saturating_sub(previous_backlog_count),
        });
        failure_scope.0 = None;
        self.failed = false;
        Ok(NativeVnStepOutput {
            fixed_step: input.timing.fixed_step,
            vn_state: NativeVnStateView::project(&self.state),
            presentations: output.presentations,
            audio: output.audio,
            timeline: output.timeline,
            properties: output.properties,
            coverage_reached: output.coverage_reached.into_iter().collect(),
        })
    }

    /// Advance Luau timeline segments by presentation time and return their samples.
    pub fn advance_presentation(
        &mut self,
        seconds: f64,
    ) -> Result<Vec<NativeVnPropertySample>, VnError> {
        if self.failed {
            return Err(VnError::diagnostic(
                "ROUKAKU_NATIVE_VN_SESSION_FAILED",
                "failed session cannot advance presentation",
            ));
        }
        if let Err(error) = self.authoring.advance_presentation(seconds) {
            self.failed = true;
            self.engine.world().task_scope().cancel();
            return Err(authoring_error(error));
        }
        self.apply_story_events()?;
        Ok(std::mem::take(&mut self.pending.properties))
    }
}

// Cancels outstanding work on both Result errors and panic unwinding.
struct StepFailureScope(Option<roukaku_runtime::TaskScope>);

impl Drop for StepFailureScope {
    fn drop(&mut self) {
        if let Some(scope) = &self.0 {
            scope.cancel();
        }
    }
}

/// Owned in-process presentation output with a typed display projection.
#[derive(Debug, Clone, PartialEq)]
pub struct NativeVnStepOutput {
    pub fixed_step: u64,
    pub vn_state: NativeVnStateView,
    pub presentations: Vec<PresentationCommand>,
    pub audio: Vec<VnAudioCommand>,
    pub timeline: Vec<VnTimelineTask>,
    pub properties: Vec<NativeVnPropertySample>,
    pub coverage_reached: Vec<String>,
}
