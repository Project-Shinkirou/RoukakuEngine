--!strict
-- Module: story.main
local paragraphs: {[string]: () -> ()} = {}

paragraphs["prologue"] = function()
    rk.call("roukaku.vn.stage", "stage", "experience.stage", {viewport = {width = 1920, height = 1080}, safe_area = {width = 16, height = 9}})
    rk.call("roukaku.vn.stage", "layer", "experience.layer.bg", {id = "bg", kind = "background", z = 0, blend = "normal"})
    rk.call("roukaku.vn.stage", "layer", "experience.layer.character", {id = "character", kind = "sprite", z = 100, blend = "normal", clip = "stage"})
    rk.call("roukaku.vn.stage", "layer", "experience.layer.foreground", {id = "foreground", kind = "sprite", z = 150, blend = "normal", clip = "stage"})
    rk.call("roukaku.vn.stage", "layer", "experience.layer.effects", {id = "effects", kind = "effect", z = 250, blend = "normal", clip = "stage"})
    rk.call("roukaku.vn.stage", "layer", "experience.layer.video", {id = "video", kind = "video", z = 200, blend = "normal", clip = "stage"})
    rk.call("roukaku.vn.stage", "layer", "experience.layer.ui", {id = "ui", kind = "text", z = 900, blend = "normal", input = "message"})
    rk.call("roukaku.vn.stage", "preload", "experience.preload.background", {asset = "asset:/key-art/title-background"})
    rk.enter("story.main", "experience_read")
end

paragraphs["experience_menu"] = function()
    rk.call("roukaku.vn.stage", "camera", "experience.menu.camera", {target = "main", x = {millionths = 0}, y = {millionths = 0}, zoom = {millionths = 1000000}, rotation = {millionths = 0}, duration_ms = 0})
    rk.call("roukaku.vn.stage", "clear_layer", "experience.clear.foreground", {layer = "foreground", duration_ms = 0, interrupt = "replace_from_current"})
    rk.call("roukaku.vn.stage", "background", "experience.background", {asset = "asset:/key-art/title-background", layer = "bg", duration_ms = 0, interrupt = "replace_from_current"})
    rk.call("roukaku.vn.stage", "clear_layer", "experience.clear.character", {layer = "character", duration_ms = 0, interrupt = "replace_from_current"})
    rk.call("roukaku.vn.stage", "clear_layer", "experience.clear.video", {layer = "video", duration_ms = 0, interrupt = "replace_from_current"})
    local selected = rk.call("roukaku.vn.choice", "select", "experience.menu", {{id = "experience.option.story", text = "experience.story"}, {id = "experience.option.read", text = "experience.read"}, {id = "experience.option.stage", text = "experience.stage"}, {id = "experience.option.media", text = "experience.media"}, {id = "experience.option.save", text = "experience.save"}, {id = "experience.option.title", text = "experience.title"}}, {key = "experience.welcome"})
    if selected == "experience.option.story" then
        rk.enter("story.signal_in_the_glass_rain", "scn_common_01_glass_train")
    elseif selected == "experience.option.read" then
        rk.enter("story.main", "experience_read")
    elseif selected == "experience.option.stage" then
        rk.enter("story.main", "experience_stage")
    elseif selected == "experience.option.media" then
        rk.enter("story.main", "experience_media")
    elseif selected == "experience.option.save" then
        rk.enter("story.main", "experience_save")
    elseif selected == "experience.option.title" then
        rk.enter("story.main", "experience_title")
    end
end

paragraphs["experience_read"] = function()
    rk.call("roukaku.vn.stage", "background", "experience.read.background", {asset = "asset:/background/metro-station", layer = "bg", duration_ms = 0, interrupt = "replace_from_current"})
    rk.call("roukaku.vn.dialog", "show", "experience.read.first", {key = "experience.read.first", window = "main"})
    rk.call("roukaku.vn.dialog", "show", "experience.read.second", {key = "experience.read.second", window = "main"})
    rk.enter("story.main", "experience_stage")
end

