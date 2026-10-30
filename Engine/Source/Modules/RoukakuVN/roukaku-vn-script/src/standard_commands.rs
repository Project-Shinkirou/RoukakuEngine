use std::collections::BTreeMap;

use roukaku_core::Diagnostic;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    CompiledCommand, CompiledStory, PresentationCommand, StageCommand, TimelineCommand,
    VnMovieEndBehavior,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct VnStandardCommandManifest {
    pub schema: String,
    pub provider_id: String,
    pub commands: Vec<VnStandardCommandDescriptor>,
}

impl VnStandardCommandManifest {
    pub fn standard() -> Self {
        tracing::info!(
            event = "vn.commands.registry.create",
            "RoukakuVN standard command registry created"
        );
        Self {
            schema: "roukaku.vn.standard_command_manifest.v2".to_string(),
            provider_id: "roukaku.vn.standard_commands".to_string(),
            commands: vec![
                descriptor("background", "roukaku.command.background.v2"),
                descriptor("show", "roukaku.command.show.v2"),
                descriptor("hide", "roukaku.command.hide.v2"),
                descriptor("move", "roukaku.command.move.v2"),
                descriptor("camera", "roukaku.command.camera.v2"),
                descriptor("transition", "roukaku.command.transition.v2"),
                descriptor("shake", "roukaku.command.shake.v2"),
                descriptor("movie", "roukaku.command.movie.v2"),
                descriptor("voice", "roukaku.command.voice.v2"),
                descriptor("bgm", "roukaku.command.bgm.v2"),
                descriptor("se", "roukaku.command.se.v2"),
                descriptor("audio", "roukaku.command.audio_control.v2"),
                descriptor("stage", "roukaku.command.stage.v2"),
                descriptor("layer", "roukaku.command.layer.v2"),
                descriptor("timeline", "roukaku.command.timeline.v2"),
                descriptor("effect", "roukaku.command.effect.v2"),
            ],
        }
    }

    /// Validate the manifest shape. Luau stories emit stage commands at run
    /// time, so Players validate each command against this manifest when it arrives.
    pub fn validate(&self) -> VnStandardCommandValidationReport {
        let mut diagnostics = Vec::new();
        if self.schema != "roukaku.vn.standard_command_manifest.v2" {
            diagnostics.push(Diagnostic::blocking(
                "ROUKAKU_VN_STANDARD_COMMAND_SCHEMA",
                "standard command manifest schema is invalid",
            ));
        }
        if self.provider_id != "roukaku.vn.standard_commands" {
            diagnostics.push(Diagnostic::blocking(
                "ROUKAKU_VN_STANDARD_COMMAND_PROVIDER",
                "standard command manifest must use roukaku.vn.standard_commands",
            ));
        }
        for required in ["show", "hide", "camera", "movie", "voice", "bgm", "se"] {
            if !self
                .commands
                .iter()
                .any(|descriptor| descriptor.command == required)
            {
                diagnostics.push(
                    Diagnostic::blocking(
                        "ROUKAKU_VN_STANDARD_COMMAND_REQUIRED",
                        "standard command manifest is missing a required command",
                    )
                    .with_field("command", required),
                );
            }
        }
        VnStandardCommandValidationReport {
            passed: diagnostics.is_empty(),
            diagnostics,
            command_count: self.commands.len(),
            checked_usage_count: 0,
        }
    }

    pub fn validate_usage(&self, compiled: &CompiledStory) -> VnStandardCommandValidationReport {
        tracing::debug!(
            event = "vn.commands.validate.start",
            command_count = self.commands.len(),
            state_count = compiled.states.len(),
            "RoukakuVN command validation started"
        );
        let VnStandardCommandValidationReport {
            mut diagnostics, ..
        } = self.validate();
        let descriptors = self
            .commands
            .iter()
            .map(|descriptor| (descriptor.command.as_str(), descriptor))
            .collect::<BTreeMap<_, _>>();

        let mut checked_usage_count = 0usize;
        for command in compiled
            .states
            .values()
            .flat_map(|state| &state.scenes)
            .flat_map(|scene| &scene.commands)
        {
            let CompiledCommand::Presentation {
                id,
                command: PresentationCommand::Stage(stage),
            } = command
            else {
                continue;
            };
            checked_usage_count += 1;
            let command = stage.kind();
            let Some(_descriptor) = descriptors.get(command) else {
                diagnostics.push(
                    Diagnostic::blocking(
                        "ROUKAKU_VN_STANDARD_COMMAND_UNKNOWN",
                        "compiled story uses a presentation command without a standard descriptor",
                    )
                    .with_field("command_id", id)
                    .with_field("command", command),
                );
                continue;
            };
            match stage {
                StageCommand::Movie {
                    end: VnMovieEndBehavior::Wait,
                    fence,
                    fallback,
                    ..
                } if fence.is_none() || fallback.is_none() => diagnostics.push(
                    Diagnostic::blocking(
                        "ROUKAKU_VN_STANDARD_COMMAND_FALLBACK",
                        "movie end:wait requires a fence and deterministic fallback frame",
                    )
                    .with_field("command_id", id),
                ),
                StageCommand::Timeline(TimelineCommand::Start(timeline))
                    if timeline.tracks.is_empty()
                        || timeline
                            .tracks
                            .iter()
                            .any(|track| track.keyframes.len() < 2) =>
                {
                    diagnostics.push(
                        Diagnostic::blocking(
                            "ROUKAKU_VN_STANDARD_TIMELINE_TRACK",
                            "timeline requires typed tracks with at least two keyframes",
                        )
                        .with_field("command_id", id),
                    );
                }
                _ => {}
            }
        }

        VnStandardCommandValidationReport {
            passed: diagnostics.is_empty(),
            diagnostics,
            command_count: self.commands.len(),
            checked_usage_count,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct VnStandardCommandDescriptor {
    pub command: String,
    pub schema: String,
    pub provider_id: String,
    pub release_checks: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct VnStandardCommandValidationReport {
    pub passed: bool,
    pub diagnostics: Vec<Diagnostic>,
    pub command_count: usize,
    pub checked_usage_count: usize,
}

fn descriptor(
    command: impl Into<String>,
    schema: impl Into<String>,
) -> VnStandardCommandDescriptor {
    VnStandardCommandDescriptor {
        command: command.into(),
        schema: schema.into(),
        provider_id: "roukaku.vn.standard_commands".to_string(),
        release_checks: vec![
            "vn.standard_commands".to_string(),
            "vn.presentation_provider".to_string(),
        ],
    }
}
