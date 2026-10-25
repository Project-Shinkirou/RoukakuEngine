use std::{collections::BTreeMap, sync::Arc};

use roukaku_text::{TextLayoutProvider, TextLayoutResult};
use serde::{Deserialize, Serialize};

use serde_json::Value;

/// Stable source identity, independent of a coroutine or module revision.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ParagraphId {
    pub module: String,
    pub entry: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeOccurrence {
    pub paragraph: ParagraphId,
    pub node: String,
    /// One-based visit count for this source node within the paragraph invocation.
    pub visit: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChoiceOption {
    pub id: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NodeContent {
    Line {
        data: Value,
    },
    Choice {
        options: Vec<ChoiceOption>,
        selected: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeRecord {
    pub occurrence: NodeOccurrence,
    pub content: NodeContent,
}

/// This token is live-only. It is never a saved execution position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputWait {
    pub token: u64,
    pub action: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoryStatus {
    Idle,
    Waiting(InputWait),
    Completed,
    Cancelled,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunctionRef {
    pub module: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Keyframe {
    pub time: f64,
    pub value: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PropertyTrack {
    pub target: String,
    pub property: String,
    pub keys: Vec<Keyframe>,
}

/// `complete` supplies the required result when skipping the timed `call`.
/// Either implementation runs once, including when both references are equal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimelineCallback {
    pub time: f64,
    pub call: FunctionRef,
    pub complete: FunctionRef,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimelineDeclaration {
    pub duration: f64,
    pub tracks: Vec<PropertyTrack>,
    #[serde(default)]
    pub callbacks: Vec<TimelineCallback>,
    pub on_complete: Option<FunctionRef>,
    pub on_cancel: Option<FunctionRef>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegmentStatus {
    Running,
    Completed,
    Cancelled,
}

/// Explicit business data only. Functions, threads and native handles cannot enter it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoverableState {
    pub variables: BTreeMap<String, Value>,
    pub scene: BTreeMap<String, Value>,
    pub extensions: BTreeMap<String, Value>,
}

impl RecoverableState {
    pub(super) fn scope_mut(&mut self, scope: &str) -> mlua::Result<&mut BTreeMap<String, Value>> {
        match scope {
            "variables" => Ok(&mut self.variables),
            "scene" => Ok(&mut self.scene),
            "extensions" => Ok(&mut self.extensions),
            _ => Err(mlua::Error::runtime("unknown recoverable state scope")),
        }
    }
}

/// Project presentation adapter. Display receives the actual shared layout,
/// including source/cluster mapping and glyph resources, not a second measurement.
pub trait AuthoringHost: TextLayoutProvider {
    fn display_text(&self, id: &str, layout: Arc<TextLayoutResult>) -> mlua::Result<()>;
    fn set_property(&self, target: &str, property: &str, value: f64) -> mlua::Result<()>;
    /// Stop/release resources owned by a segment, for either terminal outcome.
    fn release_segment(&self, id: u64) -> mlua::Result<()>;
}
