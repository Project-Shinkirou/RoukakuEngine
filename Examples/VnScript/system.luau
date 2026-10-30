--!strict
-- Module: story.system
local paragraphs: {[string]: () -> ()} = {}

paragraphs["title"] = function()
    local selected = rk.call("roukaku.vn.menu", "show", "page.title", {kind = "title", policy = "roukaku.policy.standard", options = {{id = "system.start", text = "system.start"}, {id = "system.config.link", text = "system.config"}, {id = "system.gallery.link", text = "system.gallery"}}})
    if selected == "system.start" then
        rk.enter("story.main", "prologue")
    elseif selected == "system.config.link" then
        rk.enter("story.system", "config")
    elseif selected == "system.gallery.link" then
        rk.enter("story.system", "gallery")
    end
end

paragraphs["config"] = function()
    rk.call("roukaku.vn.menu", "show", "page.config", {kind = "config", policy = "roukaku.policy.standard", bindings = {{key = "audio.bgm_volume", target = "system.audio.bgm_volume"}, {key = "text.speed", target = "system.text.speed"}}})
end

paragraphs["gallery"] = function()
    rk.call("roukaku.vn.menu", "show", "page.gallery", {kind = "gallery", policy = "roukaku.policy.standard", source = {read_state = "cg_unlocks"}})
end

paragraphs["replay"] = function()
    rk.call("roukaku.vn.menu", "show", "page.replay", {kind = "replay", policy = "roukaku.policy.standard", source = {backlog = "voice_replay"}})
end

paragraphs["chart"] = function()
    rk.call("roukaku.vn.menu", "show", "page.chart", {kind = "chart", policy = "roukaku.policy.standard", source = {route_coverage = true}})
end

return { paragraphs = paragraphs }
