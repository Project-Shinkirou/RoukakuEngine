use anyhow::{bail, Context, Result};
use clap::Args;
use std::{
    fs,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

#[derive(Args)]
pub struct Options {
    pub project: PathBuf,
    #[arg(long)]
    pub output: PathBuf,
    #[arg(long)]
    pub target: Option<String>,
    /// Prebuilt Player for projects without a player/Cargo.toml.
    #[arg(long)]
    pub prebuilt: Option<PathBuf>,
}

pub fn run(options: &Options) -> Result<()> {
    let project = options
        .project
        .canonicalize()
        .context("project directory")?;
    let manifest = project.join("player/Cargo.toml");
    if !manifest.exists() {
        let prebuilt = options
            .prebuilt
            .as_ref()
            .context("pure-content project requires --prebuilt Player")?;
        fs::copy(prebuilt, &options.output).context("copying prebuilt Player")?;
        return Ok(());
    }
    if options.prebuilt.is_some() {
        bail!("project has Rust extensions: --prebuilt cannot replace its compiled Player");
    }
    let mut command = Command::new("cargo");
    command
        .current_dir(&project)
        .args([
            "build",
            "--locked",
            "--release",
            "--message-format=json-render-diagnostics",
            "--manifest-path",
        ])
        .arg(&manifest)
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    if let Some(target) = &options.target {
        command.args(["--target", target]);
    }
    let mut child = command
        .spawn()
        .context("building project-specific Player")?;
    let parsed = (|| -> Result<PathBuf> {
        let mut executable = None;
        for line in BufReader::new(child.stdout.take().context("Cargo stdout")?).lines() {
            let value: serde_json::Value = serde_json::from_str(&line?)?;
            if value["reason"] == "compiler-message" {
                if let Some(rendered) = value["message"]["rendered"].as_str() {
                    eprint!("{rendered}");
                }
            }
            if value["reason"] == "compiler-artifact"
                && value["manifest_path"].as_str().is_some_and(|path| {
                    Path::new(path).canonicalize().ok().as_ref() == Some(&manifest)
                })
            {
                if let Some(path) = value["executable"].as_str() {
                    if executable.replace(PathBuf::from(path)).is_some() {
                        bail!("project player crate must declare exactly one binary");
                    }
                }
            }
        }
        executable.context("Cargo produced no project Player executable")
    })();
    if parsed.is_err() {
        let _ = child.kill();
    }
    let status = child.wait()?;
    if !status.success() {
        bail!("project Player build failed: {status}");
    }
    fs::copy(parsed?, &options.output).context("staging project-specific Player")?;
    Ok(())
}
