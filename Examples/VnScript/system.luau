--!strict
-- Module: story.system
local paragraphs: {[string]: () -> ()} = {}
local actions: {[string]: () -> ()} = {}

paragraphs["title"] = function()
    local selected = rk.call("roukaku.vn.menu", "show", "page.title", {kind = "title", policy = "roukaku.policy.standard", options = {{id = "system.start", text = "system.start"}, {id = "system.config.link", text = "system.config"}, {id = "system.gallery.link", text = "system.gallery"}}})
    if selected == "system.start" then
        rk.enter("story.main", "prologue")
    elseif selected == "system.config.link" then
        rk.enter("story.system", "config")
    elseif selected == "system.gallery.link" then
        rk.enter("story.system", "gallery")
    end
    local action = actions[selected]
    if action then action() end
end

paragraphs["config"] = function()
    local selected = rk.call("roukaku.vn.menu", "show", "page.config", {kind = "config", policy = "roukaku.policy.standard", bindings = {{key = "audio.bgm_volume", target = "system.audio.bgm_volume"}, {key = "text.speed", target = "system.text.speed"}}})
    local action = actions[selected]
    if action then action() end
end

paragraphs["gallery"] = function()
    local selected = rk.call("roukaku.vn.menu", "show", "page.gallery", {kind = "gallery", policy = "roukaku.policy.standard", source = {read_state = "cg_unlocks"}})
    local action = actions[selected]
    if action then action() end
end

paragraphs["replay"] = function()
    local selected = rk.call("roukaku.vn.menu", "show", "page.replay", {kind = "replay", policy = "roukaku.policy.standard", source = {backlog = "voice_replay"}})
    local action = actions[selected]
    if action then action() end
end

paragraphs["chart"] = function()
    local selected = rk.call("roukaku.vn.menu", "show", "page.chart", {kind = "chart", policy = "roukaku.policy.standard", source = {route_coverage = true}})
    local action = actions[selected]
    if action then action() end
end

return { paragraphs = paragraphs, actions = actions }
