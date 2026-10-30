use std::collections::BTreeMap;

/// Built-in VN behavior modules are ordinary replaceable Luau modules.
pub fn default_vn_modules() -> BTreeMap<String, String> {
    BTreeMap::from([
        (
            "roukaku.vn.action".into(),
            r#"
return {
    run = function(node, command, params)
        rk.host_query("vn.action.start", { node = node, command = command, params = params or {} })
        return rk.wait_input("action:" .. node)
    end,
}
"#
            .into(),
        ),
        (
            "roukaku.vn.dialog".into(),
            r#"
local function effect_params(effect)
    local params = {}
    for key, value in effect do
        if key ~= "command" then
            params[key] = value
        end
    end
    return params
end

return {
    show = function(node, data)
        if data.effects then
            for index, effect in data.effects do
                rk.call("roukaku.vn.action", "run", node .. ".effect." .. index, effect.command, effect_params(effect))
            end
        end
        local occurrence = rk.line(node, data)
        rk.wait_input("advance")
        return occurrence
    end,
}
"#
            .into(),
        ),
        (
            "roukaku.vn.choice".into(),
            r#"
return {
    select = function(node, options, data)
        local occurrence = rk.choice(node, options, data or {})
        local selected = rk.wait_input("choose")
        rk.choice_result(occurrence, selected)
        return selected
    end,
}
"#
            .into(),
        ),
        (
            "roukaku.vn.menu".into(),
            r#"
return {
    show = function(node, data)
        local prepared = rk.host_query("vn.menu.prepare", data)
        local occurrence = rk.choice(node, prepared.options, data)
        local selected = rk.wait_input("menu.select")
        rk.choice_result(occurrence, selected)
        return selected
    end,
}
"#
            .into(),
        ),
        (
            "roukaku.vn.stage".into(),
            r#"
local actions = {
    "preload", "stage", "layer", "background", "show", "hide",
    "clear_layer", "layer_visibility", "backdrop", "shade", "skip_allowed",
    "move", "camera", "movie", "voice", "bgm", "se", "movie_audio",
    "audio", "audio_bus_enabled", "transition", "shake", "timeline", "effect",
}
local module = {}
for _, name in actions do
    local action = name
    module[action] = function(node, data)
        return rk.present(node, action, data)
    end
end
module.wait = function(node, fence)
    return rk.wait_input("stage:" .. fence)
end
return module
"#
            .into(),
        ),
        (
            "roukaku.vn.history".into(),
            r#"
return {
    read = function()
        return rk.host_query("vn.history.read", {})
    end,
    close = function()
        return rk.wait_input("history.close")
    end,
}
"#
            .into(),
        ),
        (
            "roukaku.vn.skip".into(),
            r#"
return {
    enabled = function()
        return rk.host_query("vn.skip.enabled", {})
    end,
    should_advance = function(node)
        return rk.host_query("vn.skip.should_advance", { node = node })
    end,
}
"#
            .into(),
        ),
    ])
}
