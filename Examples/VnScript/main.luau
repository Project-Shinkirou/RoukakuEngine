--!strict
-- Module: story.main
local paragraphs: {[string]: () -> ()} = {}

local function voice_ready()
    rk.segment({
        duration = 0.24,
        tracks = {{
            target = "camera.main",
            property = "zoom",
            keys = {
                {time = 0, value = 1},
                {time = 0.12, value = 1.15},
                {time = 0.24, value = 1},
            },
        }},
    })
end

paragraphs.prologue = function()
    rk.register_state("variables", "project.affinity", 0)
    rk.call("roukaku.vn.stage", "background", "bg.room", {
        asset = "bg.room", layer = "bg", preset = "soft_fade",
        duration_ms = 300, interrupt = "replace_from_current",
    })
    rk.call("roukaku.vn.stage", "show", "hero.enter", {
        id = "hero",
        asset = rk.host_query("vn.character.resolve", {id = "hero", pose = "normal"}),
        pose = "normal", layer = "characters", placement = "center",
        fit = "contain_height", opacity = {millionths = 1000000},
        preset = "hero_enter", interrupt = "replace_from_current",
    })
    rk.call("roukaku.vn.stage", "bgm", "timeline.intro.bgm", {
        id = "timeline.intro.bgm", bus = "bgm", asset = "bgm.morning",
        looped = true, fade_ms = 600, sync = "none",
    })
    -- The voice-ready task never blocks the paragraph that starts the voice.
    rk.on_fence("voice.ready", "story.main", "voice_ready", {})
    rk.call("roukaku.vn.stage", "voice", "line.hello.voice", {
        id = "line.hello.voice", bus = "voice", asset = "voice.hero.0001",
        looped = false, fade_ms = 0, sync = {fence = "voice.ready"},
    })
    rk.call("roukaku.vn.dialog", "show", "line.hello", {
        key = "prologue.hello", speaker = "hero", voice = "voice.hero.0001",
        effects = {{command = "roukaku.cinematic.reveal_text", speed = 36, lip_sync = true}},
    })
    rk.set_state("variables", "project.affinity", rk.get_state("variables", "project.affinity") + 1)
    local selected = rk.call("roukaku.vn.choice", "select", "choice.where", {
        {id = "choice.library", text = "choice.library"},
        {id = "choice.rooftop", text = "choice.rooftop"},
    }, {key = "prologue.where"})
    if selected == "choice.library" then
        rk.enter("story.main", "library")
    elseif selected == "choice.rooftop" then
        rk.enter("story.main", "rooftop")
    end
end

paragraphs.library = function()
    rk.register_state("variables", "project.affinity", 0)
    if rk.get_state("variables", "project.affinity") >= 1 then
        rk.call("roukaku.vn.dialog", "show", "line.library", {
            key = "library.followup", speaker = "hero", voice = "voice.hero.0002",
        })
        rk.enter("story.main", "ending.good")
    end
end

paragraphs.rooftop = function()
    rk.call("roukaku.vn.dialog", "show", "line.rooftop", {
        key = "rooftop.line", speaker = "hero", voice = "voice.hero.0003",
    })
    rk.enter("story.main", "ending.normal")
end

return {paragraphs = paragraphs, voice_ready = voice_ready}
