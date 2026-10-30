use super::*;

/// An owned NativeVN session driving authored Luau story modules. Dropping it
/// cancels its world-scoped tasks and the story coroutine.
pub struct VnSession {
    pub(super) id: String,
    pub(super) policy_component: ComponentId,
    pub(super) policy_host: super::policy_io::SessionPolicyHost,
    pub(super) engine: EngineSession,
    pub(super) owner: ActorId,
    pub(super) story: Arc<LuauStoryBundle>,
    pub(super) authoring: LuauAuthoringRuntime,
    pub(super) host: Rc<super::story_host::StoryHost>,
    /// Display projection of story progress consumed by the VN UI model.
    pub(super) state: VnRuntimeState,
    /// Live story wait; never persisted.
    pub(super) wait: Option<InputWait>,
    /// The bottom system frame belongs to a story `roukaku.vn.menu` wait.
    pub(super) menu_frame: bool,
    /// Backlog length when the live paragraph started; entry replay rebuilds the rest.
    pub(super) paragraph_backlog_start: usize,
    pub(super) last_choice: Option<PendingChoice>,
    pub(super) pending: super::native_step::PendingOutput,
    pub(super) failed: bool,
    pub(super) step_complexity: Option<VnStepComplexityMetrics>,
}

impl Drop for VnSession {
    fn drop(&mut self) {
        self.policy_host.stop();
        self.engine.world().task_scope().cancel();
    }
}

impl VnSession {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn story(&self) -> &LuauStoryBundle {
        &self.story
    }

    pub fn step_complexity(&self) -> Option<&VnStepComplexityMetrics> {
        self.step_complexity.as_ref()
    }

    /// Bind the shared text layout used by `rk.measure_text`.
    pub fn bind_text_layout(&mut self, provider: Arc<dyn roukaku_text::TextLayoutProvider>) {
        self.host.set_text_layout(provider);
    }

    /// Profile-declared system actions offered by story menus.
    pub fn bind_system_actions(&mut self, actions: BTreeSet<String>) {
        self.host.set_system_actions(actions);
    }

    /// Fence identities with callbacks still registered in this session.
    pub fn callback_fences(&self) -> roukaku_vn_policy::authoring::CallbackFences<'_> {
        self.authoring.callback_fences()
    }
    /// Consume the owned session and cancel all world-scoped work before returning.
    pub fn close(self) {
        drop(self);
    }
}
