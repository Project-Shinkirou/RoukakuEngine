--!strict
-- Module: story.system
local paragraphs: {[string]: () -> ()} = {}
local actions: {[string]: () -> ()} = {}

actions["nativevn.story.start"] = function()
    rk.enter("story.signal_in_the_glass_rain", "scn_common_01_glass_train")
end

actions["nativevn.experience.open"] = function()
    rk.enter("story.main", "experience_read")
end

paragraphs["title"] = function()
    local selected = rk.call("roukaku.vn.menu", "show", "page.title", {kind = "title", policy = "roukaku.policy.standard"})
    local action = actions[selected]
    if action then action() end
end

paragraphs["save"] = function()
    local selected = rk.call("roukaku.vn.menu", "show", "page.save", {kind = "save", policy = "roukaku.policy.standard"})
    local action = actions[selected]
    if action then action() end
end

paragraphs["load"] = function()
    local selected = rk.call("roukaku.vn.menu", "show", "page.load", {kind = "load", policy = "roukaku.policy.standard"})
    local action = actions[selected]
    if action then action() end
end

paragraphs["config"] = function()
    local selected = rk.call("roukaku.vn.menu", "show", "page.config", {kind = "config", policy = "roukaku.policy.standard"})
    local action = actions[selected]
    if action then action() end
end

paragraphs["backlog"] = function()
    local selected = rk.call("roukaku.vn.menu", "show", "page.backlog", {kind = "backlog", policy = "roukaku.policy.standard"})
    local action = actions[selected]
    if action then action() end
end

paragraphs["gallery"] = function()
    local selected = rk.call("roukaku.vn.menu", "show", "page.gallery", {kind = "gallery", policy = "roukaku.policy.standard"})
    local action = actions[selected]
    if action then action() end
end

paragraphs["replay"] = function()
    local selected = rk.call("roukaku.vn.menu", "show", "page.replay", {kind = "replay", policy = "roukaku.policy.standard"})
    local action = actions[selected]
    if action then action() end
end

paragraphs["voice_replay"] = function()
    local selected = rk.call("roukaku.vn.menu", "show", "page.voice_replay", {kind = "voice_replay", policy = "roukaku.policy.standard"})
    local action = actions[selected]
    if action then action() end
end

paragraphs["route_chart"] = function()
    local selected = rk.call("roukaku.vn.menu", "show", "page.route_chart", {kind = "route_chart", policy = "roukaku.policy.standard"})
    local action = actions[selected]
    if action then action() end
end

paragraphs["localization_preview"] = function()
    local selected = rk.call("roukaku.vn.menu", "show", "page.localization_preview", {kind = "localization_preview", policy = "roukaku.policy.standard"})
    local action = actions[selected]
    if action then action() end
end

paragraphs["quick_panel"] = function()
    local selected = rk.call("roukaku.vn.menu", "show", "page.quick_panel", {kind = "quick_panel", policy = "roukaku.policy.standard"})
    local action = actions[selected]
    if action then action() end
end

return { paragraphs = paragraphs, actions = actions }
