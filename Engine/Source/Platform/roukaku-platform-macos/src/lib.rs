#[cfg(any(target_os = "macos", target_os = "ios"))]
mod accessibility;
mod factory;
#[cfg(target_os = "ios")]
pub use factory::macos::ios;
#[cfg(all(target_os = "macos", feature = "platform-test-driver"))]
mod test_driver;

pub use factory::*;
#[cfg(all(target_os = "macos", feature = "platform-test-driver"))]
pub use test_driver::*;

use roukaku_platform::{build_fingerprint, PlatformCapabilityReport};

#[cfg(target_os = "macos")]
use roukaku_platform::PlatformHostProfile;
#[cfg(not(target_os = "macos"))]
use roukaku_platform::{PlatformId, SdkStatus};

pub fn probe(target: Option<&str>) -> PlatformCapabilityReport {
    #[cfg(target_os = "macos")]
    {
        let profile = PlatformHostProfile::macos_release(
            target.unwrap_or("nativevn-game"),
            "com.roukaku.probe",
        );
        let mut report = PlatformCapabilityReport::from_profile(
            &profile,
            build_fingerprint(
                env!("CARGO_PKG_NAME"),
                env!("CARGO_PKG_VERSION"),
                ["macos-host", "appkit", "metal", "coreaudio", "avfoundation"],
            ),
            std::iter::empty::<&str>(),
        )
        .expect("built-in macOS profile is valid");
        report.diagnostics.push(roukaku_core::Diagnostic::blocking(
            "ROUKAKU_PLATFORM_RUNTIME_PROBE_REQUIRED",
            "provider availability requires a live host conformance run",
        ));
        report
    }
    #[cfg(not(target_os = "macos"))]
    {
        PlatformCapabilityReport::unavailable(
            PlatformId::Macos,
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
