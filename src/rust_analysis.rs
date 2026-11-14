//! Compiler dep-info plus explicit coverage of runtime repository inputs.
use crate::{git, model::*};
use anyhow::{Context, Result, bail};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

fn dep_info(text: &str) -> Result<(Vec<String>, Vec<String>)> {
    let joined = text.replace("\\\n", " ");
    let rule = joined
        .lines()
        .find(|line| !line.starts_with('#') && line.contains(": "))
        .context("missing dep-info rule")?;
    let rhs = rule.split_once(": ").unwrap().1;
    let mut words = Vec::new();
    let mut word = String::new();
    let mut escaped = false;
    for c in rhs.chars() {
        if escaped {
            word.push(c);
            escaped = false;
        } else if c == '\\' {
            escaped = true;
        } else if c.is_whitespace() {
            if !word.is_empty() {
                words.push(std::mem::take(&mut word));
            }
        } else {
            word.push(c);
        }
    }
    if escaped {
        bail!("trailing dep-info escape")
    }
    if !word.is_empty() {
        words.push(word)
    }
    if words.is_empty() {
        bail!("empty dep-info rule")
    }
    let bins = joined
        .lines()
        .filter_map(|l| {
            l.strip_prefix("# env-dep:CARGO_BIN_EXE_")
                .and_then(|v| v.split_once('=').map(|(name, _)| name.to_owned()))
        })
        .collect();
    Ok((words, bins))
}
fn dep_paths(artifact: &Value) -> Result<Vec<PathBuf>> {
    let mut paths = BTreeSet::new();
    for v in artifact["filenames"]
        .as_array()
        .context("missing filenames")?
    {
        let p = Path::new(v.as_str().context("invalid filename")?);
        if p.parent()
            .and_then(Path::file_name)
            .and_then(|s| s.to_str())
            == Some("deps")
        {
            let direct = p.with_extension("d");
            if direct.is_file() {
                paths.insert(direct);
            }
        } else if artifact["executable"].as_str() == p.to_str() {
            // Cargo exposes the un-hashed executable; locate its current compiler
            // output by content, then union all matching dep-info variants.
            let expected = git::hash(&fs::read(p)?);
            let prefix = format!(
                "{}-",
                artifact["target"]["name"]
                    .as_str()
                    .context("missing target name")?
                    .replace('-', "_")
            );
            let dir = p
                .parent()
                .context("missing executable parent")?
                .join("deps");
            for entry in fs::read_dir(dir)? {
                let d = entry?.path();
                if d.extension().and_then(|s| s.to_str()) != Some("d")
                    || !d
                        .file_name()
                        .unwrap()
                        .to_string_lossy()
                        .starts_with(&prefix)
                {
                    continue;
                }
                let exe = d.with_extension("");
                if exe.is_file()
                    && fs::metadata(&exe)?.len() == fs::metadata(p)?.len()
                    && git::hash(&fs::read(exe)?) == expected
                {
                    paths.insert(d);
                }
            }
        }
        if let Some(stem) = p
            .file_stem()
            .and_then(|s| s.to_str())
            .and_then(|s| s.strip_prefix("lib"))
        {
            let candidate = p.with_file_name(format!("{stem}.d"));
            if candidate.is_file() {
                paths.insert(candidate);
            }
        }
    }
    if paths.is_empty() {
        bail!("missing/ambiguous compiler dep-info")
    }
    Ok(paths.into_iter().collect())
}
pub fn graph(
    root: &Path,
    files: &BTreeMap<String, String>,
    cfg: &RustAnalysisConfig,
    operation: &Path,
    tests: &[Test],
) -> Result<Graph> {
    let artifacts: Vec<Value> =
        serde_json::from_slice(&fs::read(operation.join("cargo-artifacts.json"))?)?;
    let mut graph = Graph::default();
    let mut libraries = BTreeSet::new();
    let mut binaries = BTreeMap::new();
    let mut bin_refs = BTreeMap::<String, BTreeSet<String>>::new();
    for a in &artifacts {
        let kind = a["target"]["kind"]
            .as_array()
            .context("missing Cargo target kind")?;
        if kind.iter().any(|k| k == "custom-build") {
            graph.uncertainties.push("rust_build_script".into());
            continue;
        }
        let src = Path::new(
            a["target"]["src_path"]
                .as_str()
                .context("missing Cargo target source")?,
        )
        .canonicalize()?;
        let src = src
            .strip_prefix(root)?
            .to_str()
            .context("non UTF8 target")?
            .to_owned();
        git::safe_path(&src)?;
        if kind
            .iter()
            .any(|k| k == "lib" || k == "rlib" || k == "proc-macro")
        {
            libraries.insert(src.clone());
        }
        if kind.iter().any(|k| k == "bin") {
            binaries.insert(
                a["target"]["name"]
                    .as_str()
                    .context("missing binary name")?
                    .to_owned(),
                src.clone(),
            );
        }
        let parsed = (|| -> Result<_> {
            let mut deps = Vec::new();
            let mut bins = Vec::new();
            for p in dep_paths(a)? {
                let (d, b) = dep_info(&fs::read_to_string(p)?)?;
                deps.extend(d);
                bins.extend(b);
            }
            Ok((deps, bins))
        })();
        match parsed {
            Err(_) => graph
                .uncertainties
                .push(format!("rust_dep_info_unavailable:{src}")),
            Ok((dependencies, bins)) => {
                let deps = graph.dependencies.entry(src.clone()).or_default();
                for input in dependencies {
                    let path = Path::new(&input);
                    let path = if path.is_absolute() {
                        path.to_path_buf()
                    } else {
                        root.join(path)
                    };
                    let relative = path.canonicalize().ok().and_then(|p| {
                        p.strip_prefix(root)
                            .ok()
                            .map(|p| p.to_string_lossy().into_owned())
                    });
                    if let Some(path) = relative.filter(|p| files.contains_key(p)) {
                        if path != src {
                            deps.insert(path);
                        }
                    } else {
                        graph
                            .uncertainties
                            .push(format!("rust_unindexed_compiler_input:{src}"));
                    }
                }
                bin_refs.entry(src.clone()).or_default().extend(bins);
            }
        }
    }
    let roots: Vec<_> = graph.dependencies.keys().cloned().collect();
    for src in &roots {
        if !libraries.contains(src) {
            graph
                .dependencies
                .get_mut(src)
                .unwrap()
                .extend(libraries.iter().cloned());
        }
    }
    for (src, bins) in bin_refs {
        for bin in bins {
            if let Some(file) = binaries.get(&bin) {
                graph
                    .dependencies
                    .entry(src.clone())
                    .or_default()
                    .insert(file.clone());
            } else {
                graph
                    .uncertainties
                    .push(format!("rust_binary_dependency_unresolved:{src}:{bin}"));
            }
        }
    }
    for target in cfg.runtime_inputs.keys() {
        if !roots.contains(target) {
            bail!("runtime_inputs names unknown Cargo target: {target}")
        }
    }
    for target in tests.iter().map(|t| &t.file).collect::<BTreeSet<_>>() {
        if let Some(inputs) = cfg.runtime_inputs.get(target) {
            let deps = graph.dependencies.entry(target.clone()).or_default();
            for path in inputs {
                git::safe_path(path)?;
                if path.ends_with('/') {
                    deps.extend(files.keys().filter(|f| f.starts_with(path)).cloned());
                } else {
                    deps.insert(path.clone());
                }
            }
        } else {
            graph
                .uncertainties
                .push(format!("rust_runtime_inputs_unreviewed:{target}"));
        }
    }
    for path in files.keys() {
        if path == ".cargo/config" || path == ".cargo/config.toml" {
            graph
                .uncertainties
                .push(format!("rust_cargo_configuration:{path}"));
        }
    }
    graph.uncertainties.sort();
    graph.uncertainties.dedup();
    // Persist only paths/edges and reasons, never dep-info environment values.
    fs::write(
        operation.join("cargo-dependencies.json"),
        serde_json::to_vec_pretty(&graph)?,
    )?;
    Ok(graph)
}
#[cfg(test)]
mod tests {
    use super::dep_info;
    #[test]
    fn dep_info_handles_escaped_paths_continuations_and_binary_env() {
        let (paths,bins)=dep_info("out: src/lib.rs data/a\\ b.txt \\\n tests/case.rs\n\n# env-dep:CARGO_BIN_EXE_tool=/private/tool\n").unwrap();
        assert_eq!(paths, vec!["src/lib.rs", "data/a b.txt", "tests/case.rs"]);
        assert_eq!(bins, vec!["tool"]);
        assert!(dep_info("out:\n").is_err());
        assert!(dep_info("out: foo\\").is_err());
    }
}
