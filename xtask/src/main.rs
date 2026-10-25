mod android;
mod apple;
mod apple_package;
mod dependencies;
mod docs;
mod product;
mod project_player;
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
    /// Build a project's linked Player, or stage a pure-content prebuilt Player.
    ProjectPlayer(project_player::Options),
    /// Explicitly download and build pinned native dependencies; never installs system packages.
    Deps(dependencies::Options),
    /// Download the pinned Luau analyzer and check NativeVN controllers.
    UiCheck,
    /// Build Android JNI libraries and real Gradle artifacts.
    Android(android::Options),
    /// Run Cargo/cross with the private Apple toolchain image on Linux.
    Cross(product::Cross),
    /// Build a private Apple cross image from SHINKI_APPLE_SDK_DIR.
    AppleImage {
        #[arg(long)]
        target: String,
    },
    /// Package a built Apple Player and generated game resources.
    Package(apple_package::Options),
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
        Task::ProjectPlayer(options) => project_player::run(&options),
        Task::Deps(o) => dependencies::run(root(), &o),
        Task::UiCheck => ui::run(root()),
        Task::Android(o) => android::run(root(), &o),
        Task::Cross(o) => product::cross(root(), &o),
        Task::AppleImage { target } => apple::image(root(), &target),
        Task::Package(options) => apple_package::run(root(), &options),
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
