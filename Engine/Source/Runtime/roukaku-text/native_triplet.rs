//! Shared target-to-vcpkg mapping for xtask provisioning and the text build script.
pub fn triplet(target: &str) -> Option<&'static str> {
    match target {
        "x86_64-pc-windows-msvc" => Some("x64-windows-static-md-dynbidi"),
        "x86_64-unknown-linux-gnu" => Some("x64-linux"),
        "aarch64-pc-windows-msvc" => Some("arm64-windows-static-md"),
        "aarch64-linux-android" => Some("arm64-android-dynbidi"),
        "x86_64-linux-android" => Some("x64-android-dynbidi"),
        "aarch64-apple-darwin" => Some("arm64-osx"),
        "x86_64-apple-darwin" => Some("x64-osx"),
        "aarch64-apple-ios" => Some("arm64-ios"),
        "aarch64-apple-ios-sim" => Some("arm64-ios-simulator"),
        "x86_64-apple-ios" => Some("x64-ios"),
        _ => None,
    }
}

pub fn install_root(root: &std::path::Path, triplet: &str) -> std::path::PathBuf {
    root.join(".work")
        .join("vcpkg-subtitle")
        .join(format!("installed-{triplet}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn targets_get_disjoint_triplet_install_roots() {
        let windows = triplet("x86_64-pc-windows-msvc").unwrap();
        let android = triplet("aarch64-linux-android").unwrap();
        let windows_root = install_root(std::path::Path::new("repo"), windows);
        let android_root = install_root(std::path::Path::new("repo"), android);
        assert_ne!(windows_root, android_root);
        assert_eq!(
            windows_root.join(windows),
            std::path::Path::new(
                "repo/.work/vcpkg-subtitle/installed-x64-windows-static-md-dynbidi/x64-windows-static-md-dynbidi"
            )
        );
        assert_eq!(
            android_root.join(android),
            std::path::Path::new(
                "repo/.work/vcpkg-subtitle/installed-arm64-android-dynbidi/arm64-android-dynbidi"
            )
        );
    }
}
