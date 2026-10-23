mod android;
mod dependencies;
mod docs;
mod product;
mod release;
mod ui;
mod util;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(about = "RoukakuEngine product development tools")]
struct Cli {
    #[command(subcommand)]
    command: Task,
}

#[derive(Subcommand)]
enum Task {
    /// Check documentation links and hygiene.
    Docs,
    Fmt(product::Selection),
    Lint(product::Selection),
    Build(product::Selection),
    Test(product::Selection),
    Check(product::Selection),
    CheckFast(product::Selection),
    /// Explicitly download and build pinned native dependencies; never installs system packages.
    Deps(dependencies::Options),
    /// Download the pinned Luau analyzer and check NativeVN controllers.
    UiCheck,
    /// Build Android JNI libraries and real Gradle artifacts.
    Android(android::Options),
    /// Run Cargo using an explicitly configured osxcross installation.
    Cross(product::Cross),
    /// Build and stage desktop release binaries from Cargo artifact messages.
    Release {
        #[arg(long)]
        target: String,
        #[arg(long, value_parser = release::parse_version)]
        version: String,
        #[arg(long)]
        output: PathBuf,
    },
    /// Aggregate successful release artifacts and publish only when requested.
    ReleaseCollect {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        output: PathBuf,
        #[arg(long, value_parser = release::parse_version)]
        version: String,
        #[arg(long)]
        publish: bool,
    },
}

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("root xtask")
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Task::Docs => docs::run(root()),
        Task::Fmt(s) => product::run(root(), "fmt", &s),
        Task::Lint(s) => product::run(root(), "lint", &s),
        Task::Build(s) => product::run(root(), "build", &s),
        Task::Test(s) => product::run(root(), "test", &s),
        Task::Check(s) => product::run(root(), "check", &s),
        Task::CheckFast(s) => product::run(root(), "check-fast", &s),
        Task::Deps(o) => dependencies::run(root(), &o),
        Task::UiCheck => ui::run(root()),
        Task::Android(o) => android::run(root(), &o),
        Task::Cross(o) => product::cross(root(), &o),
        Task::Release {
            target,
            version,
            output,
        } => release::build(root(), &target, &version, &output),
        Task::ReleaseCollect {
            input,
            output,
            version,
            publish,
        } => release::collect(root(), &input, &output, &version, publish),
    }
}
