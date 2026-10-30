use super::*;

pub(super) fn parse_view(
    doc: &Document<'_>,
    value: &Value,
    at: &str,
    styles: &[CompiledUiStyle],
) -> Result<UiViewBlueprint, VnError> {
    let model_schema = string(value, "model").to_string();
    if !matches!(
        model_schema.as_str(),
        "roukaku.vn.ui_model.message.v2"
            | "roukaku.vn.ui_model.choice.v1"
            | "roukaku.vn.ui_model.title.v1"
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
    ) {
        return Err(doc.error(
            &pointer(at, "model"),
            "ROUKAKU_UI_MODEL_SCHEMA_UNKNOWN",
            "UI view references an unknown model schema",
        ));
    }
    let theme = string(value, "theme");
    let style = styles.iter().find(|style| style.theme.id == theme);
    let root = parse_node(doc, &value["root"], &pointer(at, "root"), style)?;
    let mut capabilities = BTreeSet::new();
    infer_capabilities(&root, &mut capabilities);
    let view = UiViewBlueprint {
        id: string(value, "id").to_string(),
        source_id: value
            .get("source_id")
            .and_then(Value::as_str)
            .unwrap_or_else(|| string(value, "id"))
            .to_string(),
        model_schema,
        theme_id: theme.to_string(),
        required_capabilities: capabilities.into_iter().collect(),
        root,
    };
    validate_node_bindings(&view.root, &view.model_schema, None)
        .map_err(|error| doc.attach(at, error))?;
    view.validate()
        .map_err(ui_error)
        .map_err(|error| doc.attach(at, error))?;
    Ok(view)
}

fn parse_node(
    doc: &Document<'_>,
    value: &Value,
    at: &str,
    style: Option<&CompiledUiStyle>,
) -> Result<UiNodeBlueprint, VnError> {
    let id = string(value, "id");
    let kind = string(value, "kind");
    let ctx = Context {
        document: doc,
        pointer: at.to_string(),
    };
    let mut properties = BTreeMap::new();
    if let Some(style) = style {
        style.apply(value, &mut properties);
    }
    for (key, value) in value.as_object().expect("validated control") {
        if matches!(
            key.as_str(),
            "id" | "kind" | "source_id" | "children" | "events"
        ) || (kind != "select" && matches!(key.as_str(), "items" | "item_key" | "overscan"))
        {
            continue;
        }
        properties.insert(key.clone(), parse_expr(doc, value, &pointer(at, key))?);
    }
    if properties.contains_key("anchor")
        && properties.keys().any(|key| key.starts_with("position_"))
    {
        return Err(doc.error(
            &pointer(at, "anchor"),
            "ROUKAKU_UI_POSITION_CONFLICT",
            "absolute position and anchor cannot be specified on the same widget",
        ));
    }
    let repeat = if kind != "select" {
        value
            .get("items")
            .map(|items| {
                let key = value
                    .get("item_key")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        ctx.error(
                            "ROUKAKU_UI_WIDGET_REPEAT_REQUIRED",
                            "repeated widgets require item_key",
                        )
                    })?;
                let overscan = match value.get("overscan") {
                    None => 0,
                    Some(value) => value
                        .as_u64()
                        .and_then(|n| u16::try_from(n).ok())
                        .ok_or_else(|| {
                            doc.error(
                                &pointer(at, "overscan"),
                                "ROUKAKU_UI_OVERSCAN",
                                "overscan must be an unsigned 16-bit integer",
                            )
                        })?,
                };
                Ok::<_, VnError>(UiRepeatBinding {
                    items: parse_expr(doc, items, &pointer(at, "items"))?,
                    item_key_path: parse_path(key.trim_start_matches("$item."), &ctx)?,
                    overscan,
                })
            })
            .transpose()?
    } else {
        None
    };
    let mut events = Vec::new();
    for (index, value) in array(value, "events").iter().enumerate() {
        let at = pointer(&pointer(at, "events"), &index.to_string());
        let event_ctx = Context {
            document: doc,
            pointer: at.clone(),
        };
        let event = string(value, "event");
        validate_widget_event(kind, event, &event_ctx)?;
        let mut arguments = BTreeMap::new();
        if let Some(values) = value.get("arguments").and_then(Value::as_object) {
            for (name, value) in values {
                arguments.insert(
                    name.clone(),
                    parse_expr(doc, value, &pointer(&pointer(&at, "arguments"), name))?,
                );
            }
        }
        let action_id = string(value, "action").to_string();
        validate_action(&event_ctx, &action_id, &arguments)?;
        events.push(UiEventBinding {
            event: event.to_string(),
            action_id,
            arguments,
        });
    }
    let children = array(value, "children")
        .iter()
        .enumerate()
        .map(|(index, value)| {
            parse_node(
                doc,
                value,
                &pointer(&pointer(at, "children"), &index.to_string()),
                style,
            )
        })
        .collect::<Result<_, _>>()?;
    Ok(UiNodeBlueprint {
        source_id: value
            .get("source_id")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| format!("{}/{id}", doc.path)),
        local_id: id.to_string(),
        widget: kind.to_string(),
        properties,
        events,
        children,
        repeat,
    })
}

