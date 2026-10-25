use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    sync::{Arc, OnceLock},
};

use roukaku_core::{Diagnostic, DiagnosticSeverity, SourceRef, StableId, StableIdGenerator};

use schemars::JsonSchema;

use serde::{Deserialize, Serialize};

use tracing::debug;

use crate::{
    ActionExecutionClass, ActionInvocation, ActionRegistry, ActionResourceKey, ActionTrace,
    ActorId, ActorStore, AwaitToken, Blackboard, BlackboardValue, DelayedEventId,
    DeterministicActionContext, PresentationCommand, RuntimeError, RuntimeEvent, ScheduledEvent,
    actor::{ActorStoreAccess, ActorStoreDelta, ActorStoreOverlay},
    blackboard::{BlackboardAccess, BlackboardDelta, BlackboardOverlay},
};

mod state_definition;
pub use state_definition::GuardExpr;
use state_definition::MachineCandidate;
pub use state_definition::StateDefinition;
pub use state_definition::StateMachineDefinition;
pub use state_definition::StateMachineInstance;
pub use state_definition::StateMachineStore;
pub use state_definition::StateMachineValidationReport;
pub use state_definition::TransitionDefinition;
use state_definition::resource_is_declared;
pub use state_definition::validate_state_machine;
mod validate_observed_access;
use validate_observed_access::CompiledMachineDefinition;
use validate_observed_access::execute_machine_caught;
use validate_observed_access::execute_parallel_wave;
use validate_observed_access::guard_conflict_key;
mod state_machine_tick_output;
pub use state_machine_tick_output::StateMachineSnapshot;
pub use state_machine_tick_output::StateMachineTickOutput;
