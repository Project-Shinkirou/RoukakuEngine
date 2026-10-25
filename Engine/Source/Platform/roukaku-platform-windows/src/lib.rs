mod factory;
#[cfg(target_os = "windows")]
mod source_directory;
#[cfg(all(target_os = "windows", feature = "platform-test-driver"))]
mod test_driver;
#[cfg(target_os = "windows")]
mod window_placement;

pub use factory::*;
#[cfg(target_os = "windows")]
pub use source_directory::*;
#[cfg(all(target_os = "windows", feature = "platform-test-driver"))]
pub use test_driver::*;

use roukaku_platform::{PlatformCapabilityReport, build_fingerprint};

#[cfg(target_os = "windows")]
use roukaku_platform::PlatformHostProfile;

#[cfg(not(target_os = "windows"))]
use roukaku_platform::{PlatformId, SdkStatus};

pub fn probe(target: Option<&str>) -> PlatformCapabilityReport {
    #[cfg(target_os = "windows")]
    {
        let profile = PlatformHostProfile::windows_release(
            target.unwrap_or("nativevn-game"),
            "com.roukaku.probe",
        );
        let mut report = PlatformCapabilityReport::from_profile(
            &profile,
            build_fingerprint(
                env!("CARGO_PKG_NAME"),
                env!("CARGO_PKG_VERSION"),
                ["windows-host"],
            ),
            std::iter::empty::<&str>(),
        )
        .expect("built-in Windows profile is valid");
        report.diagnostics.push(roukaku_core::Diagnostic::blocking(
            "ROUKAKU_PLATFORM_RUNTIME_PROBE_REQUIRED",
            "provider availability requires a live host conformance run",
        ));
        report
    }
    #[cfg(not(target_os = "windows"))]
    {
        PlatformCapabilityReport::unavailable(
            PlatformId::Windows,
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
#[cfg(target_os = "windows")]
mod accessibility;
