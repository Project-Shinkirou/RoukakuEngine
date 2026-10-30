use std::collections::BTreeMap;

use roukaku_package::{PackageBuildRequest, PackageBuilder, PackageReader, SectionPayload};
use roukaku_vn_package::{
    LuauStoryBundle, VN_PRESENTATION_PROVIDER_MANIFEST_SCHEMA, VnProject, VnProjectRoot,
    decode_vn_project, load_presentation_provider_manifest, package_sections_for_project,
};
use roukaku_vn_policy::authoring::ParagraphId;
use roukaku_vn_script::{RoukakuSource, compile_roukaku_ui};

fn project() -> VnProject {
    let story = LuauStoryBundle::new(
        ParagraphId {
            module: "story.main".into(),
            entry: "start".into(),
        },
        BTreeMap::from([(
            "story.main".into(),
            "return { paragraphs = { start = function() end } }".into(),
        )]),
    )
    .unwrap();
    let ui = compile_roukaku_ui(Vec::<RoukakuSource>::new(), Default::default()).unwrap();
    VnProject::new(story, ui).unwrap()
}

fn package(sections: Vec<SectionPayload>) -> PackageReader {
    let blob = PackageBuilder::build(PackageBuildRequest::fixture(
        "presentation.policy",
        "classic",
        sections,
    ))
    .unwrap();
    PackageReader::open(blob.as_bytes()).unwrap()
}

#[test]
fn package_persists_profile_bound_presentation_policy() {
    let package = package(
        package_sections_for_project(
            &project(),
            &["classic".to_string(), "advanced-vn".to_string()],
            "game",
        )
        .unwrap(),
    );

    let manifest = load_presentation_provider_manifest(&package, "classic").unwrap();
    assert_eq!(manifest.schema, VN_PRESENTATION_PROVIDER_MANIFEST_SCHEMA);
    assert!(
        manifest
            .resolve_preset("classic", "background", "soft_fade")
            .is_ok()
    );
    assert_eq!(
        load_presentation_provider_manifest(&package, "undeclared")
            .unwrap_err()
            .to_string(),
        "ROUKAKU_VN_PRESENTATION_PROFILE_UNDECLARED: requested presentation profile is not declared by the package"
    );
}

#[test]
fn luau_project_round_trips_and_rejects_retired_or_tampered_roots() {
    let project = project();
    let mut sections =
        package_sections_for_project(&project, &["classic".to_string()], "game").unwrap();
    assert_eq!(decode_vn_project(&package(sections.clone())).unwrap(), project);

    let mut mixed = sections.clone();
    mixed.push(SectionPayload::postcard("vn.story", "roukaku.vn.story", &"legacy").unwrap());
    assert!(
        decode_vn_project(&package(mixed))
            .unwrap_err()
            .to_string()
            .contains("ROUKAKU_VN_STORY_SOURCE_MIXED")
    );

    let root: VnProjectRoot = package(sections.clone())
        .container()
        .decode_postcard("vn.project")
        .unwrap();
    sections.retain(|section| section.id != "vn.project");
    let mut tampered = sections.clone();
    tampered.push(
        SectionPayload::postcard(
            "vn.project",
            "roukaku.vn.project_root.v5",
            &VnProjectRoot {
                project_hash: roukaku_core::Hash256::from_sha256(b"other"),
                ..root.clone()
            },
        )
        .unwrap(),
    );
    assert!(
        decode_vn_project(&package(tampered))
            .unwrap_err()
            .to_string()
            .contains("ROUKAKU_VN_PROJECT_HASH")
    );
    sections.push(
        SectionPayload::postcard(
            "vn.project",
            "roukaku.vn.compiled_project_root.v4",
            &VnProjectRoot {
                schema: "roukaku.vn.compiled_project_root.v4".into(),
                ..root
            },
        )
        .unwrap(),
    );
    assert!(
        decode_vn_project(&package(sections))
            .unwrap_err()
            .to_string()
            .contains("ROUKAKU_VN_PROJECT_ROOT")
    );
}

#[test]
fn legacy_presentation_policy_requires_recook() {
    let legacy = roukaku_vn_package::VnPresentationProviderManifest::standard();
    let section = SectionPayload::postcard(
        "vn.presentation_provider_manifest",
        "roukaku.vn.presentation_provider_manifest.v1",
        &legacy,
    )
    .unwrap();
    let package = package(vec![section]);

    let error = load_presentation_provider_manifest(&package, "classic").unwrap_err();
    assert!(
        error
            .to_string()
            .contains("ROUKAKU_VN_PRESENTATION_PROVIDER_SCHEMA")
    );
}
