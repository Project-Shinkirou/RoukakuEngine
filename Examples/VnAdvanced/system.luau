--!strict
-- Module: story.system
local paragraphs: {[string]: () -> ()} = {}
local actions: {[string]: () -> ()} = {}

paragraphs["title"] = function()
    local selected = rk.call("roukaku.vn.menu", "show", "system.title.config", {kind = "config"})
    local action = actions[selected]
    if action then action() end
    local selected = rk.call("roukaku.vn.menu", "show", "system.title.load", {kind = "load"})
    local action = actions[selected]
    if action then action() end
end

paragraphs["save"] = function()
    local selected = rk.call("roukaku.vn.menu", "show", "system.save.page", {kind = "save"})
    local action = actions[selected]
    if action then action() end
end

paragraphs["load"] = function()
    local selected = rk.call("roukaku.vn.menu", "show", "system.load.page", {kind = "load"})
    local action = actions[selected]
    if action then action() end
end

paragraphs["config"] = function()
    local selected = rk.call("roukaku.vn.menu", "show", "system.config.page", {kind = "config"})
    local action = actions[selected]
    if action then action() end
end

paragraphs["backlog"] = function()
    local selected = rk.call("roukaku.vn.menu", "show", "system.backlog.page", {kind = "backlog"})
    local action = actions[selected]
    if action then action() end
end

paragraphs["gallery"] = function()
    local selected = rk.call("roukaku.vn.menu", "show", "system.gallery.page", {kind = "gallery"})
    local action = actions[selected]
    if action then action() end
end

paragraphs["replay"] = function()
    local selected = rk.call("roukaku.vn.menu", "show", "system.replay.page", {kind = "replay"})
    local action = actions[selected]
    if action then action() end
end

paragraphs["route_chart"] = function()
    local selected = rk.call("roukaku.vn.menu", "show", "system.chart.page", {kind = "route_chart"})
    local action = actions[selected]
    if action then action() end
end

paragraphs["voice_replay"] = function()
    local selected = rk.call("roukaku.vn.menu", "show", "system.voice.page", {kind = "voice_replay"})
    local action = actions[selected]
    if action then action() end
end

paragraphs["localization_preview"] = function()
    local selected = rk.call("roukaku.vn.menu", "show", "system.localization.page", {kind = "localization_preview"})
    local action = actions[selected]
    if action then action() end
end

return { paragraphs = paragraphs, actions = actions }
