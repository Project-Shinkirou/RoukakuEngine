--!strict
-- Module: story.system
local paragraphs: {[string]: () -> ()} = {}
local actions: {[string]: () -> ()} = {}

actions["tsui.classic.test.open"] = function()
    rk.enter("story.system", "custom")
end

actions["tsui.classic.test.mode.ayana"] = function()
    rk.register_state("variables", "global.mode", 0)
    rk.set_state("variables", "global.mode", 0)
end

actions["tsui.classic.test.mode.kotomi"] = function()
    rk.register_state("variables", "global.mode", 1)
    rk.set_state("variables", "global.mode", 1)
end

actions["tsui.classic.test.panty.off"] = function()
    rk.register_state("variables", "global.panty", 0)
    rk.set_state("variables", "global.panty", 0)
end

actions["tsui.classic.test.panty.on"] = function()
    rk.register_state("variables", "global.panty", 1)
    rk.set_state("variables", "global.panty", 1)
end

actions["tsui.classic.test.jump.y13"] = function()
    rk.enter("director.y", "0020")
end

actions["tsui.classic.test.jump.y14"] = function()
    rk.enter("director.y", "0246")
end

actions["tsui.classic.test.jump.y15"] = function()
    rk.enter("director.y", "0405")
end

actions["tsui.classic.test.jump.y16"] = function()
    rk.enter("director.y", "0524")
end

actions["tsui.classic.test.jump.y17"] = function()
    rk.enter("director.y", "0667")
end

actions["tsui.classic.test.jump.y17_18"] = function()
    rk.enter("director.y", "0775")
end

actions["tsui.classic.test.jump.y18_19"] = function()
    rk.enter("director.y", "0922")
end

actions["tsui.classic.test.jump.y20"] = function()
    rk.enter("director.y", "1030")
end

actions["tsui.classic.test.jump.k12"] = function()
    rk.enter("director.k", "0015")
end

actions["tsui.classic.test.jump.k13"] = function()
    rk.enter("director.k", "0126")
end

actions["tsui.classic.test.jump.k14"] = function()
    rk.enter("director.k", "0249")
end

actions["tsui.classic.test.jump.k15"] = function()
    rk.enter("director.k", "0312")
end

actions["tsui.classic.test.jump.k16"] = function()
    rk.enter("director.k", "0387")
end

actions["tsui.classic.test.jump.k1701"] = function()
    rk.enter("director.k", "0456")
end

actions["tsui.classic.test.jump.k1801"] = function()
    rk.enter("director.k", "0543")
end

actions["tsui.classic.test.jump.z09"] = function()
    rk.enter("director.z", "0015")
end

actions["tsui.classic.test.jump.z10"] = function()
    rk.enter("director.z", "0060")
end

actions["tsui.classic.test.jump.z11"] = function()
    rk.enter("director.z", "0087")
end

actions["tsui.classic.test.jump.z12"] = function()
    rk.enter("director.z", "0126")
end

actions["tsui.classic.test.jump.t13"] = function()
    rk.enter("director.t", "0018")
end

actions["tsui.classic.test.jump.t14"] = function()
    rk.enter("director.t", "0111")
end

actions["tsui.classic.test.jump.t15"] = function()
    rk.enter("director.t", "0216")
end

actions["tsui.classic.test.jump.t16"] = function()
    rk.enter("director.t", "0309")
end

actions["tsui.classic.test.jump.t17"] = function()
    rk.enter("director.t", "0408")
end

actions["tsui.classic.test.jump.t17_18"] = function()
    rk.enter("director.t", "0453")
end

actions["tsui.classic.test.jump.t19"] = function()
    rk.enter("director.t", "0498")
end

paragraphs["title"] = function()
    local selected = rk.call("roukaku.vn.menu", "show", "page.title", {kind = "title", policy = "tsui.policy.ui"})
    local action = actions[selected]
    if action then action() end
end

paragraphs["quick_panel"] = function()
    local selected = rk.call("roukaku.vn.menu", "show", "page.quick_panel", {kind = "quick_panel", policy = "tsui.policy.ui"})
    local action = actions[selected]
    if action then action() end
end

paragraphs["save"] = function()
    local selected = rk.call("roukaku.vn.menu", "show", "page.save", {kind = "save", policy = "tsui.policy.ui"})
    local action = actions[selected]
    if action then action() end
end

paragraphs["load"] = function()
    local selected = rk.call("roukaku.vn.menu", "show", "page.load", {kind = "load", policy = "tsui.policy.ui"})
    local action = actions[selected]
    if action then action() end
end

paragraphs["config"] = function()
    local selected = rk.call("roukaku.vn.menu", "show", "page.config", {kind = "config", policy = "tsui.policy.ui"})
    local action = actions[selected]
    if action then action() end
end

paragraphs["backlog"] = function()
    local selected = rk.call("roukaku.vn.menu", "show", "page.backlog", {kind = "backlog", policy = "tsui.policy.ui"})
    local action = actions[selected]
    if action then action() end
end

paragraphs["custom"] = function()
    local selected = rk.call("roukaku.vn.menu", "show", "page.custom", {kind = "custom", policy = "tsui.policy.ui"})
    local action = actions[selected]
    if action then action() end
end

return { paragraphs = paragraphs, actions = actions }
