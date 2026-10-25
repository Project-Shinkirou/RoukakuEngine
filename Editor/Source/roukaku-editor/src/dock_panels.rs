use super::*;
use gpui_component::dock::{
    BasePanel, DockAreaState, DockEvent, DockLayout, DockSkin, Panel, PanelEvent, panel_handle,
    register_panel,
};

#[derive(Clone, Copy)]
enum PaneKind {
    Content,
    Authoring,
    Details,
}
impl PaneKind {
    fn name(self) -> &'static str {
        match self {
            Self::Content => "roukaku.content",
            Self::Authoring => "roukaku.authoring",
            Self::Details => "roukaku.details",
        }
    }
    fn title(self) -> &'static str {
        match self {
            Self::Content => "Content & Outliner",
            Self::Authoring => "Authoring",
            Self::Details => "Details",
        }
    }
}
struct WorkspacePane {
    owner: WeakEntity<Editor>,
    kind: PaneKind,
    focus: FocusHandle,
    _subscription: Option<Subscription>,
}
impl WorkspacePane {
    fn new(owner: WeakEntity<Editor>, kind: PaneKind, cx: &mut Context<Self>) -> Self {
        let subscription = owner
            .upgrade()
            .map(|owner| cx.observe(&owner, |_, _, cx| cx.notify()));
        Self {
            owner,
            kind,
            focus: cx.focus_handle(),
            _subscription: subscription,
        }
    }
}
impl EventEmitter<PanelEvent> for WorkspacePane {}
impl Focusable for WorkspacePane {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
impl BasePanel for WorkspacePane {
    fn panel_name(&self) -> &'static str {
        self.kind.name()
    }

    fn closable(&self, _: &App) -> bool {
        false
    }
}
impl Panel for WorkspacePane {
    fn title(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.kind.title()
    }
}
impl Render for WorkspacePane {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content = self
            .owner
            .update(cx, |editor, cx| match self.kind {
                PaneKind::Content => editor.content_panel(window, cx),
                PaneKind::Authoring => editor.authoring_panel(cx),
                PaneKind::Details => editor.details_panel(cx),
            })
            .unwrap_or_else(|_| div().into_any_element());
        div()
            .size_full()
            .flex()
            .flex_col()
            .track_focus(&self.focus)
            .child(content)
    }
}
impl Editor {
    pub(super) fn ensure_dock(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.panels.dock.is_some() {
            return;
        }
        let owner = cx.entity().downgrade();
        for kind in [PaneKind::Content, PaneKind::Authoring, PaneKind::Details] {
            let owner = owner.clone();
            register_panel(cx, kind.name(), move |_, _, cx| {
                panel_handle(cx.new(|cx| WorkspacePane::new(owner.clone(), kind, cx)))
            });
        }
        let (dock, _) = DockSkin::dock_area("roukaku-workspace", Some(1), window, cx);
        let center = [PaneKind::Content, PaneKind::Authoring, PaneKind::Details]
            .into_iter()
            .zip([250., 560., 280.])
            .fold(DockLayout::h_split(), |layout, (kind, width)| {
                let pane = cx.new(|cx| WorkspacePane::new(owner.clone(), kind, cx));
                layout.child(
                    DockLayout::tabs().panel_view(panel_handle(pane), cx),
                    Some(px(width)),
                )
            });
        dock.update(cx, |dock, cx| dock.set_center(center, window, cx));
        let path = self.project.layout_path();
        if path.exists() {
            let result = (|| -> anyhow::Result<()> {
                anyhow::ensure!(
                    std::fs::metadata(&path)?.len() <= 65536,
                    "Layout exceeds size limit"
                );
                let state: DockAreaState = serde_json::from_slice(&std::fs::read(&path)?)?;
                anyhow::ensure!(state.version == Some(1), "Layout version changed");
                validate_layout(&serde_json::to_value(&state)?)?;
                dock.update(cx, |dock, cx| dock.load(state, window, cx))?;
                Ok(())
            })();
            if let Err(error) = result {
                self.status = format!("Layout reset: {error}");
            }
        }
        self.panels.dock_subscription =
            Some(cx.subscribe(&dock, |this, dock, event: &DockEvent, cx| {
                if !matches!(event, DockEvent::LayoutChanged) {
                    return;
                }
                let result = (|| -> anyhow::Result<()> {
                    let state = dock.read(cx).dump(cx);
                    let path = this.project.layout_path();
                    std::fs::create_dir_all(path.parent().unwrap())?;
                    let mut temporary = tempfile::NamedTempFile::new_in(path.parent().unwrap())?;
                    use std::io::Write;
                    temporary.write_all(&serde_json::to_vec(&state)?)?;
                    temporary.persist(path)?;
                    Ok(())
                })();
                if let Err(error) = result {
                    this.status = format!("Could not save layout: {error}");
                    cx.notify();
                }
            }));
        self.panels.dock = Some(dock);
    }
}

// Reject missing/duplicate panes before the component builds any views.
fn validate_layout(value: &serde_json::Value) -> anyhow::Result<()> {
    fn visit(value: &serde_json::Value, counts: &mut [usize; 3]) -> anyhow::Result<()> {
        match value {
            serde_json::Value::Object(fields) => {
                if let Some(name) = fields.get("panel_name").and_then(|v| v.as_str()) {
                    match name {
                        "roukaku.content" => counts[0] += 1,
                        "roukaku.authoring" => counts[1] += 1,
                        "roukaku.details" => counts[2] += 1,
                        "StackPanel" | "TabPanel" => (),
                        _ => anyhow::bail!("Unknown workspace pane"),
                    }
                }
                for value in fields.values() {
                    visit(value, counts)?;
                }
            }
            serde_json::Value::Array(values) => {
                for value in values {
                    visit(value, counts)?;
                }
            }
            _ => (),
        }
        Ok(())
    }
    let mut counts = [0; 3];
    visit(value, &mut counts)?;
    anyhow::ensure!(
        counts == [1; 3],
        "Layout must contain each workspace pane exactly once"
    );
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[::core::prelude::v1::test]
    fn saved_layout_cannot_drop_or_duplicate_authoring_panes() {
        let mut state = serde_json::json!({"panel_name":"StackPanel", "children":[
            {"panel_name":"roukaku.content"}, {"panel_name":"roukaku.authoring"}, {"panel_name":"roukaku.details"}]});
        validate_layout(&state).unwrap();
        state["children"][2]["panel_name"] = "roukaku.content".into();
        assert!(validate_layout(&state).is_err());
        state["children"][2]["panel_name"] = "unknown".into();
        assert!(validate_layout(&state).is_err());
    }
}
