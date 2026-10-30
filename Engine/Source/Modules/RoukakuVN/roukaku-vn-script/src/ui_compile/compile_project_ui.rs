use super::*;

pub(crate) fn compile_project_ui(
    story: CompiledStory,
    sources: &[RoukakuSource],
    ui_themes: Vec<UiThemeManifest>,
    styles: Vec<CompiledUiStyle>,
    controller_sources: BTreeMap<String, String>,
) -> Result<CompiledVnProject, VnError> {
    let mut views = BTreeMap::new();
    let mut command_bindings = BTreeMap::new();
    let mut system_page_bindings = BTreeMap::new();
    let mut surface_bindings = BTreeMap::new();
    let mut profile_bindings = BTreeMap::new();
    let mut profile_scoped_bindings = BTreeMap::<String, UiProfileScopedBindings>::new();
    let mut system_ui_profiles = BTreeMap::<String, SystemUiProfilePolicy>::new();
    let mut source_map = BTreeMap::new();
    let mut controller_ids = BTreeSet::new();
    let mut theme_ids = BTreeSet::new();
    let mut themes = BTreeMap::new();
    for theme in ui_themes
        .into_iter()
        .chain(styles.iter().map(|style| style.theme.clone()))
    {
        theme.validate().map_err(ui_error)?;
        if themes.insert(theme.id.clone(), theme).is_some() {
            return Err(VnError::diagnostic(
                "ROUKAKU_UI_THEME_DUPLICATE",
                "theme id is already registered",
            ));
        }
    }
    for source in sources {
        if !source.path.ends_with(".screen.jsonc") {
            return Err(VnError::diagnostic(
                "ROUKAKU_UI_SCREEN_FORMAT",
                "UI screens must use .screen.jsonc",
            ));
        }
        let doc = Document::parse(&source.path, &source.text, false)?;
        for (index, value) in array(&doc.value, "views").iter().enumerate() {
            let at = format!("/views/{index}");
            let view = parse_view(&doc, value, &at, &styles)?;
            source_map.insert(view.source_id.clone(), doc.source(&at));
            if views.insert(view.id.clone(), view).is_some() {
                return Err(doc.error(
                    &at,
                    "ROUKAKU_UI_VIEW_DUPLICATE",
                    "UI view id is already declared",
                ));
            }
        }
        for (index, value) in array(&doc.value, "bindings").iter().enumerate() {
            let at = format!("/bindings/{index}");
            let binding = parse_binding(value);
            controller_ids.insert(binding.controller_id.clone());
            theme_ids.insert(binding.theme_id.clone());
            let (kind, key, profile) = binding_key(value);
            let duplicate = match (profile, kind) {
                (Some(profile), "command") => profile_scoped_bindings
                    .entry(profile)
                    .or_default()
                    .command_bindings
                    .insert(key, binding)
                    .is_some(),
                (Some(profile), "system_page") => profile_scoped_bindings
                    .entry(profile)
                    .or_default()
                    .system_page_bindings
                    .insert(key, binding)
                    .is_some(),
                (Some(profile), "surface") => profile_scoped_bindings
                    .entry(profile)
                    .or_default()
                    .surface_bindings
                    .insert(key, binding)
                    .is_some(),
                (None, "command") => command_bindings.insert(key, binding).is_some(),
                (None, "system_page") => system_page_bindings.insert(key, binding).is_some(),
                (None, "surface") => surface_bindings.insert(key, binding).is_some(),
                (None, "profile") => profile_bindings.insert(key, binding).is_some(),
                _ => unreachable!("schema-validated binding key"),
            };
            if duplicate {
                return Err(doc.error(
                    &at,
                    "ROUKAKU_UI_BINDING_CONFLICT",
                    "binding precedence level has multiple entries for the same key",
                ));
            }
            if let Some(id) = value.get("id").and_then(Value::as_str) {
                source_map.insert(id.to_string(), doc.source(&at));
            }
        }
        for (index, value) in array(&doc.value, "policies").iter().enumerate() {
            let at = format!("/policies/{index}");
            let policy = parse_policy(&doc, value, &at)?;
            if system_ui_profiles
                .insert(policy.profile_id.clone(), policy)
                .is_some()
            {
                return Err(doc.error(
                    &at,
                    "ROUKAKU_UI_SYSTEM_POLICY_DUPLICATE",
                    "UI profile declares more than one system policy",
                ));
            }
            if let Some(id) = value.get("id").and_then(Value::as_str) {
                source_map.insert(id.to_string(), doc.source(&at));
            }
        }
    }
    let bound_profiles = profile_scoped_bindings
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>();
    let policy_profiles = system_ui_profiles.keys().cloned().collect::<BTreeSet<_>>();
    if !policy_profiles.is_empty() && bound_profiles != policy_profiles {
        return Err(VnError::message(
            "ROUKAKU_UI_SYSTEM_POLICY_PROFILE_SET: every profile-scoped UI binding must have exactly one ui_policy",
        ));
    }

    for view in views.values() {
        theme_ids.insert(view.theme_id.clone());
    }
    if !views.is_empty() {
        for theme_id in &theme_ids {
            if !themes.contains_key(theme_id) {
                return Err(VnError::diagnostic(
                    "ROUKAKU_UI_THEME_MISSING",
                    format!("UI references unregistered theme {theme_id}"),
                ));
            }
        }
    }
    if controller_ids != controller_sources.keys().cloned().collect() {
        return Err(VnError::message(
            "ROUKAKU_UI_CONTROLLER_SOURCE_SET: every bound controller requires exactly one validated source",
        ));
    }
    for source in controller_sources.values() {
        roukaku_ui_core::validate_ui_source_text(source).map_err(ui_error)?;
    }
    for binding in command_bindings
        .values()
        .chain(system_page_bindings.values())
        .chain(surface_bindings.values())
        .chain(profile_bindings.values())
        .chain(profile_scoped_bindings.values().flat_map(|scoped| {
            scoped
                .command_bindings
                .values()
                .chain(scoped.system_page_bindings.values())
                .chain(scoped.surface_bindings.values())
        }))
    {
        if !views.contains_key(&binding.view_id) {
            return Err(VnError::diagnostic(
                "ROUKAKU_UI_BINDING_VIEW_MISSING",
                format!("binding references unknown view {}", binding.view_id),
            ));
        }
    }

    let mut ui_blueprints = UiBlueprintBundle {
        schema: "roukaku.ui_blueprint_bundle.v2".to_string(),
        views,
        hash: Hash256::from_sha256(&[]),
    };
    ui_blueprints.hash = hash_blueprints(&ui_blueprints)?;
    if !ui_blueprints.views.is_empty() {
        ui_blueprints.validate().map_err(ui_error)?;
    }
    let mut ui_bindings = UiBindingManifest {
        schema: "roukaku.ui_binding_manifest.v1".to_string(),
        command_bindings,
        system_page_bindings,
        surface_bindings,
        profile_bindings,
        profile_scoped_bindings,
        hash: Hash256::from_sha256(&[]),
    };
    ui_bindings.hash = hash_bindings(&ui_bindings)?;
    ui_bindings.validate().map_err(ui_error)?;
    let project_hash = hash_project(
        &story,
        &ui_blueprints,
        &ui_bindings,
        &themes,
        &controller_sources,
        &system_ui_profiles,
    )?;
    Ok(CompiledVnProject {
        schema: "roukaku.vn.compiled_project.v5".to_string(),
        project_hash,
        story,
        ui_blueprints,
        ui_bindings,
        ui_source_map: source_map,
        controller_ids,
        controller_sources,
        theme_ids,
        themes,
        system_ui_profiles,
    })
}
