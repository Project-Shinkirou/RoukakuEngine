use std::{cell::RefCell, collections::BTreeMap, rc::Rc, sync::Arc};

use roukaku_policy::{PolicyExecutionBudget, create_sandboxed_lua};

use roukaku_ui_core::{MAX_EFFECTS_PER_CALL, UiValue};

use roukaku_vn_ui::{
    VnUiAction, VnUiControllerEffect, VnUiControllerManifest, VnUiControllerSnapshot,
    VnUiControllerUpdate, VnUiSessionState,
};

#[cfg(feature = "portable-luau-runtime")]
use luaur_rt as mlua;

#[cfg(feature = "portable-luau-runtime")]
use luaur_rt::{Lua, LuaSerdeExt, Table, Value};

#[cfg(feature = "luau-runtime")]
use mlua::{Lua, LuaSerdeExt, Table, Value, chunk::Compiler};

use thiserror::Error;

#[cfg(test)]
mod tests;

mod contract;
use contract::ControllerInvocation;
pub use contract::LuauUiControllerError;
pub use contract::LuauUiControllerHost;
mod lua_value_to_ui_value;
#[cfg(feature = "luau-runtime")]
use lua_value_to_ui_value::create_table;
#[cfg(feature = "portable-luau-runtime")]
use lua_value_to_ui_value::create_table;
use lua_value_to_ui_value::lua_value_to_ui_value;
use lua_value_to_ui_value::runtime_error;
use lua_value_to_ui_value::validate_effects;
use lua_value_to_ui_value::validate_handler;
