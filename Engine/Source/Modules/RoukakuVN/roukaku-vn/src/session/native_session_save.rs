use super::*;
use roukaku_vn_package::authoring::{ParagraphLoadOutcome, ParagraphSave};

pub(super) const SESSION_SAVE_SCHEMA: &str = "roukaku.vn.session_save.v1";
const SESSION_SAVE_MAJOR: u16 = 1;

/// Paragraph-entry save: the live coroutine is never serialized. Load replays
/// the saved paragraph from its entry checkpoint, then reopens user overlays.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub(super) struct SessionSave {
    /// JSON-encoded `ParagraphSave`: authored state is dynamic Luau data.
    pub paragraph: Option<Vec<u8>>,
    pub locale: String,
    pub system: VnSystemState,
    pub variables: BTreeMap<String, BTreeMap<String, i64>>,
    pub backlog: Vec<BacklogEntry>,
    pub read_state: BTreeSet<String>,
    pub voice_replay: BTreeMap<String, VoiceReplayEntry>,
    pub route_coverage: BTreeSet<String>,
    pub overlays: Vec<VnSystemFrame>,
}

impl VnSession {
    fn session_save(&self) -> Result<SessionSave, VnError> {
        let paragraph = match self.state.cursor {
            Some(_) => {
                let save = self.authoring.save_paragraph().map_err(authoring_error)?;
                Some(serde_json::to_vec(&save).map_err(|error| VnError::message(error.to_string()))?)
            }
            None => None,
        };
        let overlays = self.state.system_stack[usize::from(self.menu_frame)..].to_vec();
        Ok(SessionSave {
            paragraph,
            locale: self.state.locale.clone(),
            system: self.state.system.clone(),
            variables: self.state.variables.clone(),
            backlog: self.state.backlog[..self.paragraph_backlog_start].to_vec(),
            read_state: self.state.read_state.clone(),
            voice_replay: self.state.voice_replay.clone(),
            route_coverage: self.state.route_coverage.clone(),
            overlays,
        })
    }

    pub fn save(&self) -> Result<SaveBlob, VnError> {
        if self.failed {
            return Err(VnError::diagnostic(
                "ROUKAKU_NATIVE_VN_SESSION_FAILED",
                "failed session cannot produce a save",
            ));
        }
        let save = self.session_save()?;
        let mut snapshot = self
            .engine
            .world()
            .snapshot()
            .map_err(|error| VnError::message(error.to_string()))?;
        let mut id_probe = snapshot.id_source.clone();
        let component_id = loop {
            let candidate = ComponentId(id_probe.next_id());
            if snapshot.actors.component(candidate).is_none() {
                break candidate;
            }
        };
        if !snapshot.actors.attach_component(ComponentRecord {
            component_id,
            actor_id: self.owner,
            payload: RuntimeComponentPayload::typed(
                SESSION_SAVE_SCHEMA,
                SchemaVersion::new(SESSION_SAVE_MAJOR, 0, 0),
                save,
            ),
        }) {
            return Err(VnError::diagnostic(
                "ROUKAKU_NATIVE_VN_SAVE_OWNER_MISSING",
                "VN save owner is missing from the Runtime snapshot",
            ));
        }
        roukaku_runtime::write_runtime_save(snapshot, SaveRequest::default())
            .map_err(|error| VnError::message(error.to_string()))
    }

    /// Validate a saved candidate without changing the live world or cancelling work.
    pub fn validate_save(&self, blob: &SaveBlob) -> Result<(), VnError> {
        let mut snapshot = roukaku_runtime::read_runtime_save(
            blob,
            &roukaku_core::SchemaMigrationRegistry::default(),
        )
        .map_err(|error| VnError::message(error.to_string()))?;
        snapshot
            .awaits
            .validate()
            .map_err(|error| VnError::message(format!("{}: {}", error.code, error.message)))?;
        self.prepare_candidate(&mut snapshot)
            .map(|_| ())
            .map_err(|error| VnError::message(error.to_string()))
    }

    fn prepare_candidate(&self, snapshot: &mut RuntimeSnapshot) -> Result<SessionSave, RuntimeError> {
        super::policy_io::prepare_policy(snapshot, self.owner, self.policy_component)?;
        let policy: super::policy_io::SessionPolicyState = snapshot
            .actors
            .component(self.policy_component)
            .ok_or_else(|| RuntimeError::message("missing policy component"))?
            .payload
            .decode()?;
        super::policy_actions::validate_saved_action(&policy, &self.policy_host.actions)?;
        prepare_save(
            snapshot,
            self.engine.world().package_handle(),
            self.owner,
            self.engine.seed(),
        )
    }

