use super::*;

/// Native session construction; package identity is optional for embedding.
#[derive(Debug, Clone)]
pub struct VnSessionConfig {
    pub target_id: String,
    pub seed: u64,
    pub package: Option<PackageHandle>,
    pub integrity_mode: TickIntegrityMode,
    pub worker_count: usize,
}

/// Load the default VN modules and every authored story module into a fresh VM.
pub(super) fn authoring_runtime(
    story: &LuauStoryBundle,
    host: &Rc<super::story_host::StoryHost>,
) -> Result<LuauAuthoringRuntime, VnError> {
    let host: Rc<dyn roukaku_vn_package::authoring::AuthoringHost> = host.clone();
    let mut runtime = LuauAuthoringRuntime::new(
        roukaku_vn_policy::PolicyExecutionBudget::default(),
        roukaku_runtime::TaskScope::new(),
        host,
    )
    .map_err(authoring_error)?;
    for (module, source) in &story.modules {
        runtime
            .replace_module(module, &format!("@story/{module}.luau"), source)
            .map_err(authoring_error)?;
    }
    Ok(runtime)
}

pub(super) fn initial_state(session_id: &str, config: &VnRunConfig) -> VnRuntimeState {
    VnRuntimeState {
        schema: VN_RUNTIME_STATE_SCHEMA.to_string(),
        revision: 0,
        instance_id: session_id.to_string(),
        profile: config.profile.clone(),
        locale: config.locale.clone(),
        cursor: None,
        call_stack: Vec::new(),
        system_stack: Vec::new(),
        system: VnSystemState::default(),
        pending_choice: None,
        variables: BTreeMap::new(),
        backlog: Vec::new(),
        read_state: BTreeSet::new(),
        voice_replay: BTreeMap::new(),
        route_coverage: BTreeSet::new(),
        route_flags: BTreeMap::new(),
        wait_sequence: 0,
        pending_wait: None,
    }
}

impl VnSession {
    pub fn new(
        story: Arc<LuauStoryBundle>,
        config: VnRunConfig,
        options: VnSessionConfig,
    ) -> Result<Self, VnError> {
        story
            .validate()
            .map_err(|error| VnError::message(error.to_string()))?;
        tracing::info!(
            event = "vn.provider.session.open.start",
            target_id = %options.target_id,
            seed = options.seed,
            "RoukakuVN runtime session open started"
        );
        let session_id = format!("native_vn:{}:{}", options.target_id, options.seed);
        let host = Rc::new(super::story_host::StoryHost::default());
        let authoring = authoring_runtime(&story, &host)?;
        // Product package identity and execution settings stay at the VN boundary.
        let mut world = RuntimeWorld::create_with_integrity(
            RuntimeConfig {
                seed: options.seed,
                required_slots: Vec::new(),
            },
            options.integrity_mode,
        )
        .map_err(|error| VnError::message(error.to_string()))?;
        if let Some(package) = options.package {
            world = world
                .with_package(package)
                .map_err(|error| VnError::message(error.to_string()))?;
        }
        world
            .set_machine_worker_count(options.worker_count)
            .map_err(|error| VnError::message(error.to_string()))?;
        let owner = world
            .create_actor("roukaku.vn.runtime", vec!["gameplay_runtime".to_string()])
            .map_err(|error| VnError::message(error.to_string()))?;
        let policy_component = world
            .attach_component(
                owner,
                super::policy_io::POLICY_STATE_SCHEMA,
                &super::policy_io::SessionPolicyState::default(),
            )
            .map_err(|err| VnError::message(err.to_string()))?;
        let state = initial_state(&session_id, &config);
        Ok(Self {
            id: session_id,
            policy_component,
            policy_host: Default::default(),
            engine: EngineSession::from_world(world),
            owner,
            story,
            authoring,
            host,
            state,
            wait: None,
            menu_frame: false,
            paragraph_backlog_start: 0,
            last_choice: None,
            pending: Default::default(),
            failed: false,
            step_complexity: None,
        })
    }
}
