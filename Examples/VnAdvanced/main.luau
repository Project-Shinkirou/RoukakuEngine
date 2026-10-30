--!strict
-- Module: story.main
local paragraphs: {[string]: () -> ()} = {}

paragraphs["prologue"] = function()
    rk.call("roukaku.vn.stage", "stage", "stage.opening", {viewport = {width = 1920, height = 1080}, safe_area = {width = 16, height = 9}})
    rk.call("roukaku.vn.stage", "layer", "layer.bg", {id = "bg", kind = "background", z = 0, blend = "normal"})
    rk.call("roukaku.vn.stage", "layer", "layer.characters", {id = "characters", kind = "sprite", z = 100, blend = "normal"})
    rk.call("roukaku.vn.stage", "layer", "layer.video_fx", {id = "video_fx", kind = "video", z = 200, blend = "screen", clip = "stage"})
    rk.call("roukaku.vn.stage", "layer", "layer.ui", {id = "ui", kind = "text", z = 900, blend = "normal", input = "message"})
    rk.call("roukaku.vn.stage", "background", "bg.room", {asset = "bg.room", layer = "bg", preset = "soft_fade", duration_ms = 300, interrupt = "replace_from_current"})
    rk.call("roukaku.vn.stage", "show", "hero.show", {id = "hero", asset = rk.host_query("vn.character.resolve", {id = "hero", pose = "normal"}), pose = "normal", layer = "characters", placement = "center", fit = "contain_height", opacity = {millionths = 1000000}, preset = "hero_enter", interrupt = "replace_from_current"})
    rk.call("roukaku.vn.stage", "camera", "camera.push", {target = "main", x = {millionths = 0}, y = {millionths = 0}, zoom = {millionths = 1060000}, rotation = {millionths = 0}, duration_ms = 480, preset = "slow_push"})
    rk.call("roukaku.vn.stage", "movie", "movie.light", {layer = "video_fx", asset = "movie.light_leak", alpha = {millionths = 350000}, loop_mode = "loop", ["end"] = "continue", fallback = "flat_fade", interrupt = "reject"})
    rk.call("roukaku.vn.stage", "voice", "voice.hero.0001", {id = "voice.hero.0001", bus = "voice", asset = "voice.hero.0001", looped = false, fade_ms = 0, sync = "text"})
    rk.call("roukaku.vn.dialog", "show", "line.hello", {key = "prologue.hello", speaker = "hero", window = "main", effects = {{text = "cinematic.reveal", lip_sync = true, filter = "soft_glow"}}})
    rk.call("roukaku.vn.stage", "wait", "wait.voice", "voice_end")
    rk.call("roukaku.vn.stage", "transition", "trans.flash", {preset = "flash_soft", duration_ms = 220})
    local selected = rk.call("roukaku.vn.choice", "select", "choice.where", {{id = "choice.library", text = "choice.library"}, {id = "choice.rooftop", text = "choice.rooftop"}})
    if selected == "choice.library" then
        rk.enter("story.main", "library")
    elseif selected == "choice.rooftop" then
        rk.enter("story.main", "rooftop")
    end
end

paragraphs["library"] = function()
    rk.call("roukaku.vn.stage", "background", "bg.library", {asset = "bg.library", layer = "bg", preset = "crossfade", duration_ms = 420, interrupt = "replace_from_current"})
    rk.call("roukaku.vn.stage", "hide", "hero.hide", {id = "hero", preset = "fade", duration_ms = 180, interrupt = "replace_from_current"})
    rk.call("roukaku.vn.stage", "bgm", "bgm.library", {id = "bgm.library", bus = "bgm", asset = "bgm.library", looped = true, fade_ms = 600, sync = "none"})
    rk.call("roukaku.vn.dialog", "show", "line.library", {key = "library.entry", speaker = "narrator"})
    rk.call("roukaku.vn.menu", "show", "system.save.open", {kind = "save"})
end

paragraphs["rooftop"] = function()
    rk.call("roukaku.vn.stage", "background", "bg.rooftop", {asset = "bg.rooftop", layer = "bg", preset = "crossfade", duration_ms = 420, interrupt = "replace_from_current"})
    rk.call("roukaku.vn.stage", "shake", "camera.shake", {target = "camera.main", strength = {millionths = 3000000}, duration_ms = 160})
    rk.call("roukaku.vn.stage", "se", "se.wind", {id = "se.wind", bus = "se", asset = "se.wind", looped = false, fade_ms = 0, sync = "none"})
    rk.call("roukaku.vn.dialog", "show", "line.rooftop", {key = "rooftop.entry", speaker = "narrator"})
    rk.call("roukaku.vn.menu", "show", "system.chart.open", {kind = "route_chart"})
end

return { paragraphs = paragraphs }
