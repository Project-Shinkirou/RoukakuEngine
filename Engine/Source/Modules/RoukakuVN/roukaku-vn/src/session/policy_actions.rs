use super::*;
use roukaku_vn_package::{ExtensionFieldKind, ExtensionValue, FixedScalar};
use roukaku_vn_policy::{
    LuauPolicy, PolicyActionBinding, PolicyActionContinuation, PolicyActionOutcome,
    PolicyExecutionBudget, PolicyIoContext, PolicyIoStatus, PolicyQueryContext,
};

/// Variable writes committed by a completed action, delivered to the story.
type VariableWrites = Vec<(String, String, i64)>;

impl VnSession {
    /// Bind validated, immutable package entries before launching the story.
    pub fn bind_policy_actions(
        &mut self,
        bindings: Vec<PolicyActionBinding>,
    ) -> Result<(), VnError> {
        if self.state.cursor.is_some() || !self.policy_host.actions.is_empty() {
            return Err(action_error("actions must be bound once before launch"));
        }
        let mut actions = BTreeMap::new();
        for binding in bindings {
            binding.validate()?;
            if actions
                .insert(binding.descriptor.command.clone(), binding)
                .is_some()
            {
                return Err(action_error("duplicate action binding"));
            }
        }
        self.policy_host.actions = actions;
        Ok(())
    }

    /// True only while gameplay owns the action fence. System-page navigation pauses it.
    pub fn policy_action_pending(&self) -> Result<bool, VnError> {
        let Some(wait) = self.state.pending_wait.as_ref() else {
            return Ok(false);
        };
        if self.policy_host.actions.is_empty()
            || !self.state.system_stack.is_empty()
            || wait.kind != VnWaitKind::Fence
            || wait.fence.strip_prefix("policy.") != Some(wait.command_id.as_str())
        {
            return Ok(false);
        }
        let policy = self.policy_state()?;
        Ok(policy
            .action
            .as_ref()
            .is_some_and(|action| action.source_id == wait.command_id))
    }

    /// Convert story parameters with the package descriptor, then start the action.
    pub(super) fn queue_policy_action(
        &mut self,
        node: &str,
        command: &str,
        params: &serde_json::Value,
    ) -> Result<(), VnError> {
        let binding = self
            .policy_host
            .actions
            .get(command)
            .ok_or_else(|| action_error("action has no package binding"))?;
        let empty = serde_json::Map::new();
        let params = match params {
            serde_json::Value::Object(params) => params,
            serde_json::Value::Null => &empty,
            // Luau encodes an empty table as an empty array.
            serde_json::Value::Array(values) if values.is_empty() => &empty,
            _ => return Err(action_error("action parameters must be a table")),
        };
        let descriptor = &binding.descriptor;
        let mut fields = BTreeMap::new();
        for (name, value) in params {
            let field = descriptor
                .fields
                .get(name)
                .ok_or_else(|| action_error("action parameter is not declared"))?;
            fields.insert(name.clone(), extension_value(field.kind, value)?);
        }
        let command = ExtensionPresentationCommand {
            command: descriptor.command.clone(),
            provider_id: descriptor.provider_id.clone(),
            schema: descriptor.schema.clone(),
            fields,
        };
        binding.validate_command(&command)?;
        let mut policy = self.policy_state()?;
        if policy.action.is_some() {
            return Err(action_error("another action is still active"));
        }
        policy.action_serial = policy
            .action_serial
            .checked_add(1)
            .ok_or_else(|| action_error("action identity exhausted"))?;
        policy.action = Some(PolicyActionContinuation {
            source_id: node.to_owned(),
            instance: policy.action_serial,
            command,
            waiting_key: None,
            state: BTreeMap::new(),
        });
        self.write_policy(&policy)
    }

