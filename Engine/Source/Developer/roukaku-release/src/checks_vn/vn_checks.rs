use super::*;

pub(crate) fn vn_checks(
    package: &PackageReader,
    profile: &str,
    selected_target: Option<&str>,
) -> Vec<ReleaseCheckRecord> {
    if !release_profile_requires_vn(profile)
        && !package.has_section("vn.project")
        && !package.has_section("vn.profile_manifest")
    {
        return Vec::new();
    }
    vec![
        vn_project_check(package, profile),
        vn_locale_config_check(package),
        vn_profile_manifest_check(package, profile, selected_target),
        vn_policy_bundle_check(package, profile),
        vn_extension_bindings_check(package, profile),
        vn_standard_commands_check(package, profile),
        vn_presentation_provider_check(package, profile),
        vn_system_ui_profile_check(package, profile),
    ]
}

pub(crate) fn vn_locale_config_check(package: &PackageReader) -> ReleaseCheckRecord {
    let config = match load_player_locale_config(package) {
        Ok(config) => config,
        Err(error) => {
            return ReleaseCheckRecord {
                id: "player.locale_config".to_string(),
                domain: ReleaseDomain::Vn,
                status: CheckStatus::Blocked,
                summary: "Player locale config or a declared localization table is invalid"
                    .to_string(),
                diagnostic: Some(Diagnostic::blocking(
                    "ROUKAKU_PLAYER_LOCALE_CONFIG",
                    error.to_string(),
                )),
                evidence: vec![evidence("section", "player.locale_config")],
            };
        }
    };
    let entry = package
        .container()
        .section_entry("player.locale_config")
        .expect("validated locale config section must exist");
    ReleaseCheckRecord {
        id: "player.locale_config".to_string(),
        domain: ReleaseDomain::Vn,
        status: CheckStatus::Pass,
        summary: "Player locale config and every declared localization table are valid".to_string(),
        diagnostic: None,
        evidence: vec![
            evidence("section", "player.locale_config"),
            evidence("section_hash", entry.hash),
            evidence("default_locale", config.default_locale),
            evidence("locale_count", config.available_locales.len()),
        ],
    }
}

pub(crate) fn vn_project_check(package: &PackageReader, profile: &str) -> ReleaseCheckRecord {
    let project = match decode_vn_project(package) {
        Ok(project) => project,
        Err(err) => {
            return ReleaseCheckRecord {
                id: "vn.project".to_string(),
                domain: ReleaseDomain::Vn,
                status: CheckStatus::Blocked,
                summary: "vn.project root, Luau story bundle or UI section is missing or invalid"
                    .to_string(),
                diagnostic: Some(Diagnostic::blocking("ROUKAKU_VN_PROJECT", err.to_string())),
                evidence: vec![
                    evidence("section", "vn.project"),
                    evidence("profile", profile),
                ],
            };
        }
    };
    ReleaseCheckRecord {
        id: "vn.project".to_string(),
        domain: ReleaseDomain::Vn,
        status: CheckStatus::Pass,
        summary: "VN project root, Luau story bundle and UI sections are valid".to_string(),
        diagnostic: None,
        evidence: vec![
            evidence("project_hash", project.project_hash),
            evidence("story_revision", project.story.source_revision),
            evidence("story_module_count", project.story.modules.len()),
        ],
    }
}