paragraphs["experience_stage"] = function()
    rk.call("roukaku.vn.stage", "show", "experience.actor.show", {id = "experience.actor", asset = "asset:/character/sprites/lin-yao/neutral-center", layer = "character", placement = "left", fit = "contain_height", opacity = {millionths = 200000}, interrupt = "replace_from_current"})
    rk.call("roukaku.vn.stage", "show", "experience.witness.show", {id = "experience.witness", asset = "asset:/character/sprites/zhou-heng/guarded-center", layer = "foreground", placement = "right", fit = "contain_height", opacity = {millionths = 750000}, interrupt = "replace_from_current"})
    rk.segment({duration = 4, tracks = {{target = "experience.actor", property = "x", keys = {{time = 0, value = 320}, {time = 4, value = 760}}}, {target = "camera.main", property = "zoom", keys = {{time = 0, value = 1}, {time = 4, value = 1.12}}}}})
    local fade = rk.segment({duration = 20, tracks = {{target = "experience.actor", property = "opacity", keys = {{time = 0, value = 0.2}, {time = 20, value = 1}}}}})
    rk.call("roukaku.vn.dialog", "show", "experience.stage.first", {key = "experience.stage.first", window = "main"})
    rk.call("roukaku.vn.stage", "shade", "experience.signal.dim", {color = {18, 34, 56, 255}, opacity = {millionths = 200000}})
    rk.cancel(fade)
    fade = rk.segment({duration = 8, tracks = {{target = "experience.actor", property = "opacity", keys = {{time = 0, value = 1}, {time = 8, value = 0.35}}}}})
    rk.call("roukaku.vn.dialog", "show", "experience.stage.second", {key = "experience.stage.second", window = "main"})
    rk.cancel(fade)
    rk.call("roukaku.vn.stage", "timeline", "experience.fade.finish", {start = {id = "experience.finish", join = "block", tracks = {{easing = "linear", target = "experience.actor", property = "opacity", keyframes = {{time_ms = 0, value = {millionths = 350000}}, {time_ms = 800, value = {millionths = 1000000}}}}}, fence = "experience.finished", budget_us = 2000}})
    rk.call("roukaku.vn.stage", "wait", "experience.fade.finish.wait", "experience.finished")
    rk.call("roukaku.vn.stage", "shade", "experience.signal.clear", {color = {18, 34, 56, 255}, opacity = {millionths = 0}})
    rk.call("roukaku.vn.dialog", "show", "experience.stage.third", {key = "experience.stage.third", window = "main"})
    rk.enter("story.main", "experience_save")
end

paragraphs["experience_media"] = function()
    rk.call("roukaku.vn.dialog", "show", "experience.media.before", {key = "experience.media.before", window = "main"})
    rk.call("roukaku.vn.stage", "se", "experience.media.sound", {id = "experience.media.sound", bus = "se", asset = "asset:/se/distant-thunder", looped = false, fade_ms = 0, sync = "none"})
    rk.call("roukaku.vn.stage", "movie", "experience.media.movie", {layer = "video", asset = "asset:/movie/rain-signal-loop", alpha = {millionths = 1000000}, loop_mode = "once", ["end"] = "wait", fence = "experience.movie.done", fallback = "asset:/key-art/title-background", interrupt = "replace_from_current"})
    rk.call("roukaku.vn.stage", "wait", "experience.media.movie.wait", "experience.movie.done")
    rk.call("roukaku.vn.dialog", "show", "experience.media.after", {key = "experience.media.after", window = "main"})
    rk.enter("story.main", "experience_menu")
end

paragraphs["experience_save"] = function()
    rk.call("roukaku.vn.stage", "show", "experience.keepsake.show", {id = "experience.witness", asset = "asset:/character/sprites/zhou-heng/relieved-center", layer = "foreground", placement = "right", fit = "contain_height", opacity = {millionths = 250000}, interrupt = "replace_from_current"})
    rk.call("roukaku.vn.stage", "timeline", "experience.keepsake.fade", {start = {id = "experience.keepsake.fade", join = "fire_and_forget", tracks = {{easing = "linear", target = "experience.witness", property = "opacity", keyframes = {{time_ms = 0, value = {millionths = 250000}}, {time_ms = 60000, value = {millionths = 1000000}}}}}, budget_us = 2000}})
    rk.call("roukaku.vn.dialog", "show", "experience.save.before", {key = "experience.save.before", window = "main"})
    rk.call("roukaku.vn.dialog", "show", "experience.save.after", {key = "experience.save.after", window = "main"})
    local selected = rk.call("roukaku.vn.choice", "select", "experience.save.choice", {{id = "experience.save.rain", text = "experience.media"}, {id = "experience.save.back", text = "experience.back"}}, {key = "experience.read.question"})
    if selected == "experience.save.rain" then
        rk.enter("story.main", "experience_media")
    elseif selected == "experience.save.back" then
        rk.enter("story.main", "experience_menu")
    end
end

paragraphs["experience_title"] = function()
    rk.call("roukaku.vn.menu", "show", "experience.title.page", {kind = "title", policy = "roukaku.policy.standard"})
    rk.enter("story.main", "experience_menu")
end

return { paragraphs = paragraphs }