    /// Runs the bound action once; on completion returns its source node and writes.
    pub(super) fn poll_policy_action(
        &mut self,
    ) -> Result<Option<(String, VariableWrites)>, VnError> {
        if !self.policy_action_pending()? {
            return Err(action_error(
                "policy tick requires its current action fence",
            ));
        }
        let mut policy = self.policy_state()?;
        let mut action = policy
            .action
            .take()
            .ok_or_else(|| action_error("missing action"))?;
        if let Some(key) = &action.waiting_key {
            let key = action.request_key(key)?;
            let record = policy
                .io
                .requests
                .get(&key)
                .ok_or_else(|| action_error("waiting action request is missing"))?;
            if record.status == PolicyIoStatus::Pending {
                return Ok(None);
            }
        }
        let binding = self
            .policy_host
            .actions
            .get(&action.command.command)
            .ok_or_else(|| action_error("missing action binding"))?;
        policy.state.variables = self.state.variables.clone();
        let outcome = LuauPolicy::new()?.invoke_action(
            binding,
            &mut action,
            &mut policy.state,
            &PolicyQueryContext::default(),
            PolicyExecutionBudget::default(),
            PolicyIoContext {
                state: &mut policy.io,
                capabilities: &self.policy_host.capabilities,
            },
        )?;
        let result = match outcome {
            PolicyActionOutcome::Await { .. } => {
                policy.action = Some(action);
                None
            }
            PolicyActionOutcome::Complete => {
                let prefix = format!("a{}.", action.instance);
                policy
                    .io
                    .requests
                    .retain(|key, _| !key.starts_with(&prefix));
                let writes = policy
                    .state
                    .mutation_trace
                    .iter()
                    .map(|m| (m.scope.clone(), m.key.clone(), m.value))
                    .collect();
                Some((action.source_id, writes))
            }
        };
        self.sync_policy_awaits(&mut policy)?;
        self.write_policy(&policy)?;
        self.dispatch_policy_io(&policy)?;
        Ok(result)
    }

    /// Commit action writes to session variables and resume the story with them.
    pub(super) fn complete_policy_action(
        &mut self,
        node: &str,
        writes: VariableWrites,
    ) -> Result<(), VnError> {
        let mut payload = Vec::with_capacity(writes.len());
        for (scope, key, value) in writes {
            payload.push(serde_json::json!({ "scope": scope, "key": key, "value": value }));
            self.state
                .variables
                .entry(scope)
                .or_default()
                .insert(key, value);
        }
        self.resume_action(node, serde_json::Value::Array(payload))
    }
}

fn extension_value(
    kind: ExtensionFieldKind,
    value: &serde_json::Value,
) -> Result<ExtensionValue, VnError> {
    let invalid = || action_error("action parameter does not match its declared kind");
    Ok(match kind {
        ExtensionFieldKind::String => {
            ExtensionValue::String(value.as_str().ok_or_else(invalid)?.into())
        }
        ExtensionFieldKind::Symbol => {
            ExtensionValue::Symbol(value.as_str().ok_or_else(invalid)?.into())
        }
        ExtensionFieldKind::AssetUri => {
            ExtensionValue::AssetUri(value.as_str().ok_or_else(invalid)?.into())
        }
        ExtensionFieldKind::Boolean => {
            ExtensionValue::Boolean(value.as_bool().ok_or_else(invalid)?)
        }
        ExtensionFieldKind::Integer => {
            let number = value.as_f64().ok_or_else(invalid)?;
            if number.fract() != 0.0 || number.abs() > (1_u64 << 53) as f64 {
                return Err(invalid());
            }
            ExtensionValue::Integer(number as i64)
        }
        ExtensionFieldKind::Fixed => {
            let millionths = (value.as_f64().ok_or_else(invalid)? * 1_000_000.0).round();
            if !millionths.is_finite() || millionths.abs() > i64::MAX as f64 {
                return Err(invalid());
            }
            ExtensionValue::Fixed(FixedScalar {
                millionths: millionths as i64,
            })
        }
    })
}

fn action_error(message: &str) -> VnError {
    VnError::diagnostic("ROUKAKU_VN_POLICY_ACTION", message)
}

/// Validate a saved continuation against the immutable package bindings.
pub(super) fn validate_saved_action(
    policy: &super::policy_io::SessionPolicyState,
    bindings: &BTreeMap<String, PolicyActionBinding>,
) -> Result<(), RuntimeError> {
    let fail = || {
        RuntimeError::message(
            "ROUKAKU_VN_POLICY_ACTION_RESTORE: continuation does not match the package",
        )
    };
    let Some(action) = &policy.action else {
        return Ok(());
    };
    action.validate().map_err(|_| fail())?;
    if action.instance == 0 || action.instance != policy.action_serial {
        return Err(fail());
    }
    let binding = bindings.get(&action.command.command).ok_or_else(fail)?;
    binding
        .validate_command(&action.command)
        .map_err(|_| fail())?;
    let prefix = format!("a{}.", action.instance);
    for (key, record) in &policy.io.requests {
        if let Some(local) = key.strip_prefix(&prefix) {
            action.request_key(local).map_err(|_| fail())?;
            if !binding.read_text.contains(&record.request.resource) {
                return Err(fail());
            }
        }
    }
    if let Some(key) = &action.waiting_key {
        let key = action.request_key(key).map_err(|_| fail())?;
        if !policy.io.requests.contains_key(&key) {
            return Err(fail());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
