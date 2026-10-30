use roukaku_core::SchemaVersion;
use roukaku_runtime::RuntimeWorld;
use roukaku_runtime::{
    ActorId, ComponentId, ComponentRecord, EngineSession, EventPayload, OrderedTickIngress,
    PackageHandle, RuntimeComponentPayload, RuntimeConfig, RuntimeError, RuntimeSnapshot,
    SaveBlob, SaveRequest, TickIngress, TickInput, TickIntegrityMode, TickRequest,
};
use roukaku_vn_package::authoring::{
    InputWait, LuauAuthoringRuntime, NodeContent, NodeRecord, ParagraphId, StoryStatus,
};
use roukaku_vn_package::{
    AudioCue, BacklogEntry, BacklogLayoutMetadata, ChoiceOption, ExtensionPresentationCommand,
    LuauStoryBundle, PendingChoice, PresentationCommand, ReadingMode, StageCommand,
    SystemPageKind, SystemUnlockKind, TimelineCommand, VN_RUNTIME_STATE_SCHEMA, VnAudioBus,
    VnAudioCommand, VnAudioSync, VnCommandCursor, VnError, VnPlayerCommand, VnRunConfig,
    VnRuntimeState, VnSystemFrame, VnSystemState, VnTimelineTask, VnWaitKind, VnWaitState,
    VoiceReplayEntry,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    rc::Rc,
    sync::Arc,
};
mod native_open;
mod native_session;
mod native_session_save;
mod native_session_step;
mod native_step;
mod native_view;
mod policy_actions;
mod policy_io;
mod story_host;
pub use native_open::VnSessionConfig;
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;
pub use native_session::VnSession;
pub use native_step::{
    NativeVnPropertySample, NativeVnStepCommand, NativeVnStepInput, NativeVnStepOutput,
};
pub use native_view::NativeVnStateView;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct VnStepComplexityMetrics {
    pub schema: String,
    pub previous_backlog_count: usize,
    pub appended_backlog_entries: usize,
}

/// Authoring errors carry their stable diagnostic code as the message prefix.
fn authoring_error(error: mlua::Error) -> VnError {
    VnError::diagnostic("ROUKAKU_VN_STORY_FAILED", error.to_string())
}

/// Stable display identity of a Luau paragraph, also used as its route node.
pub(crate) fn paragraph_route_id(paragraph: &ParagraphId) -> String {
    format!("{}/{}", paragraph.module, paragraph.entry)
}

fn parse_route_id(id: &str) -> Option<ParagraphId> {
    let (module, entry) = id.split_once('/')?;
    (!module.is_empty() && !entry.is_empty()).then(|| ParagraphId {
        module: module.to_owned(),
        entry: entry.to_owned(),
    })
}

use roukaku_vn_policy::VnPolicyState;
