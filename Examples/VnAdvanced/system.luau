--!strict
-- Module: story.system
local paragraphs: {[string]: () -> ()} = {}

paragraphs["title"] = function()
    rk.call("roukaku.vn.menu", "show", "system.title.config", {kind = "config"})
    rk.call("roukaku.vn.menu", "show", "system.title.load", {kind = "load"})
end

paragraphs["save"] = function()
    rk.call("roukaku.vn.menu", "show", "system.save.page", {kind = "save"})
end

paragraphs["load"] = function()
    rk.call("roukaku.vn.menu", "show", "system.load.page", {kind = "load"})
end

paragraphs["config"] = function()
    rk.call("roukaku.vn.menu", "show", "system.config.page", {kind = "config"})
end

paragraphs["backlog"] = function()
    rk.call("roukaku.vn.menu", "show", "system.backlog.page", {kind = "backlog"})
end

paragraphs["gallery"] = function()
    rk.call("roukaku.vn.menu", "show", "system.gallery.page", {kind = "gallery"})
end

paragraphs["replay"] = function()
    rk.call("roukaku.vn.menu", "show", "system.replay.page", {kind = "replay"})
end

paragraphs["route_chart"] = function()
    rk.call("roukaku.vn.menu", "show", "system.chart.page", {kind = "route_chart"})
end

paragraphs["voice_replay"] = function()
    rk.call("roukaku.vn.menu", "show", "system.voice.page", {kind = "voice_replay"})
end

paragraphs["localization_preview"] = function()
    rk.call("roukaku.vn.menu", "show", "system.localization.page", {kind = "localization_preview"})
end

return { paragraphs = paragraphs }
