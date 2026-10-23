use roukaku_core::{
    Hash256, PerformanceBudget, PerformanceMetricBudget, PerformanceRecorder,
    PerformanceRunIdentity, PerformanceThresholds, PerformanceUnit, PERFORMANCE_BUDGET_SCHEMA,
};

use roukaku_package::{
    ContainerKind, PackageBuildRequest, PackageBuilder, PackageManifest, PackageReader,
    RoukakuContainerBuilder, SectionPayload, CURRENT_CONTAINER_VERSION,
};

use roukaku_platform::{
    ConformanceCheck, ConformanceStatus, PlatformCapabilityReport, PlatformHostConformanceReport,
    PlatformHostProfile, PlatformId, PLATFORM_HOST_CONFORMANCE_REPORT_SCHEMA,
};

use roukaku_player_core::{
    PlayerAutomationCheck, PlayerAutomationEvidence, PlayerAutomationReport,
    PlayerAutomationStatus, PlayerPlatform, PlayerPresentationReport,
    PLAYER_PRESENTATION_REPORT_SCHEMA,
};

use roukaku_release::{
    CheckStatus, PackageValidateRequest, ProductPerformanceEvidence, ReleaseDomain,
    ReleaseValidator,
};

use roukaku_vn_package::{
    package_sections_for_project, PLAYER_LOCALE_CONFIG_SCHEMA, VN_LOCALIZATION_TABLE_SCHEMA,
};

use roukaku_vn_script::{compile_roukaku_project, RoukakuSource};

#[path = "release_report/headless_gate.rs"]
mod headless_gate;
#[cfg(feature = "ffmpeg-vcpkg")]
use headless_gate::required_ffmpeg_gate_passes_only_after_native_probe;
#[path = "release_report/extension_gate.rs"]
mod extension_gate;
#[path = "release_report/performance_gate.rs"]
mod performance_gate;
#[path = "release_report/platform_gate.rs"]
mod platform_gate;
#[path = "release_report/vn_gate.rs"]
mod vn_gate;
use extension_gate::append_valid_locale_sections;
use extension_gate::cooked_project_section;
use extension_gate::nativevn_story_with_system_pages;
use extension_gate::nativevn_target_manifest;
use extension_gate::package_request;
use extension_gate::package_with_target_manifest;
use extension_gate::platform_capability;
use extension_gate::player_report;
