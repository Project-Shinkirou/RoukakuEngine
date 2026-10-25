use crate::util::{capture, copy, hash, new_output};
use anyhow::{Context, Result, bail};
use std::{
    collections::BTreeSet,
    fs,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

pub fn parse_version(value: &str) -> std::result::Result<String, String> {
    let value = value.strip_prefix('v').unwrap_or(value);
    semver::Version::parse(value)
        .map(|_| value.to_owned())
        .map_err(|e| e.to_string())
}
fn version(value: &str) -> Result<()> {
    semver::Version::parse(value).context("release version must be semver")?;
    Ok(())
}

pub fn build(root: &Path, target: &str, ver: &str, output: &Path) -> Result<()> {
    version(ver)?;
    let host = capture(Command::new("rustc").arg("-vV"))?;
    let rustc = String::from_utf8(host.stdout)?;
    if !crate::apple::supported(target)
        && !rustc.lines().any(|line| line == format!("host: {target}"))
    {
        bail!("desktop release requires a native runner matching {target}");
    }
    if !matches!(
        target,
        "x86_64-pc-windows-msvc"
            | "x86_64-unknown-linux-gnu"
            | "x86_64-apple-darwin"
            | "aarch64-apple-darwin"
    ) {
        bail!("desktop release target is unsupported");
    }
    new_output(output)?;
    let result = stage(root, target, ver, output, &rustc);
    if result.is_err() {
        fs::remove_dir_all(output).context("cleaning failed release staging")?;
    }
    result
}

fn stage(root: &Path, target: &str, ver: &str, output: &Path, rustc: &str) -> Result<()> {
    let temp = tempfile::tempdir_in(output)?;
    let stage = temp.path();
    let bin = stage.join("bin");
    fs::create_dir(&bin)?;
    let mut found = BTreeSet::new();
    for (workspace, packages) in [
        (root.to_path_buf(), vec!["roukaku-player", "roukaku-cli"]),
        (root.join("Editor"), vec!["roukaku-editor"]),
    ] {
        let mut c = if crate::apple::supported(target) {
            crate::apple::command(root, target)?
        } else {
            Command::new("cargo")
        };
        c.current_dir(&workspace).args([
            "build",
            "--locked",
            "--release",
            "--target",
            target,
            "--message-format=json-render-diagnostics",
        ]);
        for package in packages {
            c.args(["-p", package]);
        }
        c.env(
            "CARGO_TARGET_DIR",
            root.join(".work/release-target").join(target),
        )
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
        let mut child = c.spawn()?;
        let parsed = (|| -> Result<()> {
            for line in BufReader::new(child.stdout.take().context("Cargo stdout")?).lines() {
                let line = line?;
                let Ok(value) = serde_json::from_str::<serde_json::Value>(&line) else {
                    continue;
                };
                if value["reason"] == "compiler-message"
                    && let Some(rendered) = value["message"]["rendered"].as_str()
                {
                    eprint!("{rendered}");
                }
                if value["reason"] != "compiler-artifact" {
                    continue;
                }
                let Some(name) = value["target"]["name"].as_str() else {
                    continue;
                };
                if !matches!(name, "roukaku" | "roukaku-player" | "roukaku-editor") {
                    continue;
                }
                let Some(executable) = value["executable"].as_str() else {
                    continue;
                };
                let source = Path::new(executable);
                copy(
                    source,
                    &bin.join(source.file_name().context("binary name")?),
                )?;
                found.insert(name.to_string());
            }
            Ok(())
        })();
        if parsed.is_err() {
            let _ = child.kill();
        }
        let status = child.wait()?;
        parsed?;
        if !status.success() {
            bail!("release build failed for {target}: {status}");
        }
    }
    if found.len() != 3 {
        bail!("Cargo did not produce all Player, Editor and CLI binaries");
    }
    let licenses = stage.join("licenses");
    fs::create_dir(&licenses)?;
    for file in ["LICENSE-MIT", "LICENSE-APACHE", "ORIGINS.md"] {
        copy(&root.join(file), &licenses.join(file))?;
    }
    if crate::apple::supported(target) {
        for entry in walkdir::WalkDir::new(crate::apple_package::native_share(root, target)?)
            .follow_links(false)
        {
            let entry = entry?;
            if entry.file_name() == "copyright" && entry.file_type().is_file() {
                let port = entry
                    .path()
                    .parent()
                    .and_then(Path::file_name)
                    .context("Apple native license port")?;
                copy(
                    entry.path(),
                    &licenses.join(format!("{}-copyright", port.to_string_lossy())),
                )?;
            }
        }
    }
    // The subtitle owner has one dynamic Windows dependency. Ship it with its port licenses.
    if target.contains("windows") {
        let native = PathBuf::from(
            std::env::var_os("ROUKAKU_SUBTITLE_VCPKG_ROOT")
                .context("subtitle dependency root is required for release")?,
        )
        .join("installed/x64-windows-static-md-dynbidi");
        let mut dlls = 0;
        for entry in fs::read_dir(native.join("bin"))? {
            let path = entry?.path();
            if path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("dll"))
            {
                copy(&path, &bin.join(path.file_name().context("DLL name")?))?;
                dlls += 1;
            }
        }
        if dlls == 0 {
            bail!("subtitle runtime DLL is missing");
        }
        for entry in walkdir::WalkDir::new(native.join("share")) {
            let entry = entry?;
            if entry.file_name() == "copyright" && entry.file_type().is_file() {
                let port = entry
                    .path()
                    .parent()
                    .and_then(Path::file_name)
                    .context("vcpkg port name")?;
                copy(
                    entry.path(),
                    &licenses.join(format!("{}-copyright", port.to_string_lossy())),
                )?;
            }
        }
    }
    let commit = capture(
        Command::new("git")
            .current_dir(root)
            .args(["rev-parse", "HEAD"]),
    )?;
    fs::write(
        stage.join("build.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"version":ver,"target":target,"source_commit":String::from_utf8_lossy(&commit.stdout).trim(),"rustc":rustc,"profile":"release","features":"package defaults","cargo_lock_sha256":hash(&root.join("Cargo.lock"))?,"editor_lock_sha256":hash(&root.join("Editor/Cargo.lock"))?}),
        )?,
    )?;
    if crate::apple::supported(target) {
        for (executable, name, id) in [
            (
                "roukaku-player",
                "RoukakuPlayer",
                "org.shinkirou.roukakuplayer",
            ),
            (
                "roukaku-editor",
                "RoukakuEditor",
                "org.shinkirou.roukakueditor",
            ),
        ] {
            let app_stage = tempfile::tempdir_in(stage)?;
            copy(&bin.join(executable), &app_stage.path().join(executable))?;
            for entry in fs::read_dir(&bin)? {
                let path = entry?.path();
                if path
                    .extension()
                    .is_some_and(|extension| extension == "dylib")
                {
                    copy(
                        &path,
                        &app_stage
                            .path()
                            .join(path.file_name().context("dylib filename")?),
                    )?;
                }
            }
            let app = crate::apple::bundle(app_stage.path(), target, executable, name, id, ver)?;
            fs::rename(&app, stage.join(app.file_name().context("app filename")?))?;
            fs::remove_file(bin.join(executable))?;
        }
    }
    let archive_path = output.join(format!("roukaku-{ver}-{target}.zip"));
    archive(stage, &archive_path)?;
    fs::write(
        output.join(format!("{target}-SHA256SUMS")),
        format!(
            "{}  {}\n",
            hash(&archive_path)?.trim_start_matches("sha256:"),
            archive_path.file_name().unwrap().to_string_lossy()
        ),
    )?;
    Ok(())
}

