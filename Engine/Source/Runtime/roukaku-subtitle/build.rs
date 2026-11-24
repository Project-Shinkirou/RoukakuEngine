use std::env;

fn main() {
    println!("cargo:rerun-if-changed=src/bridge.c");
    let includes = env::var_os("DEP_ROUKAKU_LIBASS_INCLUDE_DIRS").expect("libass include paths");
    let windows = env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows");
    let mut bridge = cc::Build::new();
    bridge
        .file("src/bridge.c")
        .includes(env::split_paths(&includes))
        .warnings(true);
    if windows {
        bridge.static_crt(false);
    }
    // bridge.c checks LIBASS_VERSION against 0x01705000 using the actual
    // installed header; it also uses the official track layout and va_list.
    bridge.compile("roukaku_subtitle_bridge");
}
