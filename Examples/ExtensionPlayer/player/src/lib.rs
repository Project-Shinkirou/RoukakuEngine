pub fn extensions() -> Result<roukaku_plugin::WorkExtensionRegistry, String> {
    let mut registry = roukaku_plugin::WorkExtensionRegistry::default();
    registry.register::<roukaku_example_progress_meter::ProgressMeter>()?;
    Ok(registry)
}