pub(crate) fn archive(source: &Path, destination: &Path) -> Result<()> {
    let mut writer = zip::ZipWriter::new(
        fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(destination)?,
    );
    let mut entries = walkdir::WalkDir::new(source)
        .into_iter()
        .collect::<std::result::Result<Vec<_>, _>>()?;
    entries.sort_by(|a, b| a.path().cmp(b.path()));
    for entry in entries {
        if entry.file_type().is_symlink() {
            bail!("release staging contains a link");
        }
        if !entry.file_type().is_file() {
            continue;
        }
        let name = entry
            .path()
            .strip_prefix(source)?
            .to_string_lossy()
            .replace('\\', "/");
        let mut options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        if name.starts_with("bin/")
            || name.contains(".app/Contents/MacOS/")
            || name.ends_with(".app/roukaku-player")
        {
            options = options.unix_permissions(0o755);
        }
        writer.start_file(name, options)?;
        std::io::copy(&mut fs::File::open(entry.path())?, &mut writer)?;
    }
    writer.finish()?;
    Ok(())
}

pub fn collect(root: &Path, input: &Path, output: &Path, ver: &str, publish: bool) -> Result<()> {
    version(ver)?;
    if !input.is_dir() {
        bail!("release artifact input is missing");
    }
    new_output(output)?;
    let result = collect_files(input, output, ver);
    if result.is_err() {
        fs::remove_dir_all(output)?;
    }
    result?;
    if publish {
        publish_release(root, output, ver)?;
    }
    Ok(())
}

