use std::collections::{BTreeMap, BTreeSet};

use roukaku_core::{Diagnostic, DiagnosticSeverity, Hash128, SourceRef};

use crate::{
    lower::{lower_sources_from_cst, ParsedLine},
    stage_compile::{compile_extension_command, compile_stage_command},
    BranchOp, ChoiceOption, CommandManifest, CommandManifestEntry, CommandProvider,
    CommandRegistry, CommandSourceMap, CompiledCommand, CompiledStory, CompiledVnProject,
    ExtensionCommandDescriptor, MutationOp, PresentationCommand, RoukakuSource, RoukakuSourceRole,
    RouteEdge, RouteGraph, RouteNode, Scene, State, Story, StoryManifest, StoryManifestEntry,
    SystemActionEffect, SystemActionProgram, SystemPageKind, SystemStoryManifest,
    VariableCondition, VariableManifest, VariableScopeManifest, VnError,
};

mod semantic_pass;
pub use semantic_pass::compile_roukaku_project;
use semantic_pass::trace_pass;
use semantic_pass::CompileBuilder;
pub use semantic_pass::CompileRoukakuProjectOptions;
pub use semantic_pass::SemanticPass;
pub use semantic_pass::SEMANTIC_PASS_ORDER;
mod validate_structure;

mod push_system_action;

mod resolved_edges_from;
use resolved_edges_from::command_manifest_identity;
use resolved_edges_from::duplicate_id_diagnostic;
use resolved_edges_from::is_allowed_variable_scope;
use resolved_edges_from::is_terminal_target;
use resolved_edges_from::parse_variable_condition;
use resolved_edges_from::required_attr;
pub use resolved_edges_from::resolve_target;
use resolved_edges_from::semantic_story;
use resolved_edges_from::validate_system_action_symbol;
use resolved_edges_from::TargetKind;
