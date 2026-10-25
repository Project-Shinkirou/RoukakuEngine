fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    roukaku_example_extension_player::extensions()?.install()?;
    roukaku_player::run()
}
