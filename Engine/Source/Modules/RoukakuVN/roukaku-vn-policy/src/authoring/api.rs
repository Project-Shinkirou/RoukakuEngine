use std::{collections::BTreeSet, sync::Arc};

use mlua::{Function, Lua, LuaSerdeExt, Table, Value};
use roukaku_text::{TextLayoutMeasurement, TextLayoutRequest};

use super::{
    ChoiceOption, Context, FunctionRef, InputWait, NodeContent, NodeOccurrence, NodeRecord,
    SegmentStatus, TimelineDeclaration, timeline,
};

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
    bind!("line", |lua: &Lua,
                   context: &Context,
                   (node, data): (String, Value)| {
        let data = context.data(lua, data)?;
        record(lua, context, node, NodeContent::Line { data })
    });
    bind!(
        "choice",
        |lua: &Lua, context: &Context, (node, options): (String, Value)| {
            let options: Vec<ChoiceOption> = context.data(lua, options)?;
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
            local resolve, wait = api._resolve, api._wait
            local yield = coroutine.yield
            api._resolve, api._wait = nil, nil
            api.call = function(module, name, ...)
                return resolve(module, name)(...)
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
    let result = lua.to_value(&occurrence)?;
    shared.records.push(NodeRecord {
        occurrence,
        content,
    });
    Ok(result)
}
