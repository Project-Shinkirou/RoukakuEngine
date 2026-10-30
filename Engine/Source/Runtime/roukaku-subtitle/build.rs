#[path = "../roukaku-text/native_triplet.rs"]
mod native_triplet;
use std::{env, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=src/bridge.c");
    println!("cargo:rerun-if-env-changed=ROUKAKU_SUBTITLE_VCPKG_ROOT");
    let windows = env::var("CARGO_CFG_TARGET_OS").unwrap() == "windows";
    let (includes, metadata) = if windows {
        assert_eq!(
            env::var("TARGET").unwrap(),
            "x86_64-pc-windows-msvc",
            "roukaku-subtitle requires the pinned x64 MSVC native installation on Windows"
        );
        assert!(
            !env::var("CARGO_CFG_TARGET_FEATURE")
                .unwrap_or_default()
                .split(',')
                .any(|feature| feature == "crt-static"),
            "roukaku-subtitle requires the MD CRT (no +crt-static)"
        );
        let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(4)
            .unwrap()
            .to_path_buf();
        let root = env::var_os("ROUKAKU_SUBTITLE_VCPKG_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                native_triplet::install_root(
                    &workspace,
                    native_triplet::triplet(&env::var("TARGET").unwrap()).unwrap(),
                )
            });
        let tools = workspace.join(".work/vcpkg/downloads/tools/msys2");
        let pkgconf = std::fs::read_dir(tools)
            .expect("vcpkg pkg-config tools")
            .filter_map(Result::ok)
            .map(|entry| entry.path().join("usr/bin/pkg-config.exe"))
            .find(|path| path.is_file())
            .expect("vcpkg pkg-config executable");
        unsafe {
            env::set_var("PKG_CONFIG", pkgconf);
            env::set_var(
                "PKG_CONFIG_PATH",
                env::join_paths([
                    root.join("x64-windows-static-md-dynbidi/lib/pkgconfig"),
                    root.join("x64-windows-static-md-dynbidi/share/pkgconfig"),
                ])
                .unwrap(),
            );
        }
        let lib = pkg_config::Config::new()
            .exactly_version("0.17.5")
            .statik(true)
            .probe("libass")
            .expect("prepare libass 0.17.5 with cargo xtask deps --subtitle");
        (lib.include_paths, Vec::new())
    } else {
        // A real native installation is required on other platforms too. Never
        // provide a no-op renderer for a missing native dependency.
        let lib = pkg_config::Config::new()
            .exactly_version("0.17.5")
            .probe("libass")
            .expect("roukaku-subtitle requires native libass exactly 0.17.5");
        // Let pkg-config emit its complete platform metadata, including
        // absolute library files, frameworks and private linker arguments.
        (lib.include_paths, Vec::new())
    };
    let mut bridge = cc::Build::new();
    bridge
        .file("src/bridge.c")
        .includes(&includes)
        .warnings(true);
    if windows {
        bridge.static_crt(false);
    }
    // bridge.c checks LIBASS_VERSION against 0x01705000 using the actual
    // installed header; it also uses the official track layout and va_list.
    bridge.compile("roukaku_subtitle_bridge");
    for line in metadata {
        println!("{line}");
    }
    if windows {
        // vcpkg-rs omits system libraries used by libass's desktop font
        // provider, even though provider lookup is disabled at runtime.
        println!("cargo:rustc-link-lib=gdi32");
        println!("cargo:rustc-link-lib=user32");
    }
}
