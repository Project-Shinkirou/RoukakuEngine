use crate::util::{hash, run as command};
use anyhow::{bail, Context, Result};
use std::{
    fs,
    io::{Read, Write},
    path::{Component, Path},
    process::Command,
};

pub fn run(root: &Path) -> Result<()> {
    let lock: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("Tools/ui-toolchain-lock.json"))?)?;
    let target = format!(
        "{}-{}",
        match std::env::consts::OS {
            "windows" => "windows",
            "linux" => "linux",
            "macos" => "macos",
            _ => bail!("unsupported Luau host"),
        },
        std::env::consts::ARCH
    );
    let config = &lock["luau"]["targets"][&target];
    let field = |name: &str| {
        config[name]
            .as_str()
            .with_context(|| format!("Luau lock has no {name} for {target}"))
    };
    let url = field("url")?;
    let expected = field("archive_sha256")?;
    let analyzer_hash = field("analyzer_sha256")?;
    let member = field("analyzer_path")?;
    safe_member(member)?;
    let work = root.join(".work/ui-toolchain");
    fs::create_dir_all(&work)?;
    let analyzer = work
        .join(format!(
            "{}-{}",
            lock["luau"]["version"].as_str().context("Luau version")?,
            target
        ))
        .join(member);
    if !analyzer.is_file() || hash(&analyzer)? != analyzer_hash {
        let mut downloaded = tempfile::NamedTempFile::new_in(&work)?;
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?
            .block_on(async {
                let client = reqwest::Client::builder()
                    .https_only(true)
                    .timeout(std::time::Duration::from_secs(120))
                    .build()?;
                let mut response = client.get(url).send().await?.error_for_status()?;
                let mut total = 0usize;
                while let Some(bytes) = response.chunk().await? {
                    total += bytes.len();
                    if total > 128 * 1024 * 1024 {
                        bail!("Luau archive exceeds size limit");
                    }
                    downloaded.write_all(&bytes)?;
                }
                Ok::<_, anyhow::Error>(())
            })?;
        if hash(downloaded.path())? != expected {
            bail!("Luau archive hash mismatch");
        }
        let mut archive = zip::ZipArchive::new(fs::File::open(downloaded.path())?)?;
        for index in 0..archive.len() {
            let entry = archive.by_index(index)?;
            safe_member(entry.name())?;
            if entry
                .unix_mode()
                .is_some_and(|mode| mode & 0o170000 == 0o120000)
            {
                bail!("Luau archive contains a link");
            }
        }
        let mut entry = archive.by_name(member)?;
        if entry.size() > 128 * 1024 * 1024 {
            bail!("Luau analyzer exceeds size limit");
        }
        let parent = analyzer.parent().context("analyzer parent")?;
        fs::create_dir_all(parent)?;
        let mut extracted = tempfile::NamedTempFile::new_in(parent)?;
        std::io::copy(
            &mut entry.by_ref().take(128 * 1024 * 1024 + 1),
            &mut extracted,
        )?;
        if hash(extracted.path())? != analyzer_hash {
            bail!("Luau executable hash mismatch");
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(extracted.path(), fs::Permissions::from_mode(0o755))?;
        }
        extracted.persist(&analyzer)?;
    }
    let definitions = fs::read_to_string(root.join("Tools/LuauTypes/roukaku-ui.d.luau"))?;
    let mut controllers: Vec<_> = fs::read_dir(root.join("Examples/NativeVN/Controllers"))?
        .collect::<std::io::Result<Vec<_>>>()?
        .into_iter()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "luau"))
        .collect();
    controllers.sort();
    if controllers.is_empty() {
        bail!("NativeVN controller sources are missing");
    }
    let harness = tempfile::tempdir_in(&work)?;
    for source in controllers {
        let target = harness
            .path()
            .join(source.file_name().context("controller filename")?);
        fs::write(
            &target,
            format!("{definitions}\n{}", fs::read_to_string(source)?),
        )?;
        command(
            Command::new(&analyzer)
                .current_dir(root)
                .arg("--mode=strict")
                .arg(target),
        )?;
    }
    Ok(())
}

fn safe_member(name: &str) -> Result<()> {
    if name.contains(['\\', ':'])
        || Path::new(name)
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        bail!("unsafe Luau archive path");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_archive_escape_and_drive_paths() {
        for path in [
            "../analyzer",
            "/absolute",
            "C:/analyzer",
            "dir\\..\\analyzer",
        ] {
            assert!(safe_member(path).is_err());
        }
        assert!(safe_member("bin/analyzer").is_ok());
    }
}