pub(crate) fn vn_profile_manifest_check(
    package: &PackageReader,
    profile: &str,
    selected_target: Option<&str>,
) -> ReleaseCheckRecord {
    let manifest = match package
        .container()
        .decode_postcard::<VnProfileManifest>("vn.profile_manifest")
    {
        Ok(manifest) => manifest,
        Err(err) => {
            return ReleaseCheckRecord {
                id: "vn.profile_manifest".to_string(),
                domain: ReleaseDomain::Vn,
                status: CheckStatus::Blocked,
                summary: "vn.profile_manifest section is missing or invalid".to_string(),
                diagnostic: Some(Diagnostic::blocking(
                    "ROUKAKU_VN_PROFILE_MANIFEST",
                    err.to_string(),
                )),
                evidence: vec![
                    evidence("section", "vn.profile_manifest"),
                    evidence("profile", profile),
                ],
            };
        }
    };
    if manifest.schema != "roukaku.vn.profile_manifest.v1"
        || !manifest.profiles.iter().any(|p| p == profile)
    {
        return ReleaseCheckRecord {
            id: "vn.profile_manifest".to_string(),
            domain: ReleaseDomain::Vn,
            status: CheckStatus::Blocked,
            summary: "VN profile manifest does not declare the validation profile".to_string(),
            diagnostic: Some(Diagnostic::blocking(
                "ROUKAKU_VN_PROFILE_MISSING",
                "vn.profile_manifest must include the validation profile",
            )),
            evidence: vec![evidence("profile", profile)],
        };
    }
    if let Some(selected_target) = selected_target
        && manifest.target != selected_target
    {
        return ReleaseCheckRecord {
            id: "vn.profile_manifest".to_string(),
            domain: ReleaseDomain::Vn,
            status: CheckStatus::Blocked,
            summary: "VN profile manifest target does not match selected target".to_string(),
            diagnostic: Some(Diagnostic::blocking(
                "ROUKAKU_VN_PROFILE_TARGET",
                "vn.profile_manifest target must match selected target",
            )),
            evidence: vec![
                evidence("manifest_target", manifest.target),
                evidence("selected_target", selected_target),
            ],
        };
    }
    ReleaseCheckRecord {
        id: "vn.profile_manifest".to_string(),
        domain: ReleaseDomain::Vn,
        status: CheckStatus::Pass,
        summary: "VN profile manifest matches target and validation profile".to_string(),
        diagnostic: None,
        evidence: vec![
            evidence("target", manifest.target),
            evidence("profile", profile),
            evidence("profile_count", manifest.profiles.len()),
        ],
    }
}

pub(crate) fn vn_policy_bundle_check(package: &PackageReader, profile: &str) -> ReleaseCheckRecord {
    let manifest = match package
        .container()
        .decode_postcard::<VnPolicyBundleManifest>("vn.policy_bundle_manifest")
    {
        Ok(manifest) => manifest,
        Err(err) => {
            return ReleaseCheckRecord {
                id: "vn.policy_bundle".to_string(),
                domain: ReleaseDomain::Vn,
                status: CheckStatus::Blocked,
                summary: "vn.policy_bundle_manifest section is missing or invalid".to_string(),
                diagnostic: Some(Diagnostic::blocking(
                    "ROUKAKU_VN_POLICY_BUNDLE",
                    err.to_string(),
                )),
                evidence: vec![
                    evidence("section", "vn.policy_bundle_manifest"),
                    evidence("profile", profile),
                ],
            };
        }
    };

    let cache = match package
        .container()
        .decode_postcard::<VnPolicyBundleSourceCache>("vn.policy_bundle_source_cache")
    {
        Ok(cache) => cache,
        Err(err) => {
            return ReleaseCheckRecord {
                id: "vn.policy_bundle".to_string(),
                domain: ReleaseDomain::Vn,
                status: CheckStatus::Blocked,
                summary: "vn.policy_bundle_source_cache section is missing or invalid".to_string(),
                diagnostic: Some(Diagnostic::blocking(
                    "ROUKAKU_VN_POLICY_CACHE",
                    err.to_string(),
                )),
                evidence: vec![
                    evidence("section", "vn.policy_bundle_source_cache"),
                    evidence("profile", profile),
                ],
            };
        }
    };

    let report = manifest.validate_standard_with_cache(&cache);
    if !report.passed {
        return ReleaseCheckRecord {
            id: "vn.policy_bundle".to_string(),
            domain: ReleaseDomain::Vn,
            status: CheckStatus::Blocked,
            summary:
                "VN policy bundle manifest and source cache do not satisfy the standard policy gate"
                    .to_string(),
            diagnostic: report.diagnostics.first().cloned(),
            evidence: vec![
                evidence("section", "vn.policy_bundle_manifest"),
                evidence("source_cache_section", "vn.policy_bundle_source_cache"),
                evidence("profile", profile),
                evidence("bundle_count", manifest.bundles.len()),
                evidence("diagnostic_count", report.diagnostics.len()),
            ],
        };
    }

    ReleaseCheckRecord {
        id: "vn.policy_bundle".to_string(),
        domain: ReleaseDomain::Vn,
        status: CheckStatus::Pass,
        summary: "VN standard policy bundle is present and locked".to_string(),
        diagnostic: None,
        evidence: vec![
            evidence("section", "vn.policy_bundle_manifest"),
            evidence("source_cache_section", "vn.policy_bundle_source_cache"),
            evidence("profile", profile),
            evidence("bundle_count", manifest.bundles.len()),
        ],
    }
}

