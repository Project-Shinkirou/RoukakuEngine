local controllers = {
    { "tsui.message.classic", "ui.tsui.classic.message", "roukaku.vn.ui_model.message.v2" },
    { "tsui.monologue.classic", "ui.tsui.classic.monologue", "roukaku.vn.ui_model.message.v2" },
    { "tsui.opening.staggered.classic", "ui.tsui.classic.opening_staggered", "roukaku.vn.ui_model.message.v2" },
    { "tsui.opening.centered.classic", "ui.tsui.classic.opening_centered", "roukaku.vn.ui_model.message.v2" },
    { "tsui.choice.classic", "ui.tsui.classic.choice", "roukaku.vn.ui_model.choice.v1" },
    { "tsui.system.title.classic", "ui.tsui.classic.title", "roukaku.vn.ui_model.title.v1" },
    { "tsui.system.popup.classic", "ui.tsui.classic.popup", "roukaku.vn.ui_model.quick_panel.v1" },
    { "tsui.system.save.classic", "ui.tsui.classic.save", "roukaku.vn.ui_model.save.v1" },
    { "tsui.system.load.classic", "ui.tsui.classic.load", "roukaku.vn.ui_model.load.v1" },
    { "tsui.system.config.classic", "ui.tsui.classic.config", "roukaku.vn.ui_model.config.v1" },
    { "tsui.system.test.classic", "ui.tsui.classic.test", "roukaku.vn.ui_model.title.v1" },
    { "tsui.message.modern", "ui.tsui.modern.message", "roukaku.vn.ui_model.message.v2" },
    { "tsui.monologue.modern", "ui.tsui.modern.monologue", "roukaku.vn.ui_model.message.v2" },
    { "tsui.choice.modern", "ui.tsui.modern.choice", "roukaku.vn.ui_model.choice.v1" },
    { "tsui.system.title.modern", "ui.tsui.modern.title", "roukaku.vn.ui_model.title.v1" },
    { "tsui.system.quick_panel.modern", "ui.tsui.modern.quick_panel", "roukaku.vn.ui_model.quick_panel.v1" },
    { "tsui.system.save.modern", "ui.tsui.modern.save", "roukaku.vn.ui_model.save.v1" },
    { "tsui.system.load.modern", "ui.tsui.modern.load", "roukaku.vn.ui_model.load.v1" },
    { "tsui.system.config.modern", "ui.tsui.modern.config", "roukaku.vn.ui_model.config.v1" },
    { "tsui.system.backlog.modern", "ui.tsui.modern.backlog", "roukaku.vn.ui_model.backlog.v1" },
}

for _, definition in controllers do
    local on_open = nil
    local on_action = function(_context, _model, action)
        return { roukaku.ui.effect.forward(action) }
    end
    if definition[1] == "tsui.system.quick_panel.modern" then
        on_open = function(_context, _model)
            return { roukaku.ui.effect.focus("root/gold/commands/skip_all") }
        end
    end
    if definition[1] == "tsui.system.title.classic" then
        on_open = function(_context, _model)
            return { roukaku.ui.effect.focus("root/start") }
        end
    end
    if definition[1] == "tsui.system.save.classic" then
        on_open = function(_context, model)
            for _, slot_value in model.map.slots.list do
                local slot = slot_value.map
                if slot.can_write.bool then
                    return {
                        roukaku.ui.effect.focus("root/window/body/slots/slot/" .. slot.slot_id.string),
                    }
                end
            end
            return { roukaku.ui.effect.focus("root/window/body/back") }
        end
    end
    if definition[1] == "tsui.system.load.classic" then
        on_open = function(_context, model)
            for _, slot_value in model.map.slots.list do
                local slot = slot_value.map
                if slot.can_load.bool then
                    return {
                        roukaku.ui.effect.focus(
                            "root/window/body/slots/slot/" .. slot.slot_id.string
                        ),
                    }
                end
            end
            return { roukaku.ui.effect.focus("root/window/body/back") }
        end
    end
    if definition[1] == "tsui.system.popup.classic" then
        on_open = function(_context, _model)
            return { roukaku.ui.effect.focus("root/window/tabs/save") }
        end
    end
    if definition[1] == "tsui.system.config.classic" then
        on_open = function(_context, _model)
            return { roukaku.ui.effect.focus("root/window/body/reading_modes/hidden") }
        end
    end
    if definition[1] == "tsui.system.test.classic" then
        on_open = function(_context, _model)
            return { roukaku.ui.effect.focus("root/window/body/flags/mode_ayana") }
        end
    end
    if definition[1] == "tsui.system.title.modern" then
        on_open = function(_context, _model)
            return { roukaku.ui.effect.focus("root/gold/menu/continue") }
        end
    end
    if definition[1] == "tsui.system.save.modern" then
        on_open = function(_context, model)
            for _, slot_value in model.map.slots.list do
                local slot = slot_value.map
                if slot.can_write.bool then
                    return {
                        roukaku.ui.effect.focus(
                            "root/gold/content/slots/slot/" .. slot.slot_id.string
                                .. "/write/" .. slot.slot_id.string
                        ),
                    }
                end
            end
            return { roukaku.ui.effect.focus("root/gold/content/back") }
        end
    end
    if definition[1] == "tsui.system.load.modern" then
        on_open = function(_context, model)
            for _, slot_value in model.map.slots.list do
                local slot = slot_value.map
                if slot.can_load.bool then
                    return {
                        roukaku.ui.effect.focus(
                            "root/gold/content/slots/slot/" .. slot.slot_id.string
                                .. "/read/" .. slot.slot_id.string
                        ),
                    }
                end
            end
            return { roukaku.ui.effect.focus("root/gold/content/back") }
        end
    end
    if definition[1] == "tsui.system.config.modern" then
        on_open = function(_context, _model)
            return { roukaku.ui.effect.focus("root/gold/content/master") }
        end
    end
    if definition[1] == "tsui.system.backlog.modern" then
        on_open = function(_context, _model)
            return { roukaku.ui.effect.focus("root/gold/content/back") }
        end
    end
    if definition[1] == "tsui.choice.classic" or definition[1] == "tsui.choice.modern" then
        local choice_root = "root/choices"
        if definition[1] == "tsui.choice.modern" then
            choice_root = "root/gold/choices"
        end
        on_open = function(_context, model)
            for _, option_value in model.map.options.list do
                local option = option_value.map
                if option.enabled.bool then
                    return {
                        roukaku.ui.effect.focus(
                            choice_root .. "/options/option/" .. option.option_id.string
                        ),
                    }
                end
            end
            return {}
        end
    end
    if definition[1] == "tsui.system.save.modern" then
        on_action = function(_context, model, action)
            if action.request_save ~= nil then
                for _, slot_value in model.map.slots.list do
                    local slot = slot_value.map
                    if slot.slot_id.string == action.request_save.slot_id and slot.occupied.bool then
                        return {
                            roukaku.ui.effect.open_modal("ui.tsui.modern.save_confirm", {
                                slot_id = slot.slot_id.string,
                            }),
                        }
                    end
                end
            end
            return { roukaku.ui.effect.forward(action) }
        end
    end
    roukaku.ui.controller.register(definition[1], {
        schema = "roukaku.vn.ui_controller.v1",
        view = definition[2],
        model_schema = definition[3],
        snapshot = "session",
    }, { on_open = on_open, on_action = on_action })
end
