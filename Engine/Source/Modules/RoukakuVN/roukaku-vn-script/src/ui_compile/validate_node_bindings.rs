use super::*;

pub(super) fn validate_node_bindings(
    node: &UiNodeBlueprint,
    model_schema: &str,
    inherited_item_collection: Option<&str>,
) -> Result<(), VnError> {
    let item_collection = node
        .repeat
        .as_ref()
        .and_then(|repeat| match &repeat.items {
            UiValueExpr::Binding {
                root: UiBindingRoot::Model,
                path,
            } => path.first().map(String::as_str),
            _ => None,
        })
        .or(inherited_item_collection);
    for expr in node
        .properties
        .values()
        .chain(
            node.events
                .iter()
                .flat_map(|event| event.arguments.values()),
        )
        .chain(node.repeat.iter().map(|repeat| &repeat.items))
    {
        validate_bound_expr(expr, model_schema, item_collection)?;
    }
    for child in &node.children {
        validate_node_bindings(child, model_schema, item_collection)?;
    }
    Ok(())
}

pub(super) fn validate_bound_expr(
    expr: &UiValueExpr,
    model_schema: &str,
    item_collection: Option<&str>,
) -> Result<(), VnError> {
    let UiValueExpr::Binding { root, path } = expr else {
        return Ok(());
    };
    let valid = match root {
        UiBindingRoot::Model => model_path_allowed(model_schema, path),
        UiBindingRoot::Item => item_collection
            .is_some_and(|collection| item_path_allowed(model_schema, collection, path)),
        UiBindingRoot::Event => path.as_slice() == ["value"] || path.as_slice() == ["node_id"],
        UiBindingRoot::State => !path.is_empty(),
    };
    if valid {
        Ok(())
    } else {
        Err(VnError::diagnostic(
            "ROUKAKU_UI_TYPED_BINDING_PATH",
            format!(
                "binding path {:?} is not declared by model schema {model_schema}",
                path
            ),
        ))
    }
}

pub(super) fn model_path_allowed(schema: &str, path: &[String]) -> bool {
    let first = path.first().map(String::as_str);
    if is_system_page_model_schema(schema)
        && matches!(
            first,
            Some(
                "underlay_kind"
                    | "underlay_title"
                    | "underlay_message"
                    | "underlay_choice"
                    | "underlay_text_key"
                    | "underlay_speaker_key"
            )
        )
    {
        return true;
    }
    match schema {
        "roukaku.vn.ui_model.message.v2" => matches!(
            first,
            Some(
                "command_id"
                    | "text_key"
                    | "speaker_key"
                    | "voice_id"
                    | "auto_enabled"
                    | "skip_mode"
                    | "visible_graphemes"
                    | "text_graphemes"
                    | "reveal_complete"
            )
        ),
        "roukaku.vn.ui_model.choice.v1" => {
            matches!(first, Some("choice_id" | "prompt_key" | "options"))
        }
        "roukaku.vn.ui_model.title.v1" => first == Some("can_continue"),
        "roukaku.vn.ui_model.quick_panel.v1" => {
            matches!(first, Some("auto_enabled" | "skip_mode"))
        }
        "roukaku.vn.ui_model.config.v1" => matches!(
            first,
            Some(
                "master_volume"
                    | "reading_hidden"
                    | "reading_manual"
                    | "reading_fast_forward"
                    | "audio_enabled"
                    | "audio_disabled"
                    | "text_speed"
                    | "auto_delay_ms"
                    | "high_contrast"
                    | "locale"
                    | "available_locales"
                    | "player_name"
            )
        ),
        "roukaku.vn.ui_model.save.v1" | "roukaku.vn.ui_model.load.v1" => first == Some("slots"),
        "roukaku.vn.ui_model.backlog.v1" | "roukaku.vn.ui_model.voice_replay.v1" => {
            first == Some("entries")
        }
        "roukaku.vn.ui_model.gallery.v1" | "roukaku.vn.ui_model.replay.v1" => {
            first == Some("items")
        }
        "roukaku.vn.ui_model.route_chart.v1" => first == Some("nodes"),
        "roukaku.vn.ui_model.localization_preview.v1" => {
            matches!(first, Some("locale" | "entries"))
        }
        "roukaku.vn.ui_model.text_input.v1" => first == Some("input"),
        "roukaku.vn.ui_model.system.v1" => true,
        _ => false,
    }
}

pub(super) fn is_system_page_model_schema(schema: &str) -> bool {
    matches!(
        schema,
        "roukaku.vn.ui_model.title.v1"
            | "roukaku.vn.ui_model.quick_panel.v1"
            | "roukaku.vn.ui_model.config.v1"
            | "roukaku.vn.ui_model.save.v1"
            | "roukaku.vn.ui_model.load.v1"
            | "roukaku.vn.ui_model.backlog.v1"
            | "roukaku.vn.ui_model.gallery.v1"
            | "roukaku.vn.ui_model.replay.v1"
            | "roukaku.vn.ui_model.voice_replay.v1"
            | "roukaku.vn.ui_model.route_chart.v1"
            | "roukaku.vn.ui_model.localization_preview.v1"
            | "roukaku.vn.ui_model.text_input.v1"
            | "roukaku.vn.ui_model.system.v1"
    )
}

pub(super) fn item_path_allowed(schema: &str, collection: &str, path: &[String]) -> bool {
    let first = path.first().map(String::as_str);
    match (schema, collection) {
        ("roukaku.vn.ui_model.choice.v1", "options") => {
            matches!(first, Some("option_id" | "text_key" | "enabled"))
        }
        ("roukaku.vn.ui_model.save.v1" | "roukaku.vn.ui_model.load.v1", "slots") => matches!(
            first,
            Some(
                "slot_id"
                    | "occupied"
                    | "thumbnail_asset"
                    | "has_thumbnail"
                    | "title_key"
                    | "timestamp_text"
                    | "playtime_text"
                    | "metadata_text"
                    | "can_write"
                    | "can_load"
                    | "migration_status"
            )
        ),
        ("roukaku.vn.ui_model.backlog.v1" | "roukaku.vn.ui_model.voice_replay.v1", "entries") => {
            matches!(
                first,
                Some(
                    "command_id"
                        | "text_key"
                        | "speaker_key"
                        | "voice_id"
                        | "has_voice"
                        | "can_jump"
                        | "read"
                )
            )
        }
        ("roukaku.vn.ui_model.gallery.v1" | "roukaku.vn.ui_model.replay.v1", "items") => matches!(
            first,
            Some("item_id" | "label_key" | "thumbnail_asset" | "has_thumbnail" | "unlocked")
        ),
        ("roukaku.vn.ui_model.localization_preview.v1", "entries") => {
            matches!(first, Some("entry_id" | "text_key"))
        }
        ("roukaku.vn.ui_model.route_chart.v1", "nodes") => matches!(
            first,
            Some("node_id" | "label_key" | "terminal" | "reached" | "x_milli" | "y_milli")
        ),
        _ => false,
    }
}
