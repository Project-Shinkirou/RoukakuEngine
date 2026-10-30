use std::collections::BTreeMap;

/// Built-in VN behavior modules are ordinary replaceable Luau modules.
pub fn default_vn_modules() -> BTreeMap<String, String> {
    BTreeMap::from([
        (
            "roukaku.vn.dialog".into(),
            r#"
return {
    show = function(node, data)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_modules_have_stable_ids_and_luau_sources() {
        let modules = default_vn_modules();
        assert_eq!(
            modules.keys().map(String::as_str).collect::<Vec<_>>(),
            [
                "roukaku.vn.choice",
                "roukaku.vn.dialog",
                "roukaku.vn.history",
                "roukaku.vn.menu",
                "roukaku.vn.skip",
                "roukaku.vn.stage",
            ]
        );
        assert!(modules.values().all(|source| source.contains("return")));
    }
}
