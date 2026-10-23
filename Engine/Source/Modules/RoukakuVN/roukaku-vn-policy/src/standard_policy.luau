local pages = {
  "title",
  "save",
  "load",
  "config",
  "gallery",
  "replay",
  "voice_replay",
  "route_chart",
  "backlog",
  "localization_preview",
}

roukaku.command.register("roukaku.vn.message.show", {
  schema = "roukaku.vn.command.message.show.v1",
  role = "message_ui",
  budget_ms = 1,
}, function(command)
  return command ~= nil
end)

roukaku.command.register("roukaku.vn.choice.show", {
  schema = "roukaku.vn.command.choice.show.v1",
  role = "choice_ui",
  budget_ms = 1,
}, function(command)
  return command ~= nil
end)

roukaku.command.register("roukaku.vn.system.open", {
  schema = "roukaku.vn.command.system.open.v1",
  role = "system_story",
  budget_ms = 2,
}, function(command)
  return command ~= nil
end)

roukaku.command.register("roukaku.vn.timeline.run", {
  schema = "roukaku.vn.command.timeline.run.v1",
  role = "presentation_timeline",
  budget_ms = 2,
}, function(command)
  return command ~= nil
end)

roukaku.snapshot.set("roukaku.policy.standard", {
  version = 1,
  system_page_count = #pages,
  message_ui = true,
  choice_ui = true,
})

roukaku.trace.event("policy.loaded", {
  id = "roukaku.policy.standard",
  system_page_count = #pages,
})

return roukaku.snapshot.get("roukaku.policy.standard").system_page_count == #pages
