use std::{
    collections::BTreeMap,
    sync::{Arc, OnceLock},
};

use roukaku_media_core::SceneCommand;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Data-only declarations shared by work extensions and compiled Editor panels.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkExtensionDeclaration {
    pub provider_id: String,
    pub commands: BTreeMap<String, ExtensionCommandDeclaration>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExtensionCommandDeclaration {
    pub schema: String,
    pub arguments_schema: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExtensionStateSnapshot {
    pub schema: String,
    pub value: Value,
}

/// Business state only. Implementations rebuild native objects when restoring.
pub trait ExtensionState {
    fn capture(&self) -> Result<ExtensionStateSnapshot, String>;
    fn validate_restore(&self, snapshot: &ExtensionStateSnapshot) -> Result<(), String>;
    fn restore(&mut self, snapshot: &ExtensionStateSnapshot) -> Result<(), String>;
}

pub enum ExtensionRestoration<'a> {
    Transient,
    Restorable(&'a mut dyn ExtensionState),
}

pub struct ExtensionInvocation<'a> {
    pub command: &'a str,
    pub schema: &'a str,
    pub arguments: &'a Value,
}

/// Each Player session owns its extension instances; no callbacks cross a DLL ABI.
pub trait WorkExtension: Send {
    fn declaration() -> WorkExtensionDeclaration
    where
        Self: Sized;
    fn execute(&mut self, invocation: ExtensionInvocation<'_>)
    -> Result<Vec<SceneCommand>, String>;
    fn restoration(&mut self) -> ExtensionRestoration<'_>;
}

struct Registration {
    declaration: Arc<WorkExtensionDeclaration>,
    create: fn() -> Box<dyn WorkExtension>,
}

#[derive(Default)]
pub struct WorkExtensionRegistry {
    registrations: BTreeMap<String, Registration>,
}

impl WorkExtensionRegistry {
    pub fn register<E: WorkExtension + Default + 'static>(&mut self) -> Result<(), String> {
        let declaration = E::declaration();
        if declaration.provider_id.is_empty() || declaration.commands.is_empty() {
            return Err("ROUKAKU_EXTENSION_DECLARATION_EMPTY".into());
        }
        if self.registrations.contains_key(&declaration.provider_id) {
            return Err(format!(
                "ROUKAKU_EXTENSION_PROVIDER_DUPLICATE: {}",
                declaration.provider_id
            ));
        }
        self.registrations.insert(
            declaration.provider_id.clone(),
            Registration {
                declaration: Arc::new(declaration),
                create: || Box::<E>::default(),
            },
        );
        Ok(())
    }

    pub fn instantiate(&self) -> WorkExtensionRuntime {
        WorkExtensionRuntime {
            providers: self
                .registrations
                .iter()
                .map(|(id, registration)| {
                    (
                        id.clone(),
                        (
                            Arc::clone(&registration.declaration),
                            (registration.create)(),
                        ),
                    )
                })
                .collect(),
        }
    }

    /// Install once, before starting the Player or spawning session workers.
    pub fn install(self) -> Result<(), String> {
        WORK_EXTENSIONS
            .set(self)
            .map_err(|_| "ROUKAKU_EXTENSION_REGISTRY_ALREADY_INSTALLED".into())
    }
}

static WORK_EXTENSIONS: OnceLock<WorkExtensionRegistry> = OnceLock::new();

pub fn linked_work_extensions() -> &'static WorkExtensionRegistry {
    WORK_EXTENSIONS.get_or_init(WorkExtensionRegistry::default)
}

#[derive(Default)]
pub struct WorkExtensionRuntime {
    providers: BTreeMap<String, (Arc<WorkExtensionDeclaration>, Box<dyn WorkExtension>)>,
}

impl WorkExtensionRuntime {
    pub fn validate_command(
        &self,
        provider: &str,
        command: &str,
        schema: &str,
    ) -> Result<(), String> {
        let (declaration, _) = self.providers.get(provider).ok_or_else(|| format!(
            "ROUKAKU_EXTENSION_PROVIDER_UNREGISTERED: {provider}; build this project's Player with its extension crates"
        ))?;
        let contract = declaration.commands.get(command).ok_or_else(|| {
            format!("ROUKAKU_EXTENSION_COMMAND_UNREGISTERED: {provider}/{command}")
        })?;
        if contract.schema != schema {
            return Err(format!(
                "ROUKAKU_EXTENSION_COMMAND_SCHEMA: {provider}/{command} expected {}, received {schema}",
                contract.schema
            ));
        }
        Ok(())
    }

    pub fn dispatch(
        &mut self,
        provider: &str,
        invocation: ExtensionInvocation<'_>,
    ) -> Result<Vec<SceneCommand>, String> {
        self.validate_command(provider, invocation.command, invocation.schema)?;
        self.providers
            .get_mut(provider)
            .expect("validated provider")
            .1
            .execute(invocation)
    }

    /// Explicit integration seam for paragraph checkpoints and restore owners.
    pub fn restoration(&mut self, provider: &str) -> Result<ExtensionRestoration<'_>, String> {
        self.providers
            .get_mut(provider)
            .map(|(_, extension)| extension.restoration())
            .ok_or_else(|| format!("ROUKAKU_EXTENSION_PROVIDER_UNREGISTERED: {provider}"))
    }
}
