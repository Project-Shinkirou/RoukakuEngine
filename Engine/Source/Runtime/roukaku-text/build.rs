#[path = "native_triplet.rs"]
mod native_triplet;

use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=src/bridge.c");
    println!("cargo:rerun-if-changed=native_triplet.rs");
    println!("cargo:rerun-if-env-changed=ROUKAKU_TEXT_VCPKG_ROOT");
    let target = env::var("TARGET").expect("Cargo TARGET is set");
    let mut bridge = cc::Build::new();
    bridge.file("src/bridge.c").warnings(true);
    let mut metadata = Vec::new();
    if let Some(triplet) = native_triplet::triplet(&target) {
        let root = env::var_os("ROUKAKU_TEXT_VCPKG_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|| native_triplet::install_root(&manifest_root(), triplet));
        let base = root.join(triplet);
        let pkgconfig = [base.join("lib/pkgconfig"), base.join("share/pkgconfig")];
        if target.contains("windows") {
            let pkgconf_root = manifest_root().join(".work/vcpkg/downloads/tools/msys2");
            let pkgconf = fs::read_dir(pkgconf_root)
                .expect("vcpkg MSYS package tools are available")
                .filter_map(Result::ok)
                .map(|entry| entry.path().join("usr/bin/pkg-config.exe"))
                .find(|path| path.is_file())
                .expect("vcpkg pkg-config executable is available");
            unsafe {
                env::set_var("PKG_CONFIG", pkgconf);
                env::set_var("PKG_CONFIG_PATH", env::join_paths(&pkgconfig).unwrap());
            }
            let lib = pkg_config::Config::new()
                .statik(true)
                .probe("pangoft2")
                .expect("prepare Pango with cargo xtask deps --text");
            bridge.includes(&lib.include_paths).static_crt(false);
            metadata.extend(
                lib.link_paths
                    .iter()
                    .map(|path| format!("cargo:rustc-link-search=native={}", path.display())),
            );
            metadata.extend(
                lib.libs
                    .iter()
                    .map(|library| format!("cargo:rustc-link-lib={library}")),
            );
            for system in [
                "user32", "gdi32", "ole32", "shell32", "advapi32", "ws2_32", "winmm", "imm32",
                "usp10", "dnsapi", "iphlpapi", "bcrypt",
            ] {
                metadata.push(format!("cargo:rustc-link-lib={system}"));
            }
        } else {
            unsafe { env::set_var("PKG_CONFIG_PATH", env::join_paths(&pkgconfig).unwrap()) };
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
            .expect("install pangoft2 >= 1.50 for this target");
        bridge.includes(lib.include_paths);
    }
    bridge.compile("roukaku_text_pango");
    for line in metadata {
        println!("{line}");
    }
}

fn manifest_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("text crate is inside the engine workspace")
        .to_path_buf()
}
