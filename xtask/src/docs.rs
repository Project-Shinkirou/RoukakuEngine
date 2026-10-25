use anyhow::{Result, bail};
use percent_encoding::percent_decode_str;
use regex::Regex;
use std::{
    fs,
    path::{Component, Path, PathBuf},
    sync::LazyLock,
};

static PRIVATE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(?:\b[a-z]:[\\/]|/(?:home|users)/[^/\s<>]+/)").unwrap());
static SCHEME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[A-Za-z][A-Za-z0-9+.-]*:").unwrap());
static LINKS: LazyLock<[Regex; 2]> = LazyLock::new(|| {
    [
        Regex::new(r#"!?\[[^\]\n]*\]\((<[^>]+>|[^\s)]+)(?:\s+["'][^\n]*?["'])?\)"#).unwrap(),
        Regex::new(r"(?m)^ {0,3}\[[^\]^]+\]:\s*(<[^>]+>|\S+)").unwrap(),
    ]
});

pub fn run(root: &Path) -> Result<()> {
    let mut files = Vec::new();
    if root.join("docs").exists() {
        for entry in walkdir::WalkDir::new(root.join("docs")) {
            let entry = entry?;
            if entry.file_type().is_file() && entry.path().extension().is_some_and(|x| x == "md") {
                files.push(entry.into_path());
            }
        }
    }
    files.extend(
        ["README.md", "CONTRIBUTING.md"]
            .iter()
            .map(|name| root.join(name))
            .filter(|p| p.is_file()),
    );
    files.sort();
    let errors: Vec<_> = files
        .iter()
        .flat_map(|path| check_file(path, root))
        .collect();
    for error in &errors {
        println!("{error}");
    }
    println!(
        "Checked {} documents; {} link/hygiene errors.",
        files.len(),
        errors.len()
    );
    if !errors.is_empty() {
        bail!("documentation checks failed");
    }
    Ok(())
}

fn prose(text: &str) -> String {
    let mut fence: Option<(u8, usize)> = None;
    let mut result = String::with_capacity(text.len());
    for line in text.lines() {
        let trimmed = line.trim_start_matches(' ');
        let indent = line.len() - trimmed.len();
        let marker = trimmed
            .as_bytes()
            .first()
            .copied()
            .filter(|c| matches!(c, b'`' | b'~'));
        if let Some(c) = marker {
            let n = trimmed.bytes().take_while(|b| *b == c).count();
            if indent <= 3 && n >= 3 {
                match fence {
                    None => fence = Some((c, n)),
                    Some((old, len)) if old == c && n >= len => fence = None,
                    _ => {}
                }
                result.push('\n');
                continue;
            }
        }
        if fence.is_none() {
            result.push_str(line);
        }
        result.push('\n');
    }
    result
}

pub fn check_file(path: &Path, root: &Path) -> Vec<String> {
    let label = path.strip_prefix(root).unwrap_or(path).to_string_lossy();
    let Ok(content) = fs::read_to_string(path) else {
        return vec![format!("{label}: cannot read UTF-8 document")];
    };
    let mut errors = Vec::new();
    let private_path = PRIVATE.is_match(&content);
    let canonical_root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    if private_path {
        errors.push(format!("{label}: contains a private absolute path"));
    }
    if content
        .chars()
        .any(|c| (c < ' ' && !matches!(c, '\n' | '\r' | '\t')) || c == '\u{7f}')
    {
        errors.push(format!("{label}: contains a control character"));
    }
    let text = prose(&content);
    for pattern in LINKS.iter() {
        for link in pattern.captures_iter(&text) {
            let target = link[1].trim_matches(['<', '>']);
            let local = target.split(['#', '?']).next().unwrap_or("");
            if local.is_empty() || target.starts_with("//") || SCHEME.is_match(target) {
                continue;
            }
            let Ok(decoded) = percent_decode_str(local).decode_utf8() else {
                errors.push(format!("{label}: malformed link"));
                continue;
            };
            if decoded.chars().any(|c| c.is_control()) {
                errors.push(format!("{label}: malformed link"));
                continue;
            }
            let mut normalized = PathBuf::new();
            for component in path.parent().unwrap().join(decoded.as_ref()).components() {
                match component {
                    Component::ParentDir => {
                        normalized.pop();
                    }
                    Component::CurDir => {}
                    c => normalized.push(c.as_os_str()),
                }
            }
            let escaped = !normalized.starts_with(root)
                || normalized
                    .canonicalize()
                    .is_ok_and(|p| !p.starts_with(&canonical_root));
            if escaped {
                errors.push(format!("{label}: local link escapes repository"));
            } else if !normalized.exists() {
                // A document with a private path must never expose it through another diagnostic.
                if private_path {
                    errors.push(format!("{label}: missing local link"));
                } else {
                    errors.push(format!("{label}: missing link {target}"));
                }
            }
        }
    }
    errors
}

#[cfg(test)]
mod tests {
    use super::*;
    fn check(text: &str) -> Vec<String> {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("existing file.md"), "# Target").unwrap();
        let source = dir.path().join("source.md");
        fs::write(&source, text).unwrap();
        check_file(&source, dir.path())
    }
    #[test]
    fn links_resolve_real_files_and_reference_targets() {
        assert!(
            check("[file](<existing file.md> \"title\") [encoded](existing%20file.md#anchor)")
                .is_empty()
        );
        let errors = check("[a](missing.md)\n[ref]: absent.md");
        assert_eq!(errors.len(), 2);
    }
    #[test]
    fn examples_and_remote_links_do_not_access_files() {
        assert!(check("```md\n[a](missing)\n```\n~~~\n[b](missing)\n~~~\n[a](https://example.org/a) [b](#heading) [c](mailto:a@example.org)").is_empty());
    }
    #[test]
    fn rejects_repository_escape_and_controls() {
        assert_eq!(check("[a](../outside)\u{7}").len(), 2);
        assert!(!check("[a](existing%00file.md)").is_empty());
    }
    #[test]
    fn private_paths_are_redacted_even_inside_links_and_fences() {
        let value = "/home/private-person/source";
        let errors = check(&format!("[a]({value})\n```\n{value}\n```"));
        assert!(!errors.is_empty());
        assert!(errors.iter().all(|e| !e.contains(value)));
    }
}
