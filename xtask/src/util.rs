use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::Path,
    process::{Command, Output},
};

pub fn run(command: &mut Command) -> Result<()> {
    let status = command
        .status()
        .with_context(|| format!("starting {:?}", command.get_program()))?;
    if !status.success() {
        bail!("command {:?} failed: {status}", command.get_program());
    }
    Ok(())
}

pub fn capture(command: &mut Command) -> Result<Output> {
    let output = command
        .output()
        .with_context(|| format!("starting {:?}", command.get_program()))?;
    if !output.status.success() {
        bail!(
            "command {:?} failed: {}",
            command.get_program(),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(output)
}

pub fn hash(path: &Path) -> Result<String> {
    let mut file = File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = [0; 65536];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        digest.update(&buffer[..n]);
    }
    Ok(format!("sha256:{:x}", digest.finalize()))
}

pub fn copy(source: &Path, dest: &Path) -> Result<()> {
    let meta = fs::symlink_metadata(source)?;
    if !meta.is_file() || meta.file_type().is_symlink() {
        bail!("artifact is not a regular file");
    }
    let mut input = File::open(source)?;
    let mut output = OpenOptions::new().create_new(true).write(true).open(dest)?;
    std::io::copy(&mut input, &mut output)?;
    output.flush()?;
    if hash(source)? != hash(dest)? {
        bail!("artifact copy hash mismatch");
    }
    Ok(())
}

pub fn new_output(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    fs::create_dir(path).context("output must be a new directory")
}

pub fn env_file(name: &str, line: &str) -> Result<()> {
    if line.contains(['\n', '\r']) {
        bail!("invalid environment value");
    }
    println!("{line}");
    if let Some(path) = std::env::var_os(name) {
        writeln!(
            OpenOptions::new().create(true).append(true).open(path)?,
            "{line}"
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn output_and_copy_never_overwrite() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let output = dir.path().join("output");
        new_output(&output)?;
        fs::write(output.join("existing"), b"original")?;
        assert!(new_output(&output).is_err());
        let source = dir.path().join("source");
        fs::write(&source, b"replacement")?;
        assert!(copy(&source, &output.join("existing")).is_err());
        assert_eq!(fs::read(output.join("existing"))?, b"original");
        assert_eq!(fs::read(source)?, b"replacement");
        Ok(())
    }
}
