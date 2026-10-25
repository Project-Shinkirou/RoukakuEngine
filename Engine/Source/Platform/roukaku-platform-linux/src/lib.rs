#[cfg(target_os = "linux")]
mod decode;
mod factory;
#[cfg(all(target_os = "linux", feature = "platform-test-driver"))]
mod test_driver;

pub use factory::*;
#[cfg(all(target_os = "linux", feature = "platform-test-driver"))]
pub use test_driver::*;

use roukaku_platform::{PlatformCapabilityReport, build_fingerprint};

#[cfg(target_os = "linux")]
use roukaku_platform::PlatformHostProfile;

#[cfg(not(target_os = "linux"))]
use roukaku_platform::{PlatformId, SdkStatus};

pub fn probe(target: Option<&str>) -> PlatformCapabilityReport {
    #[cfg(target_os = "linux")]
    {
        let profile = PlatformHostProfile::linux_steam_sniper_release(
            target.unwrap_or("nativevn-game"),
            "com.roukaku.probe",
        );
        let mut report = PlatformCapabilityReport::from_profile(
            &profile,
            build_fingerprint(
                env!("CARGO_PKG_NAME"),
                env!("CARGO_PKG_VERSION"),
                [
                    "linux-host",
                    "steam-sniper",
                    "wayland",
                    "vulkan",
                    "alsa",
                    "gstreamer",
                ],
            ),
            std::iter::empty::<&str>(),
        )
        .expect("built-in Linux profile is valid");
        report.diagnostics.push(roukaku_core::Diagnostic::blocking(
            "ROUKAKU_PLATFORM_RUNTIME_PROBE_REQUIRED",
            "provider availability requires a live host conformance run",
        ));
        report
    }
    #[cfg(not(target_os = "linux"))]
    {
        PlatformCapabilityReport::unavailable(
            PlatformId::Linux,
            target,
            SdkStatus::Missing,
            build_fingerprint(
                env!("CARGO_PKG_NAME"),
                env!("CARGO_PKG_VERSION"),
                ["unavailable"],
            ),
        )
    }
}
