use crate::util::{capture, run as command};
use anyhow::{bail, Context, Result};
use clap::{Args, ValueEnum};
use std::{path::Path, process::Command};

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
    /// Rust target triple. Apple targets use cross on Linux and Cargo on macOS.
    #[arg(long)]
    target: Option<String>,
    #[arg(long)]
    release: bool,
}

pub fn run(root: &Path, task: &str, selected: &Selection) -> Result<()> {
    if selected.hardware && !matches!(task, "test" | "check") {
        bail!("--hardware is supported only by test and check");
    }
    if let Some(target) = &selected.target {
        if task != "build" {
            bail!("--target is supported by build; use cross for target check/clippy");
        }
        let workspace = match selected.workspace {
            Workspace::Engine => root.to_path_buf(),
            Workspace::Editor => root.join("Editor"),
            Workspace::All => bail!("target builds require an explicit workspace"),
        };
        let packages = if selected.packages.is_empty() {
            vec![if matches!(selected.workspace, Workspace::Editor) {
                "roukaku-editor"
            } else {
                "roukaku-player"
            }
            .into()]
        } else {
            selected.packages.clone()
        };
        let mut cmd = if crate::apple::supported(target) {
            crate::apple::command(root, target)?
        } else {
            Command::new("cargo")
        };
        cmd.current_dir(workspace).args([
            "build",
            "--locked",
            "--target",
            target,
            "--message-format=json-render-diagnostics",
        ]);
        for package in packages {
            cmd.args(["-p", &package]);
        }
        for feature in &selected.features {
            cmd.args(["--features", feature]);
        }
        if selected.release {
            cmd.arg("--release");
        }
        return command(&mut cmd);
    }
    if selected.release {
        bail!("--release requires --target");
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
    let mut cmd = crate::apple::command(root, &options.target)?;
    cmd.current_dir(root)
        .args([&options.task, "--locked", "--target", &options.target])
        .args(&options.args)
        .env(
            "CARGO_TARGET_DIR",
            root.join(".work/cross").join(&options.target),
        );
    command(&mut cmd)
}
