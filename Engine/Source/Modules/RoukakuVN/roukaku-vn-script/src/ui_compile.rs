use std::collections::{BTreeMap, BTreeSet};

use roukaku_core::Hash256;
use roukaku_ui_core::{
    UiBindingManifest, UiBindingRoot, UiBlueprintBundle, UiCapability, UiEventBinding,
    UiNodeBlueprint, UiProfileScopedBindings, UiRepeatBinding, UiThemeManifest, UiValue,
    UiValueExpr, UiViewBinding, UiViewBlueprint, ValidateUi,
};
use serde_json::Value;

use crate::{
    CompiledStory, CompiledVnProject, ReadingMode, RoukakuSource, SaveCompletionPolicy,
    SystemPageKind, SystemUiProfilePolicy, VnError,
};

mod jsonc;
use jsonc::{Document, pointer};
pub use jsonc::{UI_SCREEN_SCHEMA, UI_STYLE_SCHEMA};
mod intake;
use intake::*;
mod style;
pub use style::{CompiledUiStyle, compile_ui_style};
mod compile_project_ui;
pub(crate) use compile_project_ui::compile_project_ui;
mod validate_node_bindings;
use validate_node_bindings::validate_node_bindings;
mod validate_widget_properties;
use validate_widget_properties::{
    hash_bindings, hash_blueprints, hash_project, infer_capabilities, ui_error, validate_action,
    validate_widget_event,
};

struct Context<'a> {
    document: &'a Document<'a>,
    pointer: String,
}

impl Context<'_> {
    fn error(&self, code: &str, message: &str) -> VnError {
        self.document.error(&self.pointer, code, message)
    }
}

fn string<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key].as_str().expect("schema-validated string")
}

fn array<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    value
        .get(key)
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}