pub(super) fn parse_expr(
    doc: &Document<'_>,
    value: &Value,
    at: &str,
) -> Result<UiValueExpr, VnError> {
    let literal = match value {
        Value::String(value) => {
            for (prefix, root) in [
                ("$model.", UiBindingRoot::Model),
                ("$item.", UiBindingRoot::Item),
                ("$event.", UiBindingRoot::Event),
                ("$state.", UiBindingRoot::State),
            ] {
                if let Some(path) = value.strip_prefix(prefix) {
                    return Ok(UiValueExpr::Binding {
                        root,
                        path: parse_path(
                            path,
                            &Context {
                                document: doc,
                                pointer: at.to_string(),
                            },
                        )?,
                    });
                }
            }
            if value.starts_with('$') {
                return Err(doc.error(
                    at,
                    "ROUKAKU_UI_BINDING_ROOT",
                    "binding root must be $model/$item/$event/$state",
                ));
            }
            if let Some(key) = value.strip_prefix("l10n:") {
                return Ok(UiValueExpr::LocalizationKey {
                    key: key.to_string(),
                });
            }
            if let Some(asset_id) = value.strip_prefix("asset:") {
                return Ok(UiValueExpr::AssetRef {
                    asset_id: asset_id.to_string(),
                });
            }
            if let Some(token) = value.strip_prefix("theme:") {
                return Ok(UiValueExpr::ThemeToken {
                    token: token.to_string(),
                });
            }
            UiValue::String(value.clone())
        }
        Value::Bool(value) => UiValue::Bool(*value),
        Value::Null => UiValue::Null,
        Value::Number(value) => {
            if let Some(value) = value.as_i64() {
                UiValue::Integer(value)
            } else {
                UiValue::Number(value.as_f64().expect("JSON number"))
            }
        }
        _ => {
            return Err(doc.error(
                at,
                "ROUKAKU_UI_EXPRESSION",
                "UI expression must be a scalar",
            ));
        }
    };
    Ok(UiValueExpr::Literal { value: literal })
}

fn parse_path(value: &str, ctx: &Context<'_>) -> Result<Vec<String>, VnError> {
    let path: Vec<String> = value.split('.').map(str::to_string).collect();
    if path.iter().any(String::is_empty) {
        return Err(ctx.error(
            "ROUKAKU_UI_BINDING_PATH",
            "binding path contains an empty segment",
        ));
    }
    Ok(path)
}

pub(super) fn parse_binding(value: &Value) -> UiViewBinding {
    UiViewBinding {
        view_id: string(value, "view").to_string(),
        controller_id: string(value, "controller").to_string(),
        policy_bundle_id: string(value, "policy").to_string(),
        theme_id: string(value, "theme").to_string(),
    }
}

pub(super) fn binding_key(value: &Value) -> (&str, String, Option<String>) {
    for kind in ["command", "system_page", "surface"] {
        if let Some(key) = value.get(kind).and_then(Value::as_str) {
            return (
                kind,
                key.to_string(),
                value
                    .get("profile")
                    .and_then(Value::as_str)
                    .map(str::to_string),
            );
        }
    }
    ("profile", string(value, "profile").to_string(), None)
}

pub(super) fn parse_policy(
    doc: &Document<'_>,
    value: &Value,
    at: &str,
) -> Result<SystemUiProfilePolicy, VnError> {
    let list = |key| {
        array(value, key)
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .expect("validated policy identifier")
                    .to_string()
            })
            .collect::<Vec<_>>()
    };
    let save_slot_ids = list("save_slots");
    let quick_slot_id = value
        .get("quick_slot")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    if quick_slot_id
        .as_ref()
        .is_some_and(|slot| !save_slot_ids.contains(slot))
    {
        return Err(doc.error(
            &pointer(at, "quick_slot"),
            "ROUKAKU_UI_SYSTEM_QUICK_SLOT",
            "quick_slot must also be declared in save_slots",
        ));
    }
    let policy = SystemUiProfilePolicy {
        profile_id: string(value, "profile").to_string(),
        save_slot_ids,
        quick_slot_id,
        allowed_pages: list("allowed_pages")
            .iter()
            .map(|s| SystemPageKind::parse(s))
            .collect(),
        reading_modes: list("reading_modes")
            .iter()
            .map(|s| match s.as_str() {
                "hidden" => ReadingMode::Hidden,
                "manual" => ReadingMode::Manual,
                "fast_forward" => ReadingMode::FastForward,
                _ => unreachable!("schema validated reading mode"),
            })
            .collect(),
        audio_toggle: value["audio_toggle"].as_bool().expect("validated boolean"),
        save_completion: match string(value, "save_completion") {
            "stay" => SaveCompletionPolicy::Stay,
            "return_system" => SaveCompletionPolicy::ReturnSystem,
            _ => unreachable!("schema validated save policy"),
        },
        custom_action_ids: list("custom_actions").into_iter().collect(),
    };
    Ok(policy)
}
