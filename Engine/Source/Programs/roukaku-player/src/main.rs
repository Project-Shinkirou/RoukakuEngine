#[cfg(any(
    target_os = "windows",
    target_os = "linux",
    target_os = "macos",
    target_os = "ios"
))]
mod bundle_config;

mod native_session_clock;

#[cfg(any(
    target_os = "windows",
    target_os = "linux",
    target_os = "macos",
    target_os = "ios"
))]
mod preview_control;

#[cfg(any(
    target_os = "windows",
    target_os = "linux",
    target_os = "macos",
    target_os = "ios"
))]
mod preview_transport;

use roukaku_observability::{ConsoleFormat, HostObservabilityConfig, HostRole, init_host};

use roukaku_player::{
    AndroidInputHost, LinuxUinputHost, MacosCgEventHost, PlayerObservabilityOverrides,
    WebCdpInputHost, WebLiveAutomationRequest, WindowsLiveAutomationRequest, WindowsSendInputHost,
    bundled_player_resource_root, load_bundled_observability,
};

use roukaku_player_core::{
    PlayerAutomationScript, PlayerHostCommandResult, PlayerInputTranscript, PlayerPlatform,
};

use std::{env, fs, path::PathBuf};

mod player_cli_error;
use player_cli_error::PlayerCliError;
use player_cli_error::main;
mod run_bundled_game;
#[cfg(any(
    target_os = "windows",
    target_os = "linux",
    target_os = "macos",
    target_os = "ios"
))]
use run_bundled_game::run_bundled_game;
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
