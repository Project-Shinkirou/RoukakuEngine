use crate::util::{capture, env_file, run as command};
use anyhow::{Result, bail};
use clap::Args;
use std::{fs, path::Path, process::Command};

const REV: &str = "0c2a9ace1964b818b15f7ae50ba1c42404918a7b";
#[derive(Args)]
pub struct Options {
    #[arg(long)]
    pub target: String,
    #[arg(long)]
    pub ffmpeg: bool,
    #[arg(long)]
    pub subtitle: bool,
}

pub fn run(root: &Path, options: &Options) -> Result<()> {
    let triplet = match options.target.as_str() {
        "x86_64-pc-windows-msvc" => "x64-windows-static-md-dynbidi",
        "x86_64-unknown-linux-gnu" => "x64-linux",
        "aarch64-apple-darwin" => "arm64-osx",
        "x86_64-apple-darwin" => "x64-osx",
        "aarch64-linux-android" => "arm64-android-dynbidi",
        "x86_64-linux-android" => "x64-android-dynbidi",
        _ => bail!("unsupported dependency target {}", options.target),
    };
    if !options.ffmpeg && !options.subtitle {
        println!("No downloads selected; use --subtitle and/or --ffmpeg.");
        return Ok(());
    }
    let vcpkg = root.join(".work/vcpkg");
    if !vcpkg.exists() {
        fs::create_dir_all(root.join(".work"))?;
        command(
            Command::new("git")
                .current_dir(root)
                .args([
                    "clone",
                    "--no-checkout",
                    "https://github.com/microsoft/vcpkg.git",
                ])
                .arg(&vcpkg),
        )?;
        command(
            Command::new("git")
                .current_dir(&vcpkg)
                .args(["checkout", "--detach", REV]),
        )?;
    }
    let revision = capture(
        Command::new("git")
            .current_dir(&vcpkg)
            .args(["rev-parse", "HEAD"]),
    )?;
    if String::from_utf8_lossy(&revision.stdout).trim() != REV {
        bail!(
            "existing .work/vcpkg is not at the pinned revision; leave it intact and relocate it before retrying"
        );
    }
    let binary = vcpkg.join(if cfg!(windows) { "vcpkg.exe" } else { "vcpkg" });
    if !binary.is_file() {
        if cfg!(windows) {
            command(Command::new("cmd").current_dir(&vcpkg).args([
                "/C",
                "bootstrap-vcpkg.bat",
                "-disableMetrics",
            ]))?;
        } else {
            command(
                Command::new("sh")
                    .current_dir(&vcpkg)
                    .args(["bootstrap-vcpkg.sh", "-disableMetrics"]),
            )?;
        }
    }
    if options.subtitle {
        let manifest = root.join("Engine/Source/Runtime/roukaku-subtitle");
        let isolated = root.join(".work/vcpkg-subtitle");
        fs::create_dir_all(&isolated)?;
        command(
            Command::new(&binary)
                .current_dir(root)
                .arg("install")
                .arg(format!("--x-manifest-root={}", manifest.display()))
                .arg(format!(
                    "--overlay-triplets={}",
                    manifest.join("triplets").display()
                ))
                .arg(format!(
                    "--x-install-root={}",
                    isolated.join("installed").display()
                ))
                .arg(format!("--triplet={triplet}"))
                .args(["--clean-after-build", "--disable-metrics"])
                .env("VCPKG_DISABLE_METRICS", "1")
                .env("VCPKG_FORCE_SYSTEM_BINARIES", "1"),
        )?;
        fs::copy(vcpkg.join(".vcpkg-root"), isolated.join(".vcpkg-root"))?;
        env_file(
            "GITHUB_ENV",
            &format!("ROUKAKU_SUBTITLE_VCPKG_ROOT={}", isolated.display()),
        )?;
        for directory in ["bin", "debug/bin"] {
            env_file(
                "GITHUB_PATH",
                &isolated
                    .join("installed")
                    .join(triplet)
                    .join(directory)
                    .display()
                    .to_string(),
            )?;
        }
        if !options.target.contains("windows") {
            let base = isolated.join("installed").join(triplet);
            let mut pkg = vec![base.join("lib/pkgconfig"), base.join("share/pkgconfig")];
            pkg.extend(std::env::split_paths(
                &std::env::var_os("PKG_CONFIG_PATH").unwrap_or_default(),
            ));
            env_file(
                "GITHUB_ENV",
                &format!(
                    "PKG_CONFIG_PATH={}",
                    std::env::join_paths(pkg)?.to_string_lossy()
                ),
            )?;
            let key = if options.target.contains("apple") {
                "DYLD_LIBRARY_PATH"
            } else {
                "LD_LIBRARY_PATH"
            };
            let mut libs = vec![base.join("lib")];
            libs.extend(std::env::split_paths(
                &std::env::var_os(key).unwrap_or_default(),
            ));
            env_file(
                "GITHUB_ENV",
                &format!("{key}={}", std::env::join_paths(libs)?.to_string_lossy()),
            )?;
        }
    }
    if options.ffmpeg {
        let ffmpeg_triplet = match options.target.as_str() {
            "x86_64-pc-windows-msvc" => "x64-windows",
            "x86_64-unknown-linux-gnu" => "x64-linux",
            "aarch64-apple-darwin" => "arm64-osx",
            "x86_64-apple-darwin" => "x64-osx",
            _ => bail!("FFmpeg dependency preparation is not configured for this target"),
        };
        command(
            Command::new(&binary)
                .current_dir(&vcpkg)
                .arg("install")
                .arg(format!("ffmpeg:{ffmpeg_triplet}"))
                .args(["--classic", "--clean-after-build", "--disable-metrics"])
                .env("VCPKG_DISABLE_METRICS", "1"),
        )?;
        env_file("GITHUB_ENV", &format!("VCPKG_ROOT={}", vcpkg.display()))?;
        env_file(
            "GITHUB_ENV",
            &format!("VCPKG_DEFAULT_TRIPLET={ffmpeg_triplet}"),
        )?;
        for directory in ["bin", "debug/bin"] {
            env_file(
                "GITHUB_PATH",
                &vcpkg
                    .join("installed")
                    .join(ffmpeg_triplet)
                    .join(directory)
                    .display()
                    .to_string(),
            )?;
        }
    }
    Ok(())
}
