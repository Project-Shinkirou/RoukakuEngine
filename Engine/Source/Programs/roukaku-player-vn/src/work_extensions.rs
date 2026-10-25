use roukaku_plugin::{ExtensionInvocation, WorkExtensionRuntime};
use roukaku_vn_core::{ExtensionPresentationCommand, ExtensionValue};
use serde_json::Value;

/// Dispatch the same typed command used by packaged story presentation outputs.
pub fn dispatch_work_extension(
    runtime: &mut WorkExtensionRuntime,
    command: &ExtensionPresentationCommand,
) -> Result<Vec<roukaku_media_core::SceneCommand>, String> {
    let arguments = Value::Object(
        command
            .fields
            .iter()
            .map(|(name, value)| {
                let value = match value {
                    ExtensionValue::String(value)
                    | ExtensionValue::Symbol(value)
                    | ExtensionValue::AssetUri(value) => Value::String(value.clone()),
                    ExtensionValue::Integer(value) => Value::from(*value),
                    ExtensionValue::Boolean(value) => Value::from(*value),
                    ExtensionValue::Fixed(value) => Value::from(value.millionths),
                };
                (name.clone(), value)
            })
            .collect(),
    );
    runtime.dispatch(
        &command.provider_id,
        ExtensionInvocation {
            command: &command.command,
            schema: &command.schema,
            arguments: &arguments,
        },
    )
}
