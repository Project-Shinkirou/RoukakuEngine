#[path = "../../Engine/Source/Runtime/roukaku-text/native_triplet.rs"]
mod native_triplet;

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
    #[arg(long)]
    pub text: bool,
}

pub fn run(root: &Path, options: &Options) -> Result<()> {
    let triplet = native_triplet::triplet(&options.target)
        .ok_or_else(|| anyhow::anyhow!("unsupported dependency target {}", options.target))?;
    if !options.ffmpeg && !options.subtitle && !options.text {
        println!("No downloads selected; use --text, --subtitle and/or --ffmpeg.");
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
    if options.subtitle || options.text {
        let manifest = root.join("Engine/Source/Runtime/roukaku-subtitle");
        let install_root = native_triplet::install_root(root, triplet);
        fs::create_dir_all(&install_root)?;
        fs::copy(vcpkg.join(".vcpkg-root"), install_root.join(".vcpkg-root"))?;
        command(
            Command::new(&binary)
                .current_dir(root)
                .arg("install")
                .arg(format!("--x-manifest-root={}", manifest.display()))
                .arg(format!(
                    "--overlay-triplets={}",
                    manifest.join("triplets").display()
                ))
                .arg(format!("--x-install-root={}", install_root.display()))
                .arg(format!("--triplet={triplet}"))
                .args([
                    "--clean-after-build",
                    "--disable-metrics",
                    "--x-wait-for-lock",
                ])
                .env("VCPKG_DISABLE_METRICS", "1")
                .env("VCPKG_FORCE_SYSTEM_BINARIES", "1"),
        )?;
        env_file(
            "GITHUB_ENV",
            &format!("ROUKAKU_SUBTITLE_VCPKG_ROOT={}", install_root.display()),
        )?;
        env_file(
            "GITHUB_ENV",
            &format!("ROUKAKU_TEXT_VCPKG_ROOT={}", install_root.display()),
        )?;
        let base = install_root.clone();
        for directory in ["bin", "debug/bin"] {
            env_file("GITHUB_PATH", &base.join(directory).display().to_string())?;
        }
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
        if options.target.contains("windows") {
            let pkgconf_root = vcpkg.join("downloads/tools/msys2");
            let pkgconf = fs::read_dir(pkgconf_root)?
                .filter_map(std::result::Result::ok)
                .map(|entry| entry.path().join("usr/bin/pkg-config.exe"))
                .find(|path| path.is_file())
                .ok_or_else(|| anyhow::anyhow!("vcpkg pkg-config executable is missing"))?;
            env_file("GITHUB_ENV", &format!("PKG_CONFIG={}", pkgconf.display()))?;
        }
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
