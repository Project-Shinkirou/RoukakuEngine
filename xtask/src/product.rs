use crate::util::{capture, run as command};
use anyhow::{bail, Context, Result};
use clap::{Args, ValueEnum};
use std::{
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Clone, Copy, ValueEnum)]
pub enum Workspace {
    Engine,
    Editor,
    All,
}
#[derive(Args)]
pub struct Selection {
    #[arg(long, value_enum, default_value = "all")]
    workspace: Workspace,
    #[arg(short = 'p', long = "package")]
    packages: Vec<String>,
    #[arg(long)]
    features: Vec<String>,
    /// Run ignored hardware-dependent tests on a machine with the required devices.
    #[arg(long)]
    hardware: bool,
}

pub fn run(root: &Path, task: &str, selected: &Selection) -> Result<()> {
    if selected.hardware && !matches!(task, "test" | "check") {
        bail!("--hardware is supported only by test and check");
    }
    if !selected.packages.is_empty() && matches!(selected.workspace, Workspace::All) {
        bail!("--package requires an explicit workspace");
    }
    if matches!(task, "check" | "check-fast") {
        crate::docs::run(root)?;
    }
    if task == "check-fast" {
        return Ok(());
    }
    let workspaces = match selected.workspace {
        Workspace::Engine => vec![root.to_path_buf()],
        Workspace::Editor => vec![root.join("Editor")],
        Workspace::All => vec![root.to_path_buf(), root.join("Editor")],
    };
    for workspace in workspaces {
        let packages = members(&workspace, &selected.packages)?;
        if matches!(task, "fmt" | "check") {
            // cargo-fmt delegates trailing arguments to rustfmt. Cargo's global locked flag precedes the subcommand.
            let mut c = Command::new("cargo");
            c.current_dir(&workspace)
                .args(["--locked", "fmt", "--check"]);
            for package in &packages {
                c.args(["--package", package]);
            }
            command(&mut c)?;
        }
        if matches!(task, "lint" | "check") {
            cargo(
                &workspace,
                "clippy",
                &packages,
                &selected.features,
                &["--all-targets", "--", "-D", "warnings"],
            )?;
        }
        if matches!(task, "build" | "check") || (task == "test" && selected.packages.is_empty()) {
            let products: Vec<_> = packages
                .iter()
                .filter(|p| p.as_str() != "xtask")
                .cloned()
                .collect();
            if !products.is_empty() {
                cargo(&workspace, "build", &products, &selected.features, &[])?;
            }
        }
        if matches!(task, "test" | "check") {
            let extra: &[&str] = if selected.hardware {
                &["--", "--include-ignored"]
            } else {
                &[]
            };
            cargo(&workspace, "test", &packages, &selected.features, extra)?;
        }
    }
    Ok(())
}

fn members(root: &Path, requested: &[String]) -> Result<Vec<String>> {
    let output = capture(Command::new("cargo").current_dir(root).args([
        "metadata",
        "--locked",
        "--no-deps",
        "--format-version",
        "1",
    ]))?;
    let metadata: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    let ids = metadata["workspace_members"]
        .as_array()
        .context("workspace members")?;
    let packages = metadata["packages"]
        .as_array()
        .context("packages")?
        .iter()
        .filter(|p| ids.contains(&p["id"]))
        .map(|p| p["name"].as_str().context("package name").map(String::from))
        .collect::<Result<Vec<_>>>()?;
    for name in requested {
        if !packages.contains(name) {
            bail!("{name} is not a member of the selected workspace");
        }
    }
    Ok(packages
        .into_iter()
        .filter(|p| requested.is_empty() || requested.contains(p))
        .collect())
}

fn cargo(
    root: &Path,
    task: &str,
    packages: &[String],
    features: &[String],
    extra: &[&str],
) -> Result<()> {
    let mut c = Command::new("cargo");
    c.current_dir(root).args([task, "--locked"]);
    for package in packages {
        c.args(["--package", package]);
    }
    for feature in features {
        c.args(["--features", feature]);
    }
    c.args(extra);
    command(&mut c)
}

#[derive(Args)]
pub struct Cross {
    #[arg(long)]
    target: String,
    #[arg(value_parser = ["check", "clippy"])]
    task: String,
    #[arg(last = true)]
    args: Vec<String>,
}

pub fn cross(root: &Path, options: &Cross) -> Result<()> {
    let (cc, cxx, arch) = match options.target.as_str() {
        "aarch64-apple-darwin" => ("oa64-clang", "oa64-clang++", "aarch64"),
        "x86_64-apple-darwin" => ("o64-clang", "o64-clang++", "x86_64"),
        _ => bail!("cross supports only macOS x64 and arm64"),
    };
    let toolchain = PathBuf::from(
        std::env::var_os("ROUKAKU_OSXCROSS_ROOT").context("ROUKAKU_OSXCROSS_ROOT is required")?,
    )
    .canonicalize()?;
    let bin = toolchain.join("bin");
    let sdk = if let Some(value) = std::env::var_os("SDKROOT") {
        PathBuf::from(value)
    } else {
        let sdks: Vec<_> = std::fs::read_dir(toolchain.join("SDK"))?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.join("SDKSettings.json").is_file())
            .collect();
        if sdks.len() != 1 {
            bail!("set SDKROOT to one explicit macOS SDK");
        }
        sdks[0].clone()
    };
    let mut ars: Vec<_> = std::fs::read_dir(&bin)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name().is_some_and(|n| {
                let n = n.to_string_lossy();
                n.starts_with(&format!("{arch}-apple-darwin")) && n.ends_with("-ar")
            })
        })
        .collect();
    ars.sort();
    if !bin.join(cc).is_file()
        || !bin.join(cxx).is_file()
        || ars.len() != 1
        || !sdk.join("SDKSettings.json").is_file()
    {
        bail!("osxcross compiler, archiver or SDK is missing/ambiguous");
    }
    let installed = capture(Command::new("rustup").args(["target", "list", "--installed"]))?;
    if !String::from_utf8_lossy(&installed.stdout)
        .lines()
        .any(|t| t == options.target)
    {
        bail!("install Rust target {} explicitly", options.target);
    }
    let target_env = options.target.replace('-', "_");
    let mut paths = vec![bin.clone()];
    paths.extend(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    ));
    let mut c = Command::new("cargo");
    c.current_dir(root)
        .args([&options.task, "--locked", "--target", &options.target])
        .args(&options.args)
        .env("PATH", std::env::join_paths(paths)?)
        .env("SDKROOT", sdk)
        .env("MACOSX_DEPLOYMENT_TARGET", "13.0")
        .env(format!("CC_{target_env}"), bin.join(cc))
        .env(format!("CXX_{target_env}"), bin.join(cxx))
        .env(format!("AR_{target_env}"), &ars[0])
        .env(
            format!("CARGO_TARGET_{}_LINKER", target_env.to_uppercase()),
            bin.join(cc),
        )
        .env(
            "CARGO_TARGET_DIR",
            root.join(".work/cross").join(&options.target),
        );
    command(&mut c)
}
