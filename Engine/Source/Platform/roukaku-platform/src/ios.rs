use crate::{PlatformCapabilityReport, PlatformHostProfile, build_fingerprint};

#[cfg(target_os = "ios")]
mod documents;
#[cfg(target_os = "ios")]
pub use documents::{open_document, pick_document, restore_document_access};

/// Activate the application's actual playback session before opening CoreAudio.
#[cfg(target_os = "ios")]
pub fn activate_audio() -> Result<(), crate::PlatformError> {
    use objc2_avf_audio::{AVAudioSession, AVAudioSessionCategoryPlayback};
    // These methods operate on AVFAudio's process-owned singleton. Playback
    // requests neither microphone access nor a background-audio entitlement.
    unsafe {
        let session = AVAudioSession::sharedInstance();
        let category = AVAudioSessionCategoryPlayback.ok_or_else(|| {
            crate::PlatformError::new(
                crate::PlatformErrorCode::ProviderUnavailable,
                "ios.audio.activate",
                "AVAudioSession playback category is unavailable",
            )
        })?;
        session
            .setCategory_error(category)
            .and_then(|()| session.setActive_error(true))
            .map_err(|error| {
                crate::PlatformError::new(
                    crate::PlatformErrorCode::ProviderUnavailable,
                    "ios.audio.activate",
                    error.to_string(),
                )
            })
    }
}

pub fn probe(target: Option<&str>) -> PlatformCapabilityReport {
    let profile =
        PlatformHostProfile::ios_release(target.unwrap_or("nativevn-game"), "org.shinkirou.probe");
    let mut report = PlatformCapabilityReport::from_profile(
        &profile,
        build_fingerprint(
            env!("CARGO_PKG_NAME"),
            env!("CARGO_PKG_VERSION"),
            ["ios-host", "uikit", "metal", "coreaudio", "avfoundation"],
        ),
        std::iter::empty::<&str>(),
    )
    .expect("built-in iOS profile is valid");
    report.diagnostics.push(roukaku_core::Diagnostic::blocking(
        "ROUKAKU_PLATFORM_RUNTIME_PROBE_REQUIRED",
        "iOS provider availability requires a live device or simulator conformance run",
    ));
    report
}
