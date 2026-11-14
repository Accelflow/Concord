use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Component, Path, PathBuf},
    process::Command,
};

pub fn hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
pub fn output(root: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let o = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .context("start git")?;
    if !o.status.success() {
        bail!(
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&o.stderr)
        );
    }
    Ok(o.stdout)
}
pub fn text(root: &Path, args: &[&str]) -> Result<String> {
    Ok(String::from_utf8(output(root, args)?)?.trim().to_owned())
}
pub fn root() -> Result<PathBuf> {
    Ok(PathBuf::from(text(Path::new("."), &["rev-parse", "--show-toplevel"])?).canonicalize()?)
}
pub fn safe_path(path: &str) -> Result<()> {
    let p = Path::new(path);
    if path.is_empty()
        || p.is_absolute()
        || p.components().any(|c| !matches!(c, Component::Normal(_)))
    {
        bail!("expected repository-relative path: {path}");
    }
    if path == ".concord"
        || path.starts_with(".concord/")
        || path == ".git"
        || path.starts_with(".git/")
    {
        bail!("reserved path: {path}");
    }
    Ok(())
}
pub fn files(root: &Path) -> Result<BTreeMap<String, String>> {
    let raw = output(
        root,
        &[
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
        ],
    )?;
    let mut files = BTreeMap::new();
    for name in raw.split(|b| *b == 0).filter(|x| !x.is_empty()) {
        let name = std::str::from_utf8(name)?.to_owned();
        if name == ".concord" || name.starts_with(".concord/") {
            continue;
        }
        safe_path(&name)?;
        let p = root.join(&name);
        if !p.try_exists()? {
            continue;
        }
        let canonical = p.canonicalize()?;
        if !canonical.starts_with(root) || fs::symlink_metadata(&p)?.file_type().is_symlink() {
            bail!("unsupported symlink: {name}");
        }
        if !p.is_file() {
            bail!("unsupported submodule/directory: {name}");
        }
        files.insert(name, hash(&fs::read(p)?));
    }
    // Vite can load ignored .env files. Include their hashes without exposing values.
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name == ".env" || name.starts_with(".env.") {
            if entry.file_type()?.is_symlink() || !entry.file_type()?.is_file() {
                bail!("unsupported runtime input: {name}");
            }
            files.insert(name, hash(&fs::read(entry.path())?));
        }
    }
    Ok(files)
}
pub fn changes(root: &Path, base: &str) -> Result<(String, String, String, BTreeSet<String>)> {
    let resolved = text(
        root,
        &["rev-parse", "--verify", &format!("{base}^{{commit}}")],
    )?;
    let head = text(root, &["rev-parse", "HEAD"])?;
    let merge = text(root, &["merge-base", &resolved, &head])?;
    // Disable rename folding so both old and new paths are retained.
    let raw = output(
        root,
        &["diff", "--name-only", "-z", "--no-renames", &merge, "--"],
    )?;
    let staged = output(
        root,
        &[
            "diff",
            "--cached",
            "--name-only",
            "-z",
            "--no-renames",
            "HEAD",
            "--",
        ],
    )?;
    let untracked = output(root, &["ls-files", "--others", "--exclude-standard", "-z"])?;
    let mut paths = BTreeSet::new();
    for bytes in [&raw, &staged, &untracked] {
        for p in bytes.split(|b| *b == 0).filter(|x| !x.is_empty()) {
            let p = std::str::from_utf8(p)?.to_owned();
            if !p.starts_with(".concord/") {
                safe_path(&p)?;
                paths.insert(p);
            }
        }
    }
    Ok((resolved, merge, head, paths))
}
pub fn fingerprint(root: &Path, files: &BTreeMap<String, String>, extra: &str) -> Result<String> {
    let status = output(
        root,
        &[
            "status",
            "--porcelain=v1",
            "-z",
            "--untracked-files=all",
            "--",
            ".",
            ":(exclude).concord",
        ],
    )?;
    let head = output(root, &["rev-parse", "HEAD"])?;
    Ok(hash(
        &[
            serde_json::to_vec(files)?,
            status,
            head,
            extra.as_bytes().to_vec(),
            env!("CARGO_PKG_VERSION").as_bytes().to_vec(),
        ]
        .concat(),
    ))
}
