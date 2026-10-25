#[cfg(any(
    target_os = "windows",
    target_os = "linux",
    target_os = "macos",
    target_os = "ios"
))]
#[path = "bundle_config.rs"]
mod bundle_config;

use crate::native_session_clock;

#[cfg(any(
    target_os = "windows",
    target_os = "linux",
    target_os = "macos",
    target_os = "ios"
))]
#[path = "preview_control.rs"]
mod preview_control;

#[cfg(any(
    target_os = "windows",
    target_os = "linux",
    target_os = "macos",
    target_os = "ios"
))]
#[path = "preview_transport.rs"]
mod preview_transport;

use roukaku_observability::{init_host, ConsoleFormat, HostObservabilityConfig, HostRole};

use crate::{
    bundled_player_resource_root, load_bundled_observability, AndroidInputHost, LinuxUinputHost,
    MacosCgEventHost, PlayerObservabilityOverrides, WebCdpInputHost, WebLiveAutomationRequest,
    WindowsLiveAutomationRequest, WindowsSendInputHost,
};

use roukaku_player_core::{
    PlayerAutomationScript, PlayerHostCommandResult, PlayerInputTranscript, PlayerPlatform,
};

use std::{env, fs, path::PathBuf};

#[path = "player_cli_error.rs"]
mod player_cli_error;
pub use player_cli_error::main as run;
use player_cli_error::PlayerCliError;
#[path = "run_bundled_game.rs"]
mod run_bundled_game;
#[cfg(any(
    target_os = "windows",
    target_os = "linux",
    target_os = "macos",
    target_os = "ios"
))]
use run_bundled_game::run_bundled_game;
#[path = "ui_button_state.rs"]
mod ui_button_state;
#[cfg(any(
    target_os = "windows",
    target_os = "linux",
    target_os = "macos",
    target_os = "ios"
))]
use ui_button_state::execute_platform_load;
#[cfg(any(
    target_os = "windows",
    target_os = "linux",
    target_os = "macos",
    target_os = "ios"
))]
use ui_button_state::execute_platform_save;
#[cfg(any(
    target_os = "windows",
    target_os = "linux",
    target_os = "macos",
    target_os = "ios"
))]
use ui_button_state::gamepad_navigation;
#[cfg(any(
    target_os = "windows",
    target_os = "linux",
    target_os = "macos",
    target_os = "ios"
))]
use ui_button_state::log_consumed_vn_step;
use ui_button_state::parse_usize_arg;
#[cfg(any(
    target_os = "windows",
    target_os = "linux",
    target_os = "macos",
    target_os = "ios"
))]
use ui_button_state::player_platform_error;
#[cfg(not(any(
    target_os = "windows",
    target_os = "linux",
    target_os = "macos",
    target_os = "ios"
)))]
use ui_button_state::run_bundled_game;
#[cfg(any(
    target_os = "windows",
    target_os = "linux",
    target_os = "macos",
    target_os = "ios"
))]
use ui_button_state::ui_button_state;
#[cfg(any(
    target_os = "windows",
    target_os = "linux",
    target_os = "macos",
    target_os = "ios"
))]
use ui_button_state::ui_pointer_button;
use ui_button_state::write_json;
