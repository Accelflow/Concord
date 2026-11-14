//! Stable Cargo/libtest adapter. No nightly JSON protocol or arbitrary Cargo flags.
use crate::{
    git,
    model::*,
    runner::{self, ProcessResult},
};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

#[derive(Serialize, Deserialize)]
struct Target {
    file: String,
    executable: String,
    sha256: String,
}
fn command(
    root: &Path,
    cfg: &RunnerConfig,
    args: &[&str],
    dir: &Path,
    label: &str,
) -> Result<ProcessResult> {
    runner::execute(
        root,
        cfg,
        &args.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>(),
        dir,
        label,
    )
}
pub fn version(root: &Path, cfg: &RunnerConfig, dir: &Path) -> Result<String> {
    if cfg.version != "1.97.1" || !cfg.args.is_empty() {
        bail!("Cargo adapter requires version 1.97.1 and empty runner.args")
    }
    let p = command(root, cfg, &["--version"], dir, "version")?;
    let text = fs::read_to_string(p.stdout)?;
    if p.stopped || p.exit != Some(0) || !text.starts_with("cargo 1.97.1 ") {
        bail!("Cargo version mismatch or interrupted")
    }
    // Pin the compiler as well as Cargo; the text harness protocol is version-bound.
    let mut rustc = cfg.clone();
    rustc.program = "rustc".into();
    let p = command(root, &rustc, &["--version"], dir, "rustc-version")?;
    let compiler = fs::read_to_string(p.stdout)?;
    if p.stopped || p.exit != Some(0) || !compiler.starts_with("rustc 1.97.1 ") {
        bail!("rustc version mismatch or interrupted")
    }
    Ok(format!("{};{}", text.trim(), compiler.trim()))
}
pub fn collect(root: &Path, cfg: &RunnerConfig, dir: &Path) -> Result<Vec<Test>> {
    let p = command(
        root,
        cfg,
        &["metadata", "--locked", "--no-deps", "--format-version=1"],
        dir,
        "cargo-metadata",
    )?;
    if p.stopped || p.exit != Some(0) {
        bail!("Cargo metadata failed or interrupted")
    }
    let metadata: Value = serde_json::from_slice(&fs::read(p.stdout)?)?;
    let packages = metadata["packages"]
        .as_array()
        .context("missing Cargo packages")?;
    if packages.len() != 1
        || metadata["workspace_members"].as_array().map(Vec::len) != Some(1)
        || Path::new(
            packages[0]["manifest_path"]
                .as_str()
                .context("missing manifest")?,
        )
        .canonicalize()?
            != root.join("Cargo.toml").canonicalize()?
    {
        bail!(
            "Cargo adapter requires a single package rooted at this repository; multi-package workspaces are unsupported"
        )
    }
    if packages[0]["dependencies"]
        .as_array()
        .is_some_and(|deps| deps.iter().any(|dep| dep["source"].is_null()))
    {
        bail!(
            "path dependencies are unsupported: their source is outside the single-package input fingerprint"
        )
    }
    let manifest: toml::Value = toml::from_str(&fs::read_to_string(root.join("Cargo.toml"))?)?;
    for kind in ["lib", "bin", "test"] {
        if manifest
            .get(kind)
            .is_some_and(|v| v.get("harness").and_then(toml::Value::as_bool) == Some(false))
            || manifest
                .get(kind)
                .and_then(toml::Value::as_array)
                .is_some_and(|items| {
                    items
                        .iter()
                        .any(|v| v.get("harness").and_then(toml::Value::as_bool) == Some(false))
                })
        {
            bail!("custom harness=false targets are unsupported")
        }
    }
    let p = command(
        root,
        cfg,
        &[
            "test",
            "--locked",
            "--tests",
            "--no-run",
            "--message-format=json",
        ],
        dir,
        "cargo-build",
    )?;
    if p.stopped || p.exit != Some(0) {
        bail!("Cargo test compilation failed or interrupted; inspect operation logs")
    }
    let mut targets = Vec::new();
    let mut artifacts = Vec::new();
    let mut finished = false;
    for line in fs::read_to_string(p.stdout)?
        .lines()
        .filter(|s| !s.trim().is_empty())
    {
        let v: Value = serde_json::from_str(line).context("invalid Cargo build JSON")?;
        if v["reason"] == "build-finished" {
            finished = v["success"] == true;
        }
        if v["reason"] == "compiler-artifact" && v["package_id"] == packages[0]["id"] {
            artifacts.push(v.clone());
        }
        if v["reason"] == "compiler-artifact"
            && v["profile"]["test"] == true
            && v["package_id"] == packages[0]["id"]
            && let Some(executable) = v["executable"].as_str()
        {
            let file = runner::relative(
                root,
                v["target"]["src_path"]
                    .as_str()
                    .context("missing target source")?,
            )?;
            targets.push(Target {
                file,
                executable: executable.into(),
                sha256: git::hash(&fs::read(executable)?),
            });
        }
    }
    if !finished {
        bail!("missing successful Cargo build-finished record")
    }
    fs::write(
        dir.join("cargo-artifacts.json"),
        serde_json::to_vec_pretty(&artifacts)?,
    )?;
    targets.sort_by(|a, b| a.file.cmp(&b.file));
    let mut tests = Vec::new();
    let mut ids = BTreeSet::new();
    for (i, target) in targets.iter().enumerate() {
        let mut harness = cfg.clone();
        harness.program = target.executable.clone();
        let p = command(
            root,
            &harness,
            &["--list", "--format", "terse"],
            dir,
            &format!("cargo-list-{i}"),
        )?;
        if p.stopped || p.exit != Some(0) {
            bail!("Cargo harness collection failed or interrupted")
        }
        for line in fs::read_to_string(p.stdout)?
            .lines()
            .filter(|s| !s.is_empty())
        {
            let name = line.strip_suffix(": test").context(
                "unsupported libtest list output (benchmarks/custom harness are unsupported)",
            )?;
            if name.is_empty() {
                bail!("empty Cargo test name")
            }
            let id = git::hash(&serde_json::to_vec(&(&target.file, name))?);
            if !ids.insert(id.clone()) {
                bail!("ambiguous duplicate Cargo test identity")
            }
            tests.push(Test {
                id,
                file: target.file.clone(),
                name: name.into(),
            });
        }
    }
    if tests.is_empty() {
        bail!("Cargo collection returned zero cases")
    }
    fs::write(
        dir.join("cargo-targets.json"),
        serde_json::to_vec_pretty(&targets)?,
    )?;
    tests.sort_by(|a, b| (&a.file, &a.name).cmp(&(&b.file, &b.name)));
    Ok(tests)
}

