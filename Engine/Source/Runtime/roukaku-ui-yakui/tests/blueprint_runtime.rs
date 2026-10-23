use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use roukaku_core::Hash256;

use roukaku_media_core::TextureFrame;

use roukaku_ui_core::{
    UiBackend, UiBlueprintBundle, UiBlueprintFrameModel, UiBlueprintModalFrameModel, UiButtonState,
    UiCapability, UiEventBinding, UiFrameRequest, UiInputDispositionKind, UiInputEvent,
    UiInputEventKind, UiInputFrame, UiInsets, UiNodeBlueprint, UiRepeatBinding, UiSemanticAction,
    UiSemanticRole, UiThemeManifest, UiThemeValue, UiValue, UiValueExpr, UiViewBlueprint,
    UiViewport,
};

use roukaku_ui_yakui::{RoukakuYakuiBackend, BlueprintYakuiRenderer};

#[path = "blueprint_runtime/bindings.rs"]
mod bindings;
use bindings::node;
#[path = "blueprint_runtime/image_cache.rs"]
mod image_cache;
#[path = "blueprint_runtime/input_actions.rs"]
mod input_actions;
