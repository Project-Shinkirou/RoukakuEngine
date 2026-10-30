use std::{collections::BTreeSet, sync::Arc};

use mlua::{Function, Lua, LuaSerdeExt, Table, Value};
use roukaku_text::{TextLayoutMeasurement, TextLayoutRequest};

use super::{
    ChoiceOption, Context, FunctionRef, InputWait, NodeContent, NodeOccurrence, NodeRecord,
    ReplayCheckpoint, SegmentStatus, TimelineDeclaration, source_revision, timeline,
};
use roukaku_vn_script::StageCommand;

pub(super) fn install(lua: &Lua, context: &Context) -> mlua::Result<()> {
    let api = lua.create_table()?;
    macro_rules! bind {
        ($name:literal, $handler:expr) => {{
            let context = context.clone();
            api.set(
                $name,
                lua.create_function(move |lua, args| ($handler)(lua, &context, args))?,
            )?;
        }};
    }
    bind!(
        "_resolve",
        |_lua: &Lua, context: &Context, (module, name): (String, String)| {
            context.active()?;
            context.resolve(&FunctionRef { module, name })
        }
    );
    bind!("_wait", |lua: &Lua, context: &Context, action: String| {
        context.active()?;
        context.charge(action.len().saturating_add(128))?;
        let mut shared = context.shared.borrow_mut();
        if shared.paragraph.is_none()
            || shared.story_thread != Some(lua.current_thread().to_pointer() as usize)
            || action.is_empty()
            || shared.pending.is_some()
        {
            return Err(mlua::Error::runtime(
                "wait requires an active paragraph, nonempty action and no pending wait",
            ));
        }
        shared.pending = Some(InputWait {
            token: super::live_id()?,
            action,
        });
        Ok(())
    });
    bind!(
        "_enter",
        |lua: &Lua, context: &Context, (module, entry): (String, String)| {
            context.active()?;
            if module.is_empty()
                || entry.is_empty()
                || context.shared.borrow().story_thread
                    != Some(lua.current_thread().to_pointer() as usize)
            {
                return Err(mlua::Error::runtime(
                    "paragraph entry transition requires an active story and nonempty identity",
                ));
            }
            let module_table: Table = context.modules.get(module.as_str())?;
            let entries: Table = module_table.get("paragraphs")?;
            let target: Value = entries.get(entry.as_str())?;
            match target {
                Value::Function(_) => {}
                Value::Table(table) if matches!(table.get::<Value>("run")?, Value::Function(_)) => {}
                _ => return Err(mlua::Error::runtime("paragraph entry does not resolve to a function")),
            }
            context.charge(module.len().saturating_add(entry.len()))?;
            let mut shared = context.shared.borrow_mut();
            if shared.pending.is_some() || shared.pending_enter.is_some() {
                return Err(mlua::Error::runtime("paragraph transition conflicts with a pending wait"));
            }
            shared.pending_enter = Some(super::ParagraphId { module, entry });
            Ok(())
        }
    );
    bind!(
        "on_fence",
        |lua: &Lua,
         context: &Context,
         (fence, module, export, args): (String, String, String, Value)| {
            context.active()?;
            if fence.is_empty()
                || module.is_empty()
                || export.is_empty()
                || context.shared.borrow().story_thread
                    != Some(lua.current_thread().to_pointer() as usize)
            {
                return Err(mlua::Error::runtime(
                    "fence callbacks require an active story and nonempty identities",
                ));
            }
            context.resolve(&FunctionRef {
                module: module.clone(),
                name: export.clone(),
            })?;
            let args: serde_json::Value = context.data(lua, args)?;
            context.charge(fence.len().saturating_add(module.len()).saturating_add(export.len()))?;
            context
                .shared
                .borrow_mut()
                .fence_callbacks
                .entry(fence)
                .or_default()
                .push(super::FenceCallback {
                    module,
                    export,
                    args,
                });
            Ok(())
        }
    );
    bind!("random_u64", |_lua: &Lua, context: &Context, ()| {
        context.active()?;
        context.charge(64)?;
        let mut shared = context.shared.borrow_mut();
        let value = if shared.replaying {
            match shared.replay_source.get(shared.replay_cursor) {
                Some(super::ReplayEvent::RandomDraw { value }) => {
                    let value = *value;
                    shared.replay_cursor += 1;
                    value
                }
                _ => return Err(mlua::Error::runtime("ROUKAKU_VN_REPLAY_DIVERGED")),
            }
        } else {
            let mut state = shared.rng_state;
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            shared.rng_state = state;
            state & ((1_u64 << 53) - 1)
        };
        shared.replay.push(super::ReplayEvent::RandomDraw { value });
        Ok(value as i64)
    });
    bind!("once", |_lua: &Lua, context: &Context, id: String| {
        context.active()?;
        if id.is_empty() {
            return Err(mlua::Error::runtime(
                "side-effect identity must be nonempty",
            ));
        }
        context.charge(id.len().saturating_add(64))?;
        let mut shared = context.shared.borrow_mut();
        if shared.replaying {
            match shared.replay_source.get(shared.replay_cursor) {
                Some(super::ReplayEvent::SideEffect { id: expected }) if expected == &id => {
                    shared.replay_cursor += 1;
                    shared.replay.push(super::ReplayEvent::SideEffect { id });
                    Ok(false)
                }
                _ => Err(mlua::Error::runtime("ROUKAKU_VN_REPLAY_DIVERGED")),
            }
        } else {
            shared.replay.push(super::ReplayEvent::SideEffect { id });
            Ok(true)
        }
    });
    bind!(
        "host_query",
        |lua: &Lua, context: &Context, (operation, args): (String, Value)| {
            context.active()?;
            if operation.is_empty() {
                return Err(mlua::Error::runtime(
                    "host query operation must be nonempty",
                ));
            }
            let args: serde_json::Value = context.data(lua, args)?;
            let recorded = {
                let mut shared = context.shared.borrow_mut();
                if shared.replaying {
                    match shared.replay_source.get(shared.replay_cursor) {
                        Some(super::ReplayEvent::HostResult {
                            operation: expected,
                            result,
                        }) if expected == &operation => {
                            let result = result.clone();
                            shared.replay_cursor += 1;
                            Some(result)
                        }
                        _ => return Err(mlua::Error::runtime("ROUKAKU_VN_REPLAY_DIVERGED")),
                    }
                } else {
                    None
                }
            };
            let result = match recorded {
                Some(result) => result,
                None => context.host.query(&operation, args)?,
            };
            let lua_result = lua.to_value(&result)?;
            let result: serde_json::Value = context.data(lua, lua_result.clone())?;
            context
                .shared
                .borrow_mut()
                .replay
                .push(super::ReplayEvent::HostResult { operation, result });
            Ok(lua_result)
        }
    );
    bind!("line", |lua: &Lua,
                   context: &Context,
                   (node, data): (String, Value)| {
        let data = context.data(lua, data)?;
        record(lua, context, node, NodeContent::Line { data })
    });
    bind!(
        "choice",
        |lua: &Lua,
         context: &Context,
         (node, options, data): (String, Value, Option<Value>)| {
            let options: Vec<ChoiceOption> = context.data(lua, options)?;
            let data: serde_json::Value = match data {
                Some(data) => context.data(lua, data)?,
                None => serde_json::Value::Null,
            };
            let mut ids = BTreeSet::new();
            if options.is_empty()
                || options
                    .iter()
                    .any(|option| option.id.is_empty() || !ids.insert(&option.id))
            {
                return Err(mlua::Error::runtime(
                    "choice requires nonempty distinct option identities",
                ));
            }
            record(
                lua,
                context,
                node,
                NodeContent::Choice {
                    options,
                    selected: None,
                    data,
                },
            )
        }
    );
    bind!("choice_result", |lua: &Lua,
                            context: &Context,
                            (occurrence, selected): (
        Value,
        String
    )| {
        let occurrence: NodeOccurrence = context.data(lua, occurrence)?;
        context.charge(selected.len().saturating_add(64))?;
        let mut shared = context.shared.borrow_mut();
        let record = shared
            .records
            .iter_mut()
            .find(|record| record.occurrence == occurrence)
            .ok_or_else(|| mlua::Error::runtime("unknown choice occurrence"))?;
        match &mut record.content {
            NodeContent::Choice {
                options,
                selected: result,
                ..
            } if result.is_none() && options.iter().any(|option| option.id == selected) => {
                *result = Some(selected);
                Ok(())
            }
            _ => Err(mlua::Error::runtime(
                "choice result must select an available option exactly once",
            )),
        }
    });
    bind!("register_state", |lua: &Lua,
                             context: &Context,
                             (scope, key, initial): (
        String,
        String,
        Value
    )| {
        if key.is_empty() {
            return Err(mlua::Error::runtime("state key must be nonempty"));
        }
        let initial: serde_json::Value = context.data(lua, initial)?;
        context.charge(scope.len().saturating_add(key.len()))?;
        let mut shared = context.shared.borrow_mut();
        let value = shared
            .recoverable
            .scope_mut(&scope)?
            .entry(key)
            .or_insert(initial);
        lua.to_value(value)
    });
    bind!(
        "get_state",
        |lua: &Lua, context: &Context, (scope, key): (String, String)| {
            context.available()?;
            let mut shared = context.shared.borrow_mut();
            let value = shared
                .recoverable
                .scope_mut(&scope)?
                .get(&key)
                .ok_or_else(|| mlua::Error::runtime("state key is not registered"))?;
            lua.to_value(value)
        }
    );
    bind!("set_state", |lua: &Lua,
                        context: &Context,
                        (scope, key, value): (
        String,
        String,
        Value
    )| {
        let value = context.data(lua, value)?;
        let mut shared = context.shared.borrow_mut();
        let slot = shared
            .recoverable
            .scope_mut(&scope)?
            .get_mut(&key)
            .ok_or_else(|| mlua::Error::runtime("state key is not registered"))?;
        *slot = value;
        Ok(())
    });
    bind!(
        "present",
        |lua: &Lua,
         context: &Context,
         (node, action, data): (String, String, Value)| {
            let data: serde_json::Value = context.data(lua, data)?;
            context.active()?;
            if node.is_empty() {
                return Err(mlua::Error::runtime("presentation node identity must be nonempty"));
            }
            context.charge(node.len().saturating_add(action.len()).saturating_add(128))?;
            let command = stage_command(&action, data)?;
            context.host.present(&node, &command)?;
            context
                .shared
                .borrow_mut()
                .recoverable
                .scene
                .insert(node, serde_json::to_value(command).map_err(mlua::Error::external)?);
            Ok(())
        }
    );
    bind!("measure_text", |lua: &Lua,
                           context: &Context,
                           request: Value| {
        let request: TextLayoutRequest = context.data(lua, request)?;
        let layout = context
            .host
            .layout(&request)
            .map_err(mlua::Error::external)?;
        lua.to_value(&TextLayoutMeasurement::from(&layout))
    });
    bind!(
        "display_text",
        |lua: &Lua, context: &Context, (id, request): (String, Value)| {
            context.active()?;
            let request: TextLayoutRequest = context.data(lua, request)?;
            context.charge(id.len().saturating_add(64))?;
            let layout = Arc::new(
                context
                    .host
                    .layout(&request)
                    .map_err(mlua::Error::external)?,
            );
            let measurement = TextLayoutMeasurement::from(layout.as_ref());
            context.host.display_text(&id, layout)?;
            lua.to_value(&measurement)
        }
    );
    bind!("set_property", |_lua: &Lua,
                           context: &Context,
                           (target, property, value): (
        String,
        String,
        f64
    )| {
        timeline::set_property(context, &target, &property, value)
    });
    bind!("segment", |lua: &Lua,
                      context: &Context,
                      declaration: Value| {
        let declaration: TimelineDeclaration = context.data(lua, declaration)?;
        timeline::start(context, declaration)
    });
    bind!("complete", |_lua: &Lua, context: &Context, id: u64| {
        timeline::finish(context, id, SegmentStatus::Completed)
    });
    bind!("cancel", |_lua: &Lua, context: &Context, id: u64| {
        timeline::finish(context, id, SegmentStatus::Cancelled)
    });
    // Keep registry/wait implementation private. The Lua trampoline allows a
    // called project function to yield without crossing a Rust callback frame.
    let bootstrap: Function = lua
        .load(
            r#"
        return function(api)
            local resolve, wait, enter = api._resolve, api._wait, api._enter
            local yield = coroutine.yield
            api._resolve, api._wait, api._enter = nil, nil, nil
            api.call = function(module, name, ...)
                return resolve(module, name)(...)
            end
            api.enter = function(module, entry)
                enter(module, entry)
                return yield()
            end
            api.wait_input = function(action)
                wait(action)
                return yield()
            end
            return api
        end
    "#,
        )
        .set_name("@roukaku/authoring-api.luau")
        .eval()?;
    let api: Table = bootstrap.call(api)?;
    api.set_readonly(true);
    lua.globals().set("rk", api)?;
    // Freeze standard globals so project code cannot replace mechanism entry
    // points or contaminate subsequently loaded module environments.
    lua.sandbox(true)?;
    Ok(())
}


