use super::*;

pub(crate) fn vn_presentation_provider_check(
    package: &PackageReader,
    profile: &str,
) -> ReleaseCheckRecord {
    let manifest = match load_presentation_provider_manifest(package, profile) {
        Ok(manifest) => manifest,
        Err(err) => {
            return ReleaseCheckRecord {
                id: "vn.presentation_provider".to_string(),
                domain: ReleaseDomain::Vn,
                status: CheckStatus::Blocked,
                summary: "vn.presentation_provider_manifest section is missing or invalid"
                    .to_string(),
                diagnostic: Some(Diagnostic::blocking(
                    "ROUKAKU_VN_PRESENTATION_PROVIDER_MANIFEST",
                    err.to_string(),
                )),
                evidence: vec![
                    evidence("section", "vn.presentation_provider_manifest"),
                    evidence("profile", profile),
                ],
            };
        }
    };

    let report = manifest.validate_standard();
    if !report.passed {
        return ReleaseCheckRecord {
            id: "vn.presentation_provider".to_string(),
            domain: ReleaseDomain::Vn,
            status: CheckStatus::Blocked,
            summary:
                "VN presentation provider manifest lacks required filter/fallback/wait support"
                    .to_string(),
            diagnostic: report.diagnostics.first().cloned(),
            evidence: vec![
                evidence("section", "vn.presentation_provider_manifest"),
                evidence("profile", profile),
                evidence("filter_count", report.filter_count),
                evidence("fallback_count", report.fallback_count),
                evidence("wait_capability_count", report.wait_capability_count),
                evidence("profile_count", report.profile_count),
                evidence("preset_count", report.preset_count),
                evidence("diagnostic_count", report.diagnostics.len()),
            ],
        };
    }

    ReleaseCheckRecord {
        id: "vn.presentation_provider".to_string(),
        domain: ReleaseDomain::Vn,
        status: CheckStatus::Pass,
        summary:
            "VN presentation provider declares validated profile, preset, fallback and await policy"
                .to_string(),
        diagnostic: None,
        evidence: vec![
            evidence("section", "vn.presentation_provider_manifest"),
            evidence("profile", profile),
            evidence("filter_count", report.filter_count),
            evidence("fallback_count", report.fallback_count),
            evidence("wait_capability_count", report.wait_capability_count),
            evidence("profile_count", report.profile_count),
            evidence("preset_count", report.preset_count),
        ],
    }
}

pub(crate) fn vn_system_ui_profile_check(
    package: &PackageReader,
    profile: &str,
) -> ReleaseCheckRecord {
    let profile_manifest = match package
        .container()
        .decode_postcard::<VnSystemUiProfileManifest>("vn.system_ui_profile_manifest")
    {
        Ok(manifest) => manifest,
        Err(err) => {
            return ReleaseCheckRecord {
                id: "vn.system_ui_profile".to_string(),
                domain: ReleaseDomain::Vn,
                status: CheckStatus::Blocked,
                summary: "vn.system_ui_profile_manifest section is missing or invalid".to_string(),
                diagnostic: Some(Diagnostic::blocking(
                    "ROUKAKU_VN_SYSTEM_UI_PROFILE_MANIFEST",
                    err.to_string(),
                )),
                evidence: vec![
                    evidence("section", "vn.system_ui_profile_manifest"),
                    evidence("profile", profile),
                ],
            };
        }
    };
    let profile_report = profile_manifest.validate();
    if profile_report.status == SystemStoryValidationStatus::Blocked {
        return ReleaseCheckRecord {
            id: "vn.system_ui_profile".to_string(),
            domain: ReleaseDomain::Vn,
            status: CheckStatus::Blocked,
            summary: "VN system UI profile is missing migration, unlock or localization coverage"
                .to_string(),
            diagnostic: profile_report.diagnostics.first().cloned(),
            evidence: vec![
                evidence("section", "vn.system_ui_profile_manifest"),
                evidence("profile", profile),
                evidence("diagnostic_count", profile_report.diagnostics.len()),
            ],
        };
    }

    ReleaseCheckRecord {
        id: "vn.system_ui_profile".to_string(),
        domain: ReleaseDomain::Vn,
        status: CheckStatus::Pass,
        summary: "VN system UI profile declares profile and coverage policies".to_string(),
        diagnostic: None,
        evidence: vec![
            evidence("section", "vn.system_ui_profile_manifest"),
            evidence("profile", profile),
            evidence("profile_count", profile_manifest.profiles.len()),
            evidence("unlock_source_count", profile_manifest.unlock_sources.len()),
            evidence(
                "localization_locale_count",
                profile_manifest.localization.locales.len(),
            ),
            evidence("save_schema", profile_manifest.save_schema),
        ],
    }
}
