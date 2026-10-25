use roukaku_package::{PackageManifest, PackageReader};

use roukaku_platform::{
    PlatformError, PlatformErrorCode, PlatformHostProfile, migrate_host_profile_json,
    validate_host_profile,
};

use serde::{Deserialize, Serialize};

pub use roukaku_player_core::WEB_PLAYER_LIVE_EVIDENCE_SCHEMA;

#[cfg(any(target_arch = "wasm32", feature = "web-code-check"))]
mod browser;

mod web_player_live_evidence;
pub use web_player_live_evidence::WebPlayerConfig;
pub use web_player_live_evidence::WebPlayerLiveEvidence;
pub use web_player_live_evidence::validate_package;
