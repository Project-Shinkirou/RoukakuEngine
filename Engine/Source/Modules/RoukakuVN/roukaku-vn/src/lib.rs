//! Typed RoukakuVN product sessions and authoring entry points.

pub use roukaku_vn_core::{VnPlayerCommand, VnRunConfig, VnRuntime, VnWaitKind};
pub use roukaku_vn_presentation::{StageModel, VnPresentationProviderManifest};
pub use roukaku_vn_script::{
    compile_roukaku_project, format_roukaku_source, RoukakuSource, CompileRoukakuProjectOptions,
    FormatOptions, SystemStoryManifest, SystemStoryValidationStatus, VnSystemUiProfileManifest,
};

mod limits;
pub use limits::{VnHostError, VnHostLimits};
mod session;
pub use session::{
    NativeVnStateView, NativeVnStepCommand, NativeVnStepInput, NativeVnStepOutput, VnSession,
    VnSessionConfig,
};

pub use roukaku_vn_policy::{
    PolicyExecutionBudget, PolicyIoCapabilities, PolicyIoFailure, PolicyIoHost, PolicyIoState,
    PolicyIoStatus, PolicyReadText, VnPolicyState,
};