pub(crate) fn vn_extension_bindings_check(
    package: &PackageReader,
    profile: &str,
) -> ReleaseCheckRecord {
    let manifest = match package
        .container()
        .decode_postcard::<VnExtensionManifest>("vn.extension_manifest")
    {
        Ok(manifest) => manifest,
        Err(err) => {
            return ReleaseCheckRecord {
                id: "vn.extension_bindings".to_string(),
                domain: ReleaseDomain::Vn,
                status: CheckStatus::Blocked,
                summary: "vn.extension_manifest section is missing or invalid".to_string(),
                diagnostic: Some(Diagnostic::blocking(
                    "ROUKAKU_VN_EXTENSION_MANIFEST",
                    err.to_string(),
                )),
                evidence: vec![
                    evidence("section", "vn.extension_manifest"),
                    evidence("profile", profile),
                ],
            };
        }
    };

    let report = manifest.validate_required();
    if !report.passed {
        return ReleaseCheckRecord {
            id: "vn.extension_bindings".to_string(),
            domain: ReleaseDomain::Vn,
            status: CheckStatus::Blocked,
            summary: "VN extension manifest is missing required provider bindings".to_string(),
            diagnostic: report.diagnostics.first().cloned(),
            evidence: vec![
                evidence("section", "vn.extension_manifest"),
                evidence("profile", profile),
                evidence("binding_count", manifest.bindings.len()),
                evidence("diagnostic_count", report.diagnostics.len()),
            ],
        };
    }

    ReleaseCheckRecord {
        id: "vn.extension_bindings".to_string(),
        domain: ReleaseDomain::Vn,
        status: CheckStatus::Pass,
        summary: "VN extension manifest declares required explicit bindings".to_string(),
        diagnostic: None,
        evidence: vec![
            evidence("section", "vn.extension_manifest"),
            evidence("profile", profile),
            evidence("binding_count", manifest.bindings.len()),
        ],
    }
}

pub(crate) fn vn_standard_commands_check(
    package: &PackageReader,
    profile: &str,
) -> ReleaseCheckRecord {
    let manifest = match package
        .container()
        .decode_postcard::<VnStandardCommandManifest>("vn.standard_command_manifest")
    {
        Ok(manifest) => manifest,
        Err(err) => {
            return ReleaseCheckRecord {
                id: "vn.standard_commands".to_string(),
                domain: ReleaseDomain::Vn,
                status: CheckStatus::Blocked,
                summary: "vn.standard_command_manifest section is missing or invalid".to_string(),
                diagnostic: Some(Diagnostic::blocking(
                    "ROUKAKU_VN_STANDARD_COMMAND_MANIFEST",
                    err.to_string(),
                )),
                evidence: vec![
                    evidence("section", "vn.standard_command_manifest"),
                    evidence("profile", profile),
                ],
            };
        }
    };
    let report = manifest.validate();
    if !report.passed {
        return ReleaseCheckRecord {
            id: "vn.standard_commands".to_string(),
            domain: ReleaseDomain::Vn,
            status: CheckStatus::Blocked,
            summary: "VN standard command manifest or compiled command usage failed validation"
                .to_string(),
            diagnostic: report.diagnostics.first().cloned(),
            evidence: vec![
                evidence("section", "vn.standard_command_manifest"),
                evidence("profile", profile),
                evidence("command_count", report.command_count),
                evidence("checked_usage_count", report.checked_usage_count),
                evidence("diagnostic_count", report.diagnostics.len()),
            ],
        };
    }

    ReleaseCheckRecord {
        id: "vn.standard_commands".to_string(),
        domain: ReleaseDomain::Vn,
        status: CheckStatus::Pass,
        summary: "VN standard command manifest covers compiled presentation usage".to_string(),
        diagnostic: None,
        evidence: vec![
            evidence("section", "vn.standard_command_manifest"),
            evidence("profile", profile),
            evidence("command_count", report.command_count),
            evidence("checked_usage_count", report.checked_usage_count),
        ],
    }
}