fn stage_command(action: &str, data: serde_json::Value) -> mlua::Result<StageCommand> {
    let variant = match action {
        "preload" => "preload",
        "stage" => "configure",
        "layer" => "declare_layer",
        "background" => "background",
        "show" => "show",
        "hide" => "hide",
        "clear_layer" => "clear_layer",
        "layer_visibility" => "set_layer_visibility",
        "backdrop" => "backdrop",
        "shade" => "shade",
        "skip_allowed" => "set_skip_allowed",
        "move" => "move",
        "camera" => "camera",
        "movie" => "movie",
        "voice" | "bgm" | "se" | "movie_audio" => "audio",
        "audio" => "audio_control",
        "audio_bus_enabled" => "set_audio_bus_enabled",
        "transition" => "transition",
        "shake" => "shake",
        "timeline" => "timeline",
        "effect" => "effect",
        _ => return Err(mlua::Error::runtime("unknown VN stage action")),
    };
    let mut tagged = serde_json::Map::new();
    tagged.insert(variant.into(), data);
    let command: StageCommand = serde_json::from_value(serde_json::Value::Object(tagged))
        .map_err(mlua::Error::external)?;
    if command.kind() != action {
        return Err(mlua::Error::runtime(
            "VN stage action does not match its command payload",
        ));
    }
    Ok(command)
}
fn record(lua: &Lua, context: &Context, node: String, content: NodeContent) -> mlua::Result<Value> {
    context.active()?;
    context.charge(node.len().saturating_add(256))?;
    if node.is_empty() {
        return Err(mlua::Error::runtime("node identity must be nonempty"));
    }
    let mut shared = context.shared.borrow_mut();
    let paragraph = shared
        .paragraph
        .clone()
        .ok_or_else(|| mlua::Error::runtime("nodes require an active paragraph"))?;
    let visit = shared.visits.entry(node.clone()).or_default();
    *visit = visit
        .checked_add(1)
        .ok_or_else(|| mlua::Error::runtime("node occurrence exhausted"))?;
    let occurrence = NodeOccurrence {
        paragraph,
        node,
        visit: *visit,
    };
    let event_index = shared.replay.len();
    let source_revision = source_revision(&occurrence.paragraph, &shared.module_revisions)?;
    shared.checkpoints.push(ReplayCheckpoint {
        occurrence: occurrence.clone(),
        event_index,
        source_revision,
    });
    let result = lua.to_value(&occurrence)?;
    let record = NodeRecord {
        occurrence,
        content,
    };
    shared.records.push(record.clone());
    drop(shared);
    context.host.node_recorded(&record)?;
    Ok(result)
}
