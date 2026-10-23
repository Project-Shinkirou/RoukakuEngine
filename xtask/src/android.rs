use crate::util::{capture, copy, hash, new_output, run as command};
use anyhow::{bail, Context, Result};
use clap::Args;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

const NDK: &str = "30.0.15729638";
const PROJECT: &str = "Engine/Source/Programs/roukaku-player-android/android";
#[derive(Args)]
pub struct Options {
    #[arg(long)]
    package: PathBuf,
    #[arg(long)]
    target: String,
    #[arg(long)]
    application_id: String,
    #[arg(long)]
    output: PathBuf,
    #[arg(long, default_value = "release")]
    profile: String,
    #[arg(long)]
    signing_properties: Option<PathBuf>,
}

pub fn run(root: &Path, options: &Options) -> Result<()> {
    let package = options
        .package
        .canonicalize()
        .context("package is missing")?;
    if !package.is_file() || package.extension().is_none_or(|e| e != "roukakupkg") {
        bail!("package must be a .roukakupkg file");
    }
    let id = regex::Regex::new(r"^[a-z][a-z0-9_]*(\.[a-z][a-z0-9_]*)+$")?;
    if !id.is_match(&options.application_id) {
        bail!("invalid Android application id");
    }
    if options.target.is_empty() || options.target.chars().any(char::is_control) {
        bail!("invalid product target id");
    }
    if !regex::Regex::new(r"^[A-Za-z0-9_-]+$")?.is_match(&options.profile) {
        bail!("invalid Cargo profile");
    }
    let signing = options
        .signing_properties
        .as_ref()
        .map(|p| p.canonicalize())
        .transpose()?;
    if signing.as_ref().is_some_and(|p| !p.is_file()) {
        bail!("signing properties are missing");
    }
    let java = std::env::var_os("JAVA_HOME")
        .map(|p| {
            PathBuf::from(p).join(if cfg!(windows) {
                "bin/java.exe"
            } else {
                "bin/java"
            })
        })
        .unwrap_or_else(|| PathBuf::from("java"));
    let java_output = capture(Command::new(&java).arg("-version"))?;
    let java_output = format!(
        "{}{}",
        String::from_utf8_lossy(&java_output.stderr),
        String::from_utf8_lossy(&java_output.stdout)
    );
    let jdk = regex::Regex::new(r#"version "(17(?:\.[^" ]*)?)""#)?
        .captures(&java_output)
        .context("JDK 17 is required")?[1]
        .to_string();
    let sdk = PathBuf::from(
        std::env::var_os("ANDROID_SDK_ROOT")
            .or_else(|| std::env::var_os("ANDROID_HOME"))
            .context("ANDROID_SDK_ROOT or ANDROID_HOME is required")?,
    )
    .canonicalize()?;
    for relative in [
        "platforms/android-36".to_string(),
        "build-tools/36.0.0".to_string(),
        format!("ndk/{NDK}"),
    ] {
        if !sdk.join(&relative).is_dir() {
            bail!("Android SDK component is missing: {relative}");
        }
    }
    let prebuilts: Vec<_> = fs::read_dir(sdk.join(format!("ndk/{NDK}/toolchains/llvm/prebuilt")))?
        .collect::<std::io::Result<Vec<_>>>()?
        .into_iter()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    if prebuilts.len() != 1 {
        bail!("NDK host prebuilt is missing or ambiguous");
    }
    let cxx = prebuilts[0].join("sysroot/usr/lib/aarch64-linux-android/libc++_shared.so");
    if !cxx.is_file() {
        bail!("NDK libc++ shared runtime is missing");
    }
    let output = if options.output.is_absolute() {
        options.output.clone()
    } else {
        std::env::current_dir()?.join(&options.output)
    };
    new_output(&output)?;
    let result = build(
        root,
        options,
        &package,
        &output,
        &sdk,
        &cxx,
        &jdk,
        signing.as_deref(),
    );
    if result.is_err() {
        fs::remove_dir_all(&output).context("cleaning failed Android staging")?;
    }
    result
}

#[allow(clippy::too_many_arguments)]
fn build(
    root: &Path,
    options: &Options,
    package: &Path,
    output: &Path,
    sdk: &Path,
    cxx: &Path,
    jdk: &str,
    signing: Option<&Path>,
) -> Result<()> {
    let jni = output.join("jniLibs");
    let target_dir = output.join("cargo-target");
    command(
        Command::new("cargo")
            .current_dir(root)
            .args(["ndk", "-t", "arm64-v8a", "--platform", "28", "-o"])
            .arg(&jni)
            .args([
                "build",
                "--locked",
                "-p",
                "roukaku-player-android",
                "--profile",
                &options.profile,
            ])
            .env("CARGO_TARGET_DIR", &target_dir)
            .env("ANDROID_NDK_HOME", sdk.join(format!("ndk/{NDK}"))),
    )?;
    let abi = jni.join("arm64-v8a");
    if !abi.join("libroukaku_player_android.so").is_file() {
        bail!("cargo-ndk did not produce the Player JNI library");
    }
    copy(cxx, &abi.join("libc++_shared.so"))?;
    let project = root.join(PROJECT);
    let gradle = project.join(if cfg!(windows) {
        "gradlew.bat"
    } else {
        "gradlew"
    });
    let mut c = if cfg!(windows) {
        let mut c = Command::new("cmd");
        c.arg("/C").arg(&gradle);
        c
    } else {
        let mut c = Command::new("sh");
        c.arg(&gradle);
        c
    };
    c.current_dir(&project)
        .args([
            "--no-daemon",
            "clean",
            "assembleDebug",
            "assembleRelease",
            "bundleRelease",
        ])
        .arg(format!("-ProukakuRustJniDir={}", jni.display()))
        .arg(format!("-ProukakuBundledPackage={}", package.display()))
        .arg(format!("-ProukakuApplicationId={}", options.application_id));
    if let Some(path) = signing {
        c.arg(format!("-ProukakuSigningProperties={}", path.display()));
    }
    command(&mut c)?;
    let artifacts = output.join("artifacts");
    fs::create_dir(&artifacts)?;
    let release_apk = if signing.is_some() {
        "app/build/outputs/apk/release/app-release.apk"
    } else {
        "app/build/outputs/apk/release/app-release-unsigned.apk"
    };
    for relative in [
        "app/build/outputs/apk/debug/app-debug.apk",
        release_apk,
        "app/build/outputs/bundle/release/app-release.aab",
    ] {
        let source = project.join(relative);
        copy(
            &source,
            &artifacts.join(source.file_name().context("artifact name")?),
        )?;
    }
    let mut files = Vec::new();
    for directory in [&artifacts, &jni] {
        for entry in walkdir::WalkDir::new(directory) {
            let entry = entry?;
            if entry.file_type().is_symlink() {
                bail!("Android output contains a link");
            }
            if entry.file_type().is_file() {
                files.push(serde_json::json!({"path": entry.path().strip_prefix(output)?.to_string_lossy().replace('\\', "/"), "sha256": hash(entry.path())?, "size": entry.metadata()?.len()}));
            }
        }
    }
    files.sort_by(|a, b| a["path"].as_str().cmp(&b["path"].as_str()));
    let commit = capture(
        Command::new("git")
            .current_dir(root)
            .args(["rev-parse", "HEAD"]),
    )?;
    let manifest = serde_json::json!({"schema":"roukaku.android_bundle_manifest.v1", "target":options.target, "profile":options.profile,
        "application_id":options.application_id, "package_sha256":hash(package)?, "source_commit":String::from_utf8_lossy(&commit.stdout).trim(),
        "cargo_lock_sha256":hash(&root.join("Cargo.lock"))?, "jdk_version":jdk, "ndk_version":NDK, "compile_sdk":36, "min_sdk":28,
        "shipping_abis":["arm64-v8a"], "debug_apk_signing":"debug", "release_signing":if signing.is_some(){"external"}else{"unsigned"}, "files":files});
    fs::write(
        output.join("android-bundle-manifest.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    Ok(())
}
