use std::{env, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-env-changed=ROUKAKU_SUBTITLE_VCPKG_ROOT");
    let windows = env::var("CARGO_CFG_TARGET_OS").unwrap() == "windows";
    let (includes, libraries) = if windows {
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
        let root = PathBuf::from(
            env::var_os("ROUKAKU_SUBTITLE_VCPKG_ROOT")
                .expect("set ROUKAKU_SUBTITLE_VCPKG_ROOT to the isolated subtitle vcpkg root"),
        );
        let lib = vcpkg::Config::new()
            .vcpkg_root(root)
            .target_triplet("x64-windows-static-md-dynbidi")
            .copy_dlls(false)
            .find_package("libass")
            .expect("install libass 0.17.5 in the isolated subtitle vcpkg root");
        assert!(
            lib.is_static,
            "libass must be static; only fribidi may be dynamic"
        );
        for required in ["libass", "freetype", "fribidi", "harfbuzz"] {
            assert!(
                lib.ports.iter().any(|port| port == required),
                "libass installation is missing a required transitive port"
            );
        }
        (lib.include_paths, lib.link_paths)
    } else {
        let lib = pkg_config::Config::new()
            .exactly_version("0.17.5")
            .probe("libass")
            .expect("roukaku-subtitle requires native libass exactly 0.17.5");
        (lib.include_paths, lib.link_paths)
    };
    if windows {
        // vcpkg-rs omits system libraries used by libass's desktop font
        // provider, even though provider lookup is disabled at runtime.
        println!("cargo:rustc-link-lib=gdi32");
        println!("cargo:rustc-link-lib=user32");
    }
    println!("cargo::metadata=include_dirs={}", env::join_paths(includes).unwrap().to_string_lossy());
    let pkg_config_dirs = libraries.iter().map(|path| path.join("pkgconfig"));
    println!("cargo::metadata=pkg_config_dirs={}", env::join_paths(pkg_config_dirs).unwrap().to_string_lossy());
    println!("cargo::metadata=lib_dirs={}", env::join_paths(libraries).unwrap().to_string_lossy());
}