fn collect_files(input: &Path, output: &Path, ver: &str) -> Result<()> {
    let mut paths = Vec::new();
    let mut names = BTreeSet::new();
    for entry in walkdir::WalkDir::new(input) {
        let entry = entry?;
        if entry.file_type().is_symlink() {
            bail!("release input contains a link");
        }
        if !entry.file_type().is_file() || entry.path().extension().is_none_or(|e| e != "zip") {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with(&format!("roukaku-{ver}-")) || !names.insert(name.to_lowercase()) {
            bail!("unexpected or duplicate release artifact");
        }
        paths.push(entry.into_path());
    }
    if paths.is_empty() {
        bail!("no real release artifacts; refusing empty release");
    }
    paths.sort();
    let mut sums = String::new();
    for path in paths {
        let name = path.file_name().context("archive name")?;
        let dest = output.join(name);
        copy(&path, &dest)?;
        sums.push_str(&format!(
            "{}  {}\n",
            hash(&dest)?.trim_start_matches("sha256:"),
            name.to_string_lossy()
        ));
    }
    fs::write(output.join("SHA256SUMS"), sums)?;
    fs::write(
        output.join("release-notes.md"),
        format!(
            "# RoukakuEngine {ver}\n\nNative Player, Editor and CLI artifacts are attached for successful desktop builds. See each archive's build.json for source and toolchain identity.\n\n## Blocked standalone release platforms\n- Android: requires an explicitly supplied .roukakupkg and application id; use cargo xtask android. Debug APKs use debug signing; release artifacts require external signing.\n- iOS: no native Player host/signing integration.\n- OpenHarmony: no native Player host integration.\n\nSHA256SUMS covers every attached product archive.\n"
        ),
    )?;
    Ok(())
}

fn publish_release(_root: &Path, output: &Path, ver: &str) -> Result<()> {
    let token =
        std::env::var("GITHUB_TOKEN").context("GITHUB_TOKEN is required for publication")?;
    let repository = std::env::var("GITHUB_REPOSITORY").context("GITHUB_REPOSITORY is required")?;
    if repository != "Project-Shinkirou/RoukakuEngine" {
        bail!("release publication repository is not Project-Shinkirou/RoukakuEngine");
    }
    if std::env::var("GITHUB_REF").ok().as_deref() != Some(&format!("refs/tags/v{ver}")) {
        bail!("publication requires the matching version tag");
    }
    tokio::runtime::Builder::new_current_thread().enable_all().build()?.block_on(async {
        let client = reqwest::Client::builder().https_only(true).user_agent("RoukakuEngine-xtask").build()?;
        let base = format!("https://api.github.com/repos/{repository}");
        let response = client.post(format!("{base}/releases")).bearer_auth(&token).json(&serde_json::json!({"tag_name":format!("v{ver}"),"name":format!("RoukakuEngine {ver}"),"body":fs::read_to_string(output.join("release-notes.md"))?,"draft":true,"prerelease":!semver::Version::parse(ver)?.pre.is_empty()})).send().await?.error_for_status()?;
        let release: serde_json::Value = response.json().await?;
        let id = release["id"].as_u64().context("GitHub release id")?;
        // Upload only known staged assets. Failed uploads leave a draft, never a public partial release.
        for entry in fs::read_dir(output)? {
            let path = entry?.path();
            let name = path.file_name().context("asset filename")?.to_string_lossy();
            if path.extension().is_none_or(|e|e != "zip") && name != "SHA256SUMS" { continue; }
            let encoded = percent_encoding::utf8_percent_encode(&name, percent_encoding::NON_ALPHANUMERIC);
            let url = format!("https://uploads.github.com/repos/{repository}/releases/{id}/assets?name={encoded}");
            client.post(url).bearer_auth(&token).header("Content-Type","application/octet-stream").body(fs::read(path)?).send().await?.error_for_status()?;
        }
        client.patch(format!("{base}/releases/{id}")).bearer_auth(token).json(&serde_json::json!({"draft":false})).send().await?.error_for_status()?;
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_or_colliding_release_inputs_cannot_publish() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let input = dir.path().join("input");
        fs::create_dir(&input)?;
        let output = dir.path().join("output");
        assert!(collect(dir.path(), &input, &output, "0.1.0", false).is_err());
        assert!(!output.exists());
        for name in ["a", "b"] {
            fs::create_dir(input.join(name))?;
            fs::write(input.join(name).join("roukaku-0.1.0-x.zip"), b"fixture")?;
        }
        assert!(collect(dir.path(), &input, &output, "0.1.0", false).is_err());
        assert!(!output.exists());
        assert_eq!(fs::read(input.join("a/roukaku-0.1.0-x.zip"))?, b"fixture");
        Ok(())
    }
    #[test]
    fn archive_contains_real_bytes() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let source = dir.path().join("source");
        fs::create_dir(&source)?;
        fs::write(source.join("data"), b"real bytes")?;
        let target = dir.path().join("artifact.zip");
        archive(&source, &target)?;
        let mut zip = zip::ZipArchive::new(fs::File::open(target)?)?;
        let mut content = Vec::new();
        std::io::Read::read_to_end(&mut zip.by_name("data")?, &mut content)?;
        assert_eq!(content, b"real bytes");
        Ok(())
    }
}
