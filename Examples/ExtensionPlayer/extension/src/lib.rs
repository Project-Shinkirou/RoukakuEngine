use roukaku_media_core::SceneCommand;
use roukaku_plugin::{
    ExtensionCommandDeclaration, ExtensionInvocation, ExtensionRestoration, ExtensionState,
    ExtensionStateSnapshot, WorkExtension, WorkExtensionDeclaration,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

pub const PROVIDER: &str = "example.progress_meter";
pub const COMMAND: &str = "progress_meter";
pub const SCHEMA: &str = "example.progress_meter.v1";

/// A project-controlled progress indicator. Values come from the work's commands.
#[derive(Default)]
pub struct ProgressMeter {
    state: MeterState,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct MeterState {
    completed: u32,
    total: u32,
}

impl Default for MeterState {
    fn default() -> Self {
        Self {
            completed: 0,
            total: 1,
        }
    }
}
impl MeterState {
    fn validate(&self) -> Result<(), String> {
        if self.total == 0 || self.completed > self.total {
            return Err("progress_meter requires 0 <= completed <= total and total > 0".into());
        }
        Ok(())
    }

    fn from_snapshot(snapshot: &ExtensionStateSnapshot) -> Result<Self, String> {
        if snapshot.schema != SCHEMA {
            return Err("progress_meter state schema mismatch".into());
        }
        let state = Self::deserialize(&snapshot.value).map_err(|error| error.to_string())?;
        state.validate()?;
        Ok(state)
    }
}

impl WorkExtension for ProgressMeter {
    fn declaration() -> WorkExtensionDeclaration {
        WorkExtensionDeclaration {
            provider_id: PROVIDER.into(),
            commands: [(
                COMMAND.into(),
                ExtensionCommandDeclaration {
                    schema: SCHEMA.into(),
                    arguments_schema: json!({
                        "type": "object", "additionalProperties": false,
                        "required": ["completed", "total"],
                        "properties": {
                            "completed": {"type": "integer", "minimum": 0, "maximum": u32::MAX},
                            "total": {"type": "integer", "minimum": 1, "maximum": u32::MAX}
                        }
                    }),
                },
            )]
            .into(),
        }
    }

    fn execute(
        &mut self,
        invocation: ExtensionInvocation<'_>,
    ) -> Result<Vec<SceneCommand>, String> {
        let state =
            MeterState::deserialize(invocation.arguments).map_err(|error| error.to_string())?;
        state.validate()?;
        let width = (u64::from(state.completed) * 240 / u64::from(state.total)) as u32;
        self.state = state;
        Ok(vec![
            SceneCommand::rect("progress_meter.track", 16, 16, 240, 12, [48, 48, 48, 255]),
            SceneCommand::rect(
                "progress_meter.fill",
                16,
                16,
                width,
                12,
                [32, 192, 128, 255],
            ),
        ])
    }

    fn restoration(&mut self) -> ExtensionRestoration<'_> {
        ExtensionRestoration::Restorable(self)
    }
}

impl ExtensionState for ProgressMeter {
    fn capture(&self) -> Result<ExtensionStateSnapshot, String> {
        Ok(ExtensionStateSnapshot {
            schema: SCHEMA.into(),
            value: serde_json::to_value(&self.state).map_err(|error| error.to_string())?,
        })
    }

    fn validate_restore(&self, snapshot: &ExtensionStateSnapshot) -> Result<(), String> {
        MeterState::from_snapshot(snapshot).map(|_| ())
    }

    fn restore(&mut self, snapshot: &ExtensionStateSnapshot) -> Result<(), String> {
        self.state = MeterState::from_snapshot(snapshot)?;
        Ok(())
    }
}
