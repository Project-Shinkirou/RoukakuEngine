use super::*;

pub(super) fn validate_widget_event(
    widget: &str,
    event: &str,
    ctx: &Context<'_>,
) -> Result<(), VnError> {
    let allowed: &[&str] = match widget {
        "button" | "panel" | "semantic_region" | "image" | "nine_slice" => &["activate"],
        "slider" | "toggle" | "select" => &["change", "activate"],
        "text_input" => &["change", "submit", "activate"],
        "modal" => &["dismiss"],
        "canvas" => &["activate", "node_activate", "node_hover"],
        _ => &[],
    };
    if allowed.contains(&event) {
        Ok(())
    } else {
        Err(ctx.error(
            "ROUKAKU_UI_WIDGET_EVENT_UNSUPPORTED",
            &format!("widget {widget} does not expose event {event}"),
        ))
    }
}

pub(super) fn validate_action(
    ctx: &Context<'_>,
    action: &str,
    arguments: &BTreeMap<String, UiValueExpr>,
) -> Result<(), VnError> {
    let required: &[&str] = match action {
        "vn.advance" | "vn.return_system" | "vn.request_exit" | "ui.close_modal" => &[],
        "vn.choose" => &["option_id"],
        "vn.open_system" | "vn.switch_system" => &["page"],
        "vn.request_save"
        | "vn.request_save_confirmed"
        | "vn.request_load"
        | "vn.request_delete_save" => &["slot_id"],
        "vn.set_config" => &["key", "value"],
        "vn.set_auto" => &["enabled"],
        "vn.set_skip" => &["mode"],
        "vn.set_reading_mode" => &["mode"],
        "vn.set_audio_enabled" => &["enabled"],
        "vn.invoke_system_action" => &["action_id"],
        "vn.replay_voice" => &["voice_id"],
        "vn.start_replay" => &["replay_id"],
        "vn.preview_gallery" => &["item_id"],
        "vn.request_route_jump" => &["node_id"],
        "vn.request_backlog_jump" => &["command_id"],
        "vn.submit_text" => &["input_id", "value"],
        "ui.open_modal" => &["view_id"],
        "ui.set_state" => &["key", "value"],
        _ => {
            return Err(ctx.error("ROUKAKU_UI_ACTION_UNKNOWN", "action is not registered"));
        }
    };
    if let Some(missing) = required.iter().find(|name| !arguments.contains_key(**name)) {
        return Err(ctx.error(
            "ROUKAKU_UI_ACTION_ARGUMENT_MISSING",
            &format!("action {action} requires argument {missing}"),
        ));
    }
    let optional: &[&str] = match action {
        "ui.open_modal" => &["model"],
        _ => &[],
    };
    if let Some(unexpected) = arguments
        .keys()
        .find(|name| !required.contains(&name.as_str()) && !optional.contains(&name.as_str()))
    {
        return Err(ctx.error(
            "ROUKAKU_UI_ACTION_ARGUMENT_UNKNOWN",
            &format!("action {action} does not declare argument {unexpected}"),
        ));
    }
    Ok(())
}

pub(super) fn infer_capabilities(node: &UiNodeBlueprint, out: &mut BTreeSet<UiCapability>) {
    match node.widget.as_str() {
        "virtual_list" => {
            out.insert(UiCapability::VirtualList);
        }
        "virtual_grid" => {
            out.insert(UiCapability::VirtualGrid);
        }
        "nine_slice" => {
            out.insert(UiCapability::NineSlice);
        }
        "canvas" => {
            out.insert(UiCapability::Canvas);
        }
        "text_input" => {
            out.insert(UiCapability::TextInput);
            out.insert(UiCapability::Ime);
        }
        _ => {}
    }
    if !node.events.is_empty() {
        out.insert(UiCapability::Pointer);
        out.insert(UiCapability::Keyboard);
        out.insert(UiCapability::GamepadNavigation);
    }
    for child in &node.children {
        infer_capabilities(child, out);
    }
}

pub(super) fn hash_blueprints(bundle: &UiBlueprintBundle) -> Result<Hash256, VnError> {
    bundle.compute_hash().map_err(ui_error)
}

pub(super) fn hash_bindings(bindings: &UiBindingManifest) -> Result<Hash256, VnError> {
    bindings.compute_hash().map_err(ui_error)
}

pub(super) fn hash_project(
    story: &CompiledStory,
    blueprints: &UiBlueprintBundle,
    bindings: &UiBindingManifest,
    themes: &BTreeMap<String, UiThemeManifest>,
    controller_sources: &BTreeMap<String, String>,
    system_ui_profiles: &BTreeMap<String, SystemUiProfilePolicy>,
) -> Result<Hash256, VnError> {
    Ok(Hash256::from_sha256(&postcard::to_allocvec(&(
        story.story_hash,
        blueprints.hash,
        bindings.hash,
        themes,
        controller_sources,
        system_ui_profiles,
    ))?))
}

pub(super) fn ui_error(error: roukaku_ui_core::UiValidationError) -> VnError {
    VnError::diagnostic(error.code(), error.to_string())
}
