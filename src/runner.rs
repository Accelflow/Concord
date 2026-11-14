use crate::{
    git,
    model::{CaseResult, RunnerConfig, RunnerKind, Test},
};
use anyhow::{Context, Result, bail};
use serde_json::Value;
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::{
    collections::BTreeSet,
    fs::{self, File},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
pub static INTERRUPTED: AtomicBool = AtomicBool::new(false);
pub fn now() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}
pub fn id() -> String {
    format!("{}-{}", now(), std::process::id())
}
pub struct ProcessResult {
    pub exit: Option<i32>,
    pub stopped: bool,
    pub stdout: PathBuf,
}
pub fn execute(
    root: &Path,
    cfg: &RunnerConfig,
    args: &[String],
    dir: &Path,
    label: &str,
) -> Result<ProcessResult> {
    execute_with_report(root, cfg, args, dir, label, None)
}

pub fn execute_with_report(
    root: &Path,
    cfg: &RunnerConfig,
    args: &[String],
    dir: &Path,
    label: &str,
    report: Option<&Path>,
) -> Result<ProcessResult> {
    fs::create_dir_all(dir)?;
    let stdout = dir.join(format!("{label}.stdout.log"));
    let stderr = dir.join(format!("{label}.stderr.log"));
    let mut cmd = Command::new(&cfg.program);
    cmd.current_dir(root)
        .args(&cfg.args)
        .args(args)
        .env("CI", "1")
        .env("NO_COLOR", "1")
        .stdin(Stdio::null())
        .stdout(File::create(&stdout)?)
        .stderr(File::create(stderr)?);
    cmd.env_remove("CONCORD_COMMAND_REPORT");
    if let Some(path) = report {
        cmd.env("CONCORD_COMMAND_REPORT", path);
    }
    #[cfg(unix)]
    cmd.process_group(0);
    let mut child = cmd.spawn().context("start configured test runner")?;
    let start = Instant::now();
    loop {
        if INTERRUPTED.load(Ordering::SeqCst)
            || start.elapsed() > Duration::from_secs(cfg.timeout_seconds)
        {
            #[cfg(unix)]
            unsafe {
                libc::kill(-(child.id() as i32), libc::SIGKILL);
            }
            let _ = child.kill();
            let _ = child.wait();
            return Ok(ProcessResult {
                exit: None,
                stopped: true,
                stdout,
            });
        }
        if let Some(status) = child.try_wait()? {
            return Ok(ProcessResult {
                exit: status.code(),
                stopped: false,
                stdout,
            });
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}
pub fn version(root: &Path, cfg: &RunnerConfig, dir: &Path) -> Result<String> {
    if cfg.timeout_seconds == 0 || cfg.timeout_seconds > 3600 {
        bail!("timeout_seconds must be 1..3600")
    }
    if cfg.kind == RunnerKind::Commands {
        return Ok("commands-1".into());
    }
    if cfg.kind == RunnerKind::Cargo {
        return crate::cargo_runner::version(root, cfg, dir);
    }
    // Fixed adapter ABI; unknown versions require independent acceptance.
    if cfg.version != "4.1.12" {
        bail!(
            "unsupported Vitest version {}; supported: 4.1.12",
            cfg.version
        )
    }
    if cfg.args.iter().any(|a| a.starts_with('-')) {
        bail!("runner.args must contain launcher/script arguments only, no Vitest options")
    }
    let p = execute(root, cfg, &["--version".into()], dir, "version")?;
    if p.stopped {
        bail!("runner version collection interrupted/timeout")
    }
    let text = fs::read_to_string(p.stdout)?;
    let first = text.split_whitespace().next().unwrap_or("");
    if p.exit != Some(0) || first != format!("vitest/{}", cfg.version) {
        bail!("runner version mismatch")
    }
    Ok(first.to_owned())
}
pub fn relative(root: &Path, path: &str) -> Result<String> {
    let p = Path::new(path);
    let p = if p.is_absolute() {
        p.to_path_buf()
    } else {
        root.join(p)
    };
    let name = p
        .strip_prefix(root)
        .context("runner returned a path outside checkout")?
        .to_str()
        .context("non UTF-8 path")?
        .to_owned();
    git::safe_path(&name)?;
    Ok(name)
}
pub fn collect(root: &Path, cfg: &RunnerConfig, dir: &Path) -> Result<Vec<Test>> {
    if cfg.kind == RunnerKind::Commands {
        return Ok(Vec::new());
    }
    if cfg.kind == RunnerKind::Cargo {
        return crate::cargo_runner::collect(root, cfg, dir);
    }
    let p = execute(root, cfg, &["list".into(), "--json".into()], dir, "collect")?;
    if p.stopped {
        bail!("test collection interrupted/timeout")
    }
    if p.exit != Some(0) {
        bail!("test collection failed; inspect local operation logs")
    }
    let values: Vec<Value> =
        serde_json::from_slice(&fs::read(p.stdout)?).context("invalid Vitest collection JSON")?;
    let mut tests = Vec::new();
    let mut ids = BTreeSet::new();
    for v in values {
        let file = relative(root, v["file"].as_str().context("missing test file")?)?;
        let name = v["name"].as_str().context("missing test name")?.to_owned();
        let id = git::hash(&serde_json::to_vec(&(&file, &name))?);
        if !ids.insert(id.clone()) {
            bail!("ambiguous duplicate test identity: {file}:{name}")
        }
        tests.push(Test { id, file, name });
    }
    if tests.is_empty() {
        bail!("test collection returned zero cases")
    }
    tests.sort_by(|a, b| (&a.file, &a.name).cmp(&(&b.file, &b.name)));
    Ok(tests)
}
pub fn parse_results(root: &Path, path: &Path) -> Result<(Value, Vec<CaseResult>)> {
    let v: Value =
        serde_json::from_slice(&fs::read(path)?).context("invalid/missing runner report")?;
    let mut cases = Vec::new();
    for suite in v["testResults"].as_array().context("missing testResults")? {
        let file = relative(root, suite["name"].as_str().context("missing result file")?)?;
        for case in suite["assertionResults"]
            .as_array()
            .context("missing assertions")?
        {
            let status = case["status"]
                .as_str()
                .context("missing status")?
                .to_owned();
            if !["passed", "failed", "pending", "todo", "skipped", "disabled"]
                .contains(&status.as_str())
            {
                bail!("unknown runner status {status}")
            }
            cases.push(CaseResult {
                file: file.clone(),
                name: {
                    let mut names = case["ancestorTitles"]
                        .as_array()
                        .context("missing ancestorTitles")?
                        .iter()
                        .map(|v| v.as_str().context("invalid ancestor title"))
                        .collect::<Result<Vec<_>>>()?;
                    names.push(case["title"].as_str().context("missing case title")?);
                    names.join(" > ")
                },
                status,
                test_id: None,
            });
        }
    }
    Ok((v, cases))
}
