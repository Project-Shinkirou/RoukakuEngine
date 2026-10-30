use std::cell::RefCell;

use roukaku_text::{
    MediaError, TextLayoutProvider, TextLayoutProviderIdentity, TextLayoutRequest,
    TextLayoutResult,
};
use roukaku_vn_package::authoring::AuthoringHost;

use super::*;

/// Story-ordered mechanism output observed while the Luau coroutine runs.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum StoryEvent {
    ParagraphStarted(ParagraphId),
    Present { node: String, command: StageCommand },
    Record(NodeRecord),
    Property(NativeVnPropertySample),
    ActionStart {
        node: String,
        command: String,
        params: serde_json::Value,
    },
}

/// Session-owned `AuthoringHost`. It buffers events for the session to apply
/// after each resume and answers the host queries of the default VN modules.
#[derive(Default)]
pub(super) struct StoryHost {
    events: RefCell<Vec<StoryEvent>>,
    system_actions: RefCell<BTreeSet<String>>,
    text_layout: RefCell<Option<Arc<dyn TextLayoutProvider>>>,
}

impl StoryHost {
    pub(super) fn take_events(&self) -> Vec<StoryEvent> {
        std::mem::take(&mut self.events.borrow_mut())
    }

    pub(super) fn set_system_actions(&self, actions: BTreeSet<String>) {
        *self.system_actions.borrow_mut() = actions;
    }

    pub(super) fn set_text_layout(&self, provider: Arc<dyn TextLayoutProvider>) {
        *self.text_layout.borrow_mut() = Some(provider);
    }

    fn push(&self, event: StoryEvent) {
        self.events.borrow_mut().push(event);
    }

    fn text_layout(&self) -> Result<Arc<dyn TextLayoutProvider>, MediaError> {
        self.text_layout.borrow().clone().ok_or_else(|| {
            MediaError::message(
                "ROUKAKU_VN_TEXT_LAYOUT_UNBOUND: session has no shared text layout provider",
            )
        })
    }

    /// Menu options are the authored options, the profile's declared system
    /// actions, and `return`, which closes a story-owned system page.
    fn prepare_menu(&self, args: serde_json::Value) -> mlua::Result<serde_json::Value> {
        let mut options = Vec::new();
        let mut ids = BTreeSet::new();
        if let Some(authored) = args.get("options") {
            let authored: Vec<roukaku_vn_package::authoring::ChoiceOption> =
                serde_json::from_value(authored.clone()).map_err(mlua::Error::external)?;
            for option in authored {
                if ids.insert(option.id.clone()) {
                    options.push(serde_json::json!({ "id": option.id, "text": option.text }));
                }
            }
        }
        for action in self
            .system_actions
            .borrow()
            .iter()
            .map(String::as_str)
            .chain(["return"])
        {
            if ids.insert(action.to_owned()) {
                options.push(serde_json::json!({ "id": action, "text": action }));
            }
        }
        Ok(serde_json::json!({ "options": options }))
    }
}

impl TextLayoutProvider for StoryHost {
    fn identity(&self) -> Result<TextLayoutProviderIdentity, MediaError> {
        self.text_layout()?.identity()
    }

    fn layout(&self, request: &TextLayoutRequest) -> Result<TextLayoutResult, MediaError> {
        self.text_layout()?.layout(request)
    }
}

impl AuthoringHost for StoryHost {
    fn display_text(&self, id: &str, _layout: Arc<TextLayoutResult>) -> mlua::Result<()> {
        Err(mlua::Error::runtime(format!(
            "ROUKAKU_VN_DISPLAY_TEXT_UNSUPPORTED: text {id} needs a project presentation adapter; use the dialog module"
        )))
    }

    fn set_property(&self, target: &str, property: &str, value: f64) -> mlua::Result<()> {
        self.push(StoryEvent::Property(NativeVnPropertySample {
            target: target.to_owned(),
            property: property.to_owned(),
            value,
        }));
        Ok(())
    }

    fn release_segment(&self, _id: u64) -> mlua::Result<()> {
        Ok(())
    }

    fn paragraph_started(&self, paragraph: &ParagraphId) -> mlua::Result<()> {
        self.push(StoryEvent::ParagraphStarted(paragraph.clone()));
        Ok(())
    }

    fn node_recorded(&self, record: &NodeRecord) -> mlua::Result<()> {
        self.push(StoryEvent::Record(record.clone()));
        Ok(())
    }

    fn present(&self, node: &str, command: &StageCommand) -> mlua::Result<()> {
        self.push(StoryEvent::Present {
            node: node.to_owned(),
            command: command.clone(),
        });
        Ok(())
    }

    fn query(&self, operation: &str, args: serde_json::Value) -> mlua::Result<serde_json::Value> {
        match operation {
            "vn.menu.prepare" => self.prepare_menu(args),
            "vn.action.start" => {
                let field = |name: &str| {
                    args.get(name)
                        .and_then(serde_json::Value::as_str)
                        .filter(|value| !value.is_empty())
                        .map(str::to_owned)
                        .ok_or_else(|| {
                            mlua::Error::runtime(format!(
                                "ROUKAKU_VN_POLICY_ACTION: action {name} must be a nonempty string"
                            ))
                        })
                };
                self.push(StoryEvent::ActionStart {
                    node: field("node")?,
                    command: field("command")?,
                    params: args.get("params").cloned().unwrap_or_default(),
                });
                Ok(serde_json::Value::Bool(true))
            }
            _ => Err(mlua::Error::runtime(format!(
                "ROUKAKU_VN_HOST_QUERY_UNSUPPORTED: {operation}"
            ))),
        }
    }
}
