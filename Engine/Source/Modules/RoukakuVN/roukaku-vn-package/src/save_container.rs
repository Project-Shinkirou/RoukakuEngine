use roukaku_core::SchemaMigrationRegistry;
use roukaku_package::{ContainerError, SectionPayload};
use roukaku_runtime::{read_runtime_save_section, RuntimeError, SaveBlob};
use roukaku_vn_core::VnRuntimeState;
use roukaku_vn_policy::VnPolicyState;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const VN_RUNTIME_STATE_SECTION_ID: &str = "vn.runtime_state";
pub const VN_POLICY_STATE_SECTION_ID: &str = "vn.policy_state";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct VnRuntimeStateSave {
    pub schema: String,
    pub state: VnRuntimeState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct VnPolicyStateSave {
    pub schema: String,
    pub state: VnPolicyState,
}

pub fn runtime_state_save_section(
    state: &VnRuntimeState,
) -> Result<SectionPayload, ContainerError> {
    tracing::debug!(
        event = "vn.save.runtime_state.encode",
        "RoukakuVN runtime state save section encoded"
    );
    let save = VnRuntimeStateSave {
        schema: "roukaku.vn.runtime_state_save.v2".to_string(),
        state: state.clone(),
    };
    SectionPayload::postcard(
        VN_RUNTIME_STATE_SECTION_ID,
        "roukaku.vn.runtime_state_save.v2",
        &save,
    )
}

pub fn policy_state_save_section(state: &VnPolicyState) -> Result<SectionPayload, ContainerError> {
    tracing::debug!(
        event = "vn.save.policy_state.encode",
        "RoukakuVN policy state save section encoded"
    );
    let save = VnPolicyStateSave {
        schema: "roukaku.vn.policy_state_save.v2".to_string(),
        state: state.clone(),
    };
    SectionPayload::postcard(
        VN_POLICY_STATE_SECTION_ID,
        "roukaku.vn.policy_state_save.v2",
        &save,
    )
}

pub fn read_runtime_save_vn_state(save: &SaveBlob) -> Result<VnRuntimeStateSave, RuntimeError> {
    read_runtime_save_section(
        save,
        VN_RUNTIME_STATE_SECTION_ID,
        &SchemaMigrationRegistry::default(),
    )
}

pub fn read_runtime_save_policy_state(save: &SaveBlob) -> Result<VnPolicyStateSave, RuntimeError> {
    read_runtime_save_section(
        save,
        VN_POLICY_STATE_SECTION_ID,
        &SchemaMigrationRegistry::default(),
    )
}
