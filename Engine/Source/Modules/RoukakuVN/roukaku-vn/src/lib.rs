//! Typed RoukakuVN product sessions over authored Luau story modules.

pub use roukaku_vn_package::{
    LuauStoryBundle, VnPlayerCommand, VnPresentationProviderManifest, VnRunConfig, VnWaitKind,
};

mod limits;
pub use limits::{VnHostError, VnHostLimits};
mod session;
pub use session::{
    NativeVnPropertySample, NativeVnStateView, NativeVnStepCommand, NativeVnStepInput,
    NativeVnStepOutput, VnSession, VnSessionConfig, VnStepComplexityMetrics,
};

pub use roukaku_vn_policy::{
    PolicyExecutionBudget, PolicyIoCapabilities, PolicyIoFailure, PolicyIoHost, PolicyIoState,
    PolicyIoStatus, PolicyReadText, VnPolicyState,
};