    pub fn restore(&mut self, blob: SaveBlob) -> Result<roukaku_runtime::LoadReport, VnError> {
        let package = self.engine.world().package_handle().cloned();
        let owner = self.owner;
        let policy_component = self.policy_component;
        let seed = self.engine.seed();
        let actions = self.policy_host.actions.clone();
        let (_, (save, step, seed)) = self
            .engine
            .world_mut()
            .load_with_validation(
                blob,
                &roukaku_core::SchemaMigrationRegistry::default(),
                |snapshot| {
                    super::policy_io::prepare_policy(snapshot, owner, policy_component)?;
                    let policy: super::policy_io::SessionPolicyState = snapshot
                        .actors
                        .component(policy_component)
                        .ok_or_else(|| RuntimeError::message("missing policy component"))?
                        .payload
                        .decode()?;
                    super::policy_actions::validate_saved_action(&policy, &actions)?;
                    let save = prepare_save(snapshot, package.as_ref(), owner, seed)?;
                    Ok((save, snapshot.step, snapshot.config.seed))
                },
            )
            .map_err(|error| VnError::message(error.to_string()))?;
        self.policy_host.stop();
        self.failed = true;
        self.replay(save)?;
        self.failed = false;
        Ok(roukaku_runtime::LoadReport { step, seed })
    }

    fn replay(&mut self, save: SessionSave) -> Result<(), VnError> {
        self.authoring = super::native_open::authoring_runtime(&self.story, &self.host)?;
        self.host.take_events();
        let mut state = super::native_open::initial_state(&self.id, &VnRunConfig {
            profile: self.state.profile.clone(),
            locale: save.locale,
        });
        state.system = save.system;
        state.variables = save.variables;
        state.backlog = save.backlog;
        state.read_state = save.read_state;
        state.voice_replay = save.voice_replay;
        state.route_coverage = save.route_coverage;
        self.state = state;
        self.wait = None;
        self.menu_frame = false;
        self.last_choice = None;
        self.paragraph_backlog_start = self.state.backlog.len();
        self.pending = Default::default();
        self.step_complexity = None;
        let Some(paragraph) = save.paragraph else {
            return Ok(());
        };
        let paragraph: ParagraphSave = serde_json::from_slice(&paragraph).map_err(|error| {
            VnError::diagnostic("ROUKAKU_VN_SAVE_FORMAT_UNSUPPORTED", error.to_string())
        })?;
        let outcome = self.authoring.load_paragraph(paragraph);
        if let Ok(ParagraphLoadOutcome::RevisionMismatchRestarted { diagnostic }) = &outcome {
            tracing::warn!(event = "vn.save.revision_mismatch", %diagnostic);
        }
        let status = outcome.map(|_| self.authoring.status());
        self.after_story(status)?;
        if let Some(action) = self.policy_state()?.action
            && self
                .wait
                .as_ref()
                .is_none_or(|wait| wait.action != format!("action:{}", action.source_id))
        {
            return Err(VnError::diagnostic(
                "ROUKAKU_VN_POLICY_ACTION_RESTORE",
                "saved action continuation does not match the replayed story wait",
            ));
        }
        if let Some((first, rest)) = save.overlays.split_first() {
            self.state.system_stack.push(VnSystemFrame {
                return_wait: self.state.pending_wait.take(),
                return_choice: self.state.pending_choice.take(),
                ..first.clone()
            });
            self.state.system_stack.extend(rest.iter().cloned());
            self.state.pending_wait = Some(VnWaitState::new(
                VnWaitKind::SystemPage,
                "system",
                format!("system.{}", self.state.system_stack.len()),
            ));
        }
        Ok(())
    }
}

fn prepare_save(
    snapshot: &mut RuntimeSnapshot,
    package: Option<&PackageHandle>,
    owner: ActorId,
    expected_seed: u64,
) -> Result<SessionSave, RuntimeError> {
    if snapshot.machines != roukaku_runtime::StateMachineStore::default() {
        return Err(RuntimeError::message(
            "ROUKAKU_NATIVE_VN_RESTORE_LEGACY_MACHINE: NativeVN no longer executes saved forwarding state machines",
        ));
    }
    if snapshot.config.seed != expected_seed {
        return Err(RuntimeError::message(
            "ROUKAKU_NATIVE_VN_RESTORE_SEED: save belongs to a different session seed",
        ));
    }
    if snapshot.package.as_ref() != package {
        return Err(RuntimeError::message(
            "ROUKAKU_NATIVE_VN_RESTORE_PACKAGE: save belongs to a different package",
        ));
    }
    if !snapshot
        .actors
        .component_ids_for_actor_schema(owner, &VN_RUNTIME_STATE_SCHEMA.to_string())
        .is_empty()
    {
        return Err(RuntimeError::message(
            "ROUKAKU_VN_SAVE_FORMAT_UNSUPPORTED: legacy execution-state saves are rejected",
        ));
    }
    let candidates = snapshot
        .actors
        .component_ids_for_actor_schema(owner, &SESSION_SAVE_SCHEMA.to_string());
    let [component_id] = candidates.as_slice() else {
        return Err(RuntimeError::message(
            "ROUKAKU_NATIVE_VN_RESTORE_STATE_SET: exactly one paragraph save is required",
        ));
    };
    let component = snapshot.actors.component(*component_id).ok_or_else(|| {
        RuntimeError::message("ROUKAKU_NATIVE_VN_RESTORE_STATE_SET: paragraph save is missing")
    })?;
    if component.payload.version() != SchemaVersion::new(SESSION_SAVE_MAJOR, 0, 0) {
        return Err(RuntimeError::message(
            "ROUKAKU_NATIVE_VN_RESTORE_STATE_VERSION: unsupported paragraph save version",
        ));
    }
    let save: SessionSave = component.payload.decode()?;
    snapshot.actors.detach_component(*component_id);
    Ok(save)
}