/// Reject incomplete/ambiguous harness output instead of inferring pass from exit alone.
fn parse(text: &str, file: &str) -> Result<(Value, usize, usize, usize)> {
    let mut cases = Vec::new();
    let mut summary = None;
    for line in text.lines() {
        if !line.starts_with("test result: ")
            && let Some(case) = line.strip_prefix("test ")
        {
            let (name, status) = case
                .rsplit_once(" ... ")
                .context("invalid libtest case line")?;
            let status = match status {
                "ok" => "passed",
                "FAILED" => "failed",
                s if s == "ignored" || s.starts_with("ignored, ") => "skipped",
                _ => bail!("unknown libtest status"),
            };
            cases.push(json!({"ancestorTitles":[],"title":name,"status":status}));
        }
        if line.starts_with("test result: ") {
            if summary.is_some() {
                bail!("duplicate libtest summary")
            }
            let fields: Vec<_> = line.split(';').collect();
            if fields.len() != 6
                || !(line.starts_with("test result: ok. ")
                    || line.starts_with("test result: FAILED. "))
            {
                bail!("invalid libtest summary")
            }
            let first = fields[0]
                .rsplit_once(". ")
                .context("invalid summary prefix")?
                .1;
            let count = |s: &str, label: &str| -> Result<usize> {
                Ok(s.trim()
                    .strip_suffix(label)
                    .context("invalid summary count")?
                    .parse()?)
            };
            let passed = count(first, " passed")?;
            let failed = count(fields[1], " failed")?;
            let skipped = count(fields[2], " ignored")?;
            if count(fields[3], " measured")? != 0
                || count(fields[4], " filtered out")? != 0
                || !fields[5].trim().starts_with("finished in ")
            {
                bail!("filtered/benchmark/incomplete Cargo execution")
            }
            if line.starts_with("test result: ok.") != (failed == 0) {
                bail!("libtest summary status mismatch")
            }
            summary = Some((passed, failed, skipped));
        }
    }
    let (passed, failed, skipped) = summary.context("missing libtest summary")?;
    for (status, expected) in [("passed", passed), ("failed", failed), ("skipped", skipped)] {
        if cases.iter().filter(|c| c["status"] == status).count() != expected {
            bail!("libtest case/summary count mismatch")
        }
    }
    Ok((
        json!({"name":file,"assertionResults":cases}),
        passed,
        failed,
        skipped,
    ))
}
pub fn run(
    root: &Path,
    cfg: &RunnerConfig,
    selected: &BTreeSet<String>,
    operation: &Path,
    dir: &Path,
    report_path: &Path,
) -> Result<ProcessResult> {
    let targets: Vec<Target> =
        serde_json::from_slice(&fs::read(operation.join("cargo-targets.json"))?)?;
    let mut matched = BTreeSet::new();
    let mut suites = Vec::new();
    let mut native = Vec::new();
    let mut total = 0;
    let mut failures = 0;
    let mut last_stdout = PathBuf::new();
    for (i, target) in targets
        .iter()
        .enumerate()
        .filter(|(_, t)| selected.contains(&t.file))
    {
        matched.insert(target.file.clone());
        if git::hash(&fs::read(&target.executable)?) != target.sha256 {
            bail!("Cargo test artifact changed since collection")
        }
        let mut harness = cfg.clone();
        harness.program = target.executable.clone();
        let p = command(
            root,
            &harness,
            &["--format", "pretty", "--color", "never", "--test-threads=1"],
            dir,
            &format!("cargo-run-{i}"),
        )?;
        if p.stopped {
            return Ok(p);
        }
        let (suite, passed, failed, skipped) =
            parse(&fs::read_to_string(&p.stdout)?, &target.file)?;
        if p.exit != Some(if failed > 0 { 101 } else { 0 }) {
            bail!("libtest exit/result mismatch")
        }
        total += passed + failed + skipped;
        failures += failed;
        suites.push(suite);
        native.push(json!({"file":target.file,"exit_code":p.exit,"passed":passed,"failed":failed,"skipped":skipped}));
        last_stdout = p.stdout;
    }
    if matched != *selected {
        bail!("selected Cargo target missing from compiled artifacts")
    }
    fs::write(
        report_path,
        serde_json::to_vec_pretty(
            &json!({"success":failures==0,"numTotalTests":total,"testResults":suites,"cargo_targets":native}),
        )?,
    )?;
    Ok(ProcessResult {
        exit: Some(if failures > 0 { 1 } else { 0 }),
        stopped: false,
        stdout: last_stdout,
    })
}

#[cfg(test)]
mod tests {
    use super::parse;
    #[test]
    fn rejects_missing_duplicate_mismatched_filtered_and_unknown_harness_results() {
        let ok = "test one ... ok\n\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s\n";
        assert!(parse(ok, "tests/a.rs").is_ok());
        for bad in [
            "test one ... ok\n".to_owned(),
            format!("{ok}{ok}"),
            ok.replace("1 passed", "2 passed"),
            ok.replace("0 filtered out", "1 filtered out"),
            ok.replace(" ... ok", " ... mystery"),
            ok.replace("0 measured", "1 measured"),
        ] {
            assert!(parse(&bad, "tests/a.rs").is_err(), "{bad}");
        }
    }
}
