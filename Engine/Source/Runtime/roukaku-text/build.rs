use std::{env, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=src/bridge.c");
    println!("cargo:rerun-if-env-changed=ROUKAKU_TEXT_VCPKG_ROOT");
    let target = env::var("TARGET").unwrap();
    let mut bridge = cc::Build::new();
    bridge.file("src/bridge.c").warnings(true);
    let mut metadata = Vec::new();
    if let Some(root) = env::var_os("ROUKAKU_TEXT_VCPKG_ROOT") {
        let triplet = match target.as_str() {
            "x86_64-pc-windows-msvc" => "x64-windows-static-md-dynbidi",
            "x86_64-unknown-linux-gnu" => "x64-linux",
            "aarch64-linux-android" => "arm64-android-dynbidi",
            "x86_64-linux-android" => "x64-android-dynbidi",
            "aarch64-apple-darwin" => "arm64-osx",
            "x86_64-apple-darwin" => "x64-osx",
            "aarch64-apple-ios" => "arm64-ios",
            "aarch64-apple-ios-sim" => "arm64-ios-simulator",
            "x86_64-apple-ios" => "x64-ios",
            _ => {
                panic!("no Pango vcpkg triplet for {target}; supply native pangoft2 via pkg-config")
            }
        };
        let base = PathBuf::from(root).join("installed").join(triplet);
        if target.contains("windows") {
            let lib = vcpkg::Config::new()
                .vcpkg_root(base.parent().unwrap().parent().unwrap().to_path_buf())
                .target_triplet(triplet)
                .cargo_metadata(false)
                .copy_dlls(false)
                .find_package("pango")
                .expect("prepare Pango with cargo xtask deps --text");
            bridge.includes(&lib.include_paths).static_crt(false);
            for sub in ["pango-1.0", "glib-2.0", "freetype2", "harfbuzz"] {
                bridge.include(base.join("include").join(sub));
            }
            bridge.include(base.join("lib/glib-2.0/include"));
            metadata = lib.cargo_metadata;
            for system in [
                "user32", "gdi32", "ole32", "shell32", "advapi32", "ws2_32", "winmm", "imm32",
                "usp10", "dnsapi", "iphlpapi", "bcrypt",
            ] {
                metadata.push(format!("cargo:rustc-link-lib={system}"));
            }
        } else {
            let paths = [base.join("lib/pkgconfig"), base.join("share/pkgconfig")];
            // Build scripts are single-threaded; this sets only this process's pkg-config lookup.
            unsafe {
                env::set_var("PKG_CONFIG_PATH", env::join_paths(paths).unwrap());
            }
            let lib = pkg_config::Config::new()
                .statik(true)
                .probe("pangoft2")
                .expect("installed Pango FT2 backend");
            bridge.includes(lib.include_paths);
        }
    } else {
        let lib = pkg_config::Config::new()
            .atleast_version("1.50")
            .probe("pangoft2")
            .expect(
                "prepare Pango with cargo xtask deps --text or install native pangoft2 >= 1.50",
            );
        bridge.includes(lib.include_paths);
    }
    bridge.compile("roukaku_text_pango");
    for line in metadata {
        println!("{line}");
    }
}
