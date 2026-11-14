//! Canonical schema-2 sources: one document or one definition per category file.
use crate::model::*;
use anyhow::{Context, Result, bail};
use serde::{Deserialize, de::DeserializeOwned};
use std::{
    fs,
    path::{Path, PathBuf},
};

const CATEGORIES: [&str; 4] = ["requirements", "contracts", "properties", "relations"];

pub fn has_split(root: &Path) -> bool {
    CATEGORIES
        .iter()
        .any(|name| fs::symlink_metadata(root.join(name)).is_ok())
}
pub fn default_check_path() -> PathBuf {
    let root = Path::new("concord");
    if has_split(root) {
        root.to_path_buf()
    } else {
        root.join("contracts.yaml")
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Fragment<T> {
    schema_version: u32,
    definition: T,
}
fn fragments<T: DeserializeOwned>(root: &Path, category: &str) -> Result<Vec<T>> {
    let dir = root.join(category);
    let metadata = match fs::symlink_metadata(&dir) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e).with_context(|| format!("read {}", dir.display())),
    };
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        bail!(
            "definition category must be a directory, not a symlink: {}",
            dir.display()
        )
    }
    let mut paths = fs::read_dir(&dir)?
        .map(|entry| entry.map(|e| e.path()))
        .collect::<std::io::Result<Vec<_>>>()?;
    paths.sort();
    let mut values = Vec::new();
    for path in paths {
        let metadata = fs::symlink_metadata(&path)?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            bail!(
                "nested directories and symlinks are not supported: {}",
                path.display()
            )
        }
        if !matches!(
            path.extension().and_then(|e| e.to_str()),
            Some("yaml" | "yml")
        ) {
            bail!("expected .yaml or .yml definition: {}", path.display())
        }
        let item: Fragment<T> = serde_saphyr::from_str(&fs::read_to_string(&path)?)
            .with_context(|| format!("parse {}", path.display()))?;
        if item.schema_version != 2 {
            bail!("unsupported schema_version in {}", path.display())
        }
        values.push(item.definition);
    }
    Ok(values)
}
pub fn read(path: &Path) -> Result<DefinitionDocument> {
    let metadata =
        fs::symlink_metadata(path).with_context(|| format!("read {}", path.display()))?;
    if metadata.file_type().is_symlink() {
        bail!(
            "definition source must not be a symlink: {}",
            path.display()
        )
    }
    if !metadata.is_dir() {
        return serde_saphyr::from_str(&fs::read_to_string(path)?)
            .with_context(|| format!("parse {}", path.display()));
    }
    if path.join("contracts.yaml").try_exists()? || path.join("guarantees.yaml").try_exists()? {
        bail!("do not mix split definitions with contracts.yaml or guarantees.yaml")
    }
    let document = DefinitionDocument {
        schema_version: 2,
        requirements: fragments(path, "requirements")?,
        contracts: fragments(path, "contracts")?,
        properties: fragments(path, "properties")?,
        relations: fragments(path, "relations")?,
    };
    if document.requirements.is_empty()
        && document.contracts.is_empty()
        && document.properties.is_empty()
        && document.relations.is_empty()
    {
        bail!("no YAML definitions in {}", path.display())
    }
    Ok(document)
}
