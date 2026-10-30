//! Preload only explicitly authorized package text. Runtime reads never access disk.
use crate::NativeVnHostError;
use roukaku_asset::VfsUri;
use roukaku_core::Hash256;
use roukaku_package::{MountedAssetResolver, PackageReader};
use roukaku_runtime::TaskScope;
use roukaku_vn_policy::{
    POLICY_IO_MAX_BYTES, PolicyActionBinding, PolicyIoCapabilities, PolicyIoFailure, PolicyIoHost,
    PolicyReadText,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
    pin::Pin,
    sync::Arc,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedPolicyActions {
    pub(crate) package_hash: Hash256,
    pub(crate) bindings: Vec<PolicyActionBinding>,
    texts: BTreeMap<VfsUri, Arc<str>>,
    pub(crate) byte_len: u64,
}

impl PreparedPolicyActions {
    /// Synchronous bounded loading; cancellation is observed before and after each section.
    /// Callers running a platform event loop perform this on their owned startup worker.
    pub fn load(
        package: &PackageReader,
        max_bytes: u64,
        scope: &TaskScope,
    ) -> Result<Self, NativeVnHostError> {
        let resolver = MountedAssetResolver::new(package).map_err(|_| error("MANIFEST"))?;
        Self::load_with_resolver(package, max_bytes, scope, &resolver)
    }

    pub fn load_with_resolver(
        package: &PackageReader,
        max_bytes: u64,
        scope: &TaskScope,
        resolver: &MountedAssetResolver,
    ) -> Result<Self, NativeVnHostError> {
        check_cancelled(scope)?;
        let bindings = match resolver.resolve_section("vn.policy_actions", 4 * 1024 * 1024) {
            Ok(section) => {
                let bindings: Vec<PolicyActionBinding> =
                    postcard::from_bytes(&section.bytes).map_err(|_| error("BINDINGS"))?;
                for binding in &bindings {
                    binding.validate().map_err(|_| error("BINDINGS"))?;
                }
                bindings
            }
            Err(_) if !package.has_section("vn.policy_actions") => Vec::new(),
            Err(_) => return Err(error("BINDINGS")),
        };
        let mut result = Self {
            package_hash: package.package_hash(),
            bindings,
            texts: BTreeMap::new(),
            byte_len: 0,
        };
        if result.bindings.is_empty() {
            return Ok(result);
        }
        #[cfg(target_arch = "wasm32")]
        return Err(error("UNSUPPORTED"));
        #[cfg(not(target_arch = "wasm32"))]
        {
            let mut checked = BTreeSet::new();
            for binding in &result.bindings {
                if checked.insert((&binding.entry, &binding.source_hash)) {
                    let descriptors = roukaku_vn_policy::LuauPolicy::action_descriptors(
                        &binding.source,
                        Default::default(),
                    )
                    .map_err(|_| error("REGISTRATION"))?;
                    if descriptors.len()
                        != result
                            .bindings
                            .iter()
                            .filter(|other| other.entry == binding.entry)
                            .count()
                    {
                        return Err(error("REGISTRATION"));
                    }
                    for descriptor in descriptors {
                        if !result.bindings.iter().any(|other| {
                            other.entry == binding.entry
                                && other.source_hash == binding.source_hash
                                && other.descriptor == descriptor
                        }) {
                            return Err(error("REGISTRATION"));
                        }
                    }
                }
            }
            let resources: BTreeSet<_> = result
                .bindings
                .iter()
                .flat_map(|b| b.read_text.iter().cloned())
                .collect();
            PolicyIoCapabilities::read_text(resources.clone(), POLICY_IO_MAX_BYTES)
                .map_err(|_| error("GRANTS"))?;
            let total = resources.len();
            for (index, resource) in resources.into_iter().enumerate() {
                check_cancelled(scope)?;
                let asset = resolver
                    .catalog_entries()
                    .iter()
                    .find(|asset| asset.asset_id == resource.as_str())
                    .ok_or_else(|| error("ASSET_MAPPING"))?;
                if resource.prefix() != "asset"
                    || asset.media_kind != "text"
                    || (!asset.profiles.is_empty()
                        && !asset
                            .profiles
                            .iter()
                            .any(|profile| profile == package.product_context().profile()))
                {
                    return Err(error("ASSET_TYPE_OR_PROFILE"));
                }
                let resolved = resolver
                    .resolve_asset(resource.as_str(), POLICY_IO_MAX_BYTES)
                    .map_err(|_| error("RESOURCE"))?;
                if resolved.media_kind != "text"
                    || result
                        .byte_len
                        .checked_add(resolved.bytes.len() as u64)
                        .is_none_or(|size| size >= max_bytes)
                {
                    return Err(error("TEXT_BUDGET_OR_IDENTITY"));
                }
                check_cancelled(scope)?;
                let text = String::from_utf8(resolved.bytes.to_vec()).map_err(|_| error("UTF8"))?;
                result.byte_len += text.len() as u64;
                result.texts.insert(resource, Arc::from(text));
                tracing::info!(
                    event = "player.policy.preload.progress",
                    completed = index + 1,
                    total,
                    byte_count = result.byte_len,
                    "Loaded authorized policy text"
                );
            }
            check_cancelled(scope)?;
            Ok(result)
        }
    }

    pub(crate) fn capabilities(&self) -> Result<PolicyIoCapabilities, NativeVnHostError> {
        PolicyIoCapabilities::read_text(self.texts.keys().cloned(), POLICY_IO_MAX_BYTES)
            .map_err(|_| error("GRANTS"))
    }
}
impl PolicyIoHost for PreparedPolicyActions {
    fn read_text(
        &self,
        request: PolicyReadText,
    ) -> Pin<Box<dyn Future<Output = Result<String, PolicyIoFailure>> + Send>> {
        let text = self.texts.get(&request.resource).cloned();
        Box::pin(async move {
            let text = text.ok_or(PolicyIoFailure::Host)?;
            if text.len() > request.max_bytes {
                return Err(PolicyIoFailure::Limit);
            }
            let mut bytes = Vec::with_capacity(text.len());
            for chunk in text.as_bytes().chunks(16 * 1024) {
                tokio::task::yield_now().await;
                bytes.extend_from_slice(chunk);
            }
            String::from_utf8(bytes).map_err(|_| PolicyIoFailure::Host)
        })
    }
}
fn check_cancelled(scope: &TaskScope) -> Result<(), NativeVnHostError> {
    if scope.is_cancelled() {
        Err(error("CANCELLED"))
    } else {
        Ok(())
    }
}
fn error(kind: &str) -> NativeVnHostError {
    NativeVnHostError::Package(format!("ROUKAKU_PLAYER_POLICY_{kind}"))
}
