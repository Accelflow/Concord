//! Explicit repository-owned verification commands. No implicit shell or test discovery.
use crate::{git, model::*, runner};
use anyhow::{Context, Result, bail};
use regex::Regex;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Read,
    path::Path,
    time::Instant,
};

pub fn validate(
    config: &Config,
    definitions: &DefinitionDocument,
    has_file: impl Fn(&str) -> bool,
) -> Result<()> {
    let mut ids = BTreeSet::new();
    for command in &config.commands {
        if command.id.trim().is_empty()
            || !ids.insert(&command.id)
            || command.program.trim().is_empty()
            || command.program.contains('\0')
            || command.args.iter().any(|a| a.contains('\0'))
            || !(1..=3600).contains(&command.timeout_seconds)
        {
            bail!("invalid/duplicate verification command: {}", command.id)
        }
        if let Some(report) = &command.report
            && (report.required_cases.is_empty()
                || report.required_cases.iter().any(|id| !valid_case_id(id))
                || report.required_cases.iter().collect::<BTreeSet<_>>().len()
                    != report.required_cases.len())
        {
            bail!(
                "invalid required report case IDs for command: {}",
                command.id
            );
        }
        for path in &command.files {
            git::safe_path(path).with_context(|| {
                format!("invalid input for verification command {}", command.id)
            })?;
            if !has_file(path) {
                bail!(
                    "verification command input is not an indexed file or is missing: {path} (command {})",
                    command.id
                )
            }
        }
    }
    InputPatterns::compile(config)?;
    for (kind, owner, obligation) in definitions
        .properties
        .iter()
        .flat_map(|p| p.obligations.iter().map(|o| ("Property", &p.id, o)))
        .chain(
            definitions
                .relations
                .iter()
                .flat_map(|r| r.obligations.iter().map(|o| ("Relation", &r.id, o))),
        )
    {
        for id in &obligation.commands {
            if !ids.contains(id) {
                bail!(
                    "unknown verification command: {id} ({kind} {owner}, obligation {})",
                    obligation.id
                )
            }
        }
    }
    if config.runner.kind == RunnerKind::Commands && config.commands.is_empty() {
        bail!("commands runner requires registered verification commands")
    }
    Ok(())
}

pub(crate) struct InputPatterns<'a>(BTreeMap<&'a str, Vec<(&'a str, Regex)>>);

impl<'a> InputPatterns<'a> {
    pub(crate) fn compile(config: &'a Config) -> Result<Self> {
        let mut patterns = BTreeMap::new();
        for command in &config.commands {
            let mut compiled = Vec::new();
            for pattern in &command.input_patterns {
                let matcher = compile_input_pattern(pattern).with_context(|| {
                    format!(
                        "invalid input pattern {pattern:?} for verification command {}",
                        command.id
                    )
                })?;
                compiled.push((pattern.as_str(), matcher));
            }
            patterns.insert(command.id.as_str(), compiled);
        }
        Ok(Self(patterns))
    }

    pub(crate) fn matching<'b>(
        &'b self,
        id: &str,
        path: &'b str,
    ) -> impl Iterator<Item = &'a str> + 'b {
        self.0
            .get(id)
            .into_iter()
            .flatten()
            .filter_map(move |(pattern, matcher)| matcher.is_match(path).then_some(*pattern))
    }
}

fn compile_input_pattern(pattern: &str) -> Result<Regex> {
    let directory = pattern.ends_with('/');
    let path = pattern.strip_suffix('/').unwrap_or(pattern);
    git::safe_path(path)?;
    let parts: Vec<_> = path.split('/').collect();
    if parts
        .iter()
        .any(|part| part.is_empty() || *part == "." || *part == "..")
        || path.contains(['\\', '[', ']', '{', '}'])
        || path.chars().any(char::is_control)
    {
        bail!("expected a repository-relative directory or glob using *, ? and **");
    }
    if directory {
        if path.contains(['*', '?']) {
            bail!("directory inputs must be literal; use a glob without a trailing slash");
        }
        return Ok(Regex::new(&format!(
            r"\A{}/[\s\S]+\z",
            regex::escape(path)
        ))?);
    }
    if !path.contains(['*', '?']) {
        bail!("use files for an exact path or end a directory with /");
    }
    let mut expression = String::from(r"\A");
    for (index, part) in parts.iter().enumerate() {
        let last = index + 1 == parts.len();
        if *part == "**" {
            expression.push_str(if last { r"[\s\S]+" } else { "(?:[^/]+/)*" });
            continue;
        }
        if part.contains("**") {
            bail!("** must occupy a complete path segment");
        }
        for character in part.chars() {
            match character {
                '*' => expression.push_str("[^/]*"),
                '?' => expression.push_str("[^/]"),
                literal => expression.push_str(&regex::escape(literal.encode_utf8(&mut [0; 4]))),
            }
        }
        if !last {
            expression.push('/');
        }
    }
    expression.push_str(r"\z");
    Ok(Regex::new(&expression)?)
}

pub(crate) fn select(
    config: &Config,
    plan: &Plan,
    touched: &BTreeSet<String>,
    patterns: &InputPatterns<'_>,
) -> (Vec<VerificationCommand>, Vec<CommandDecision>) {
    let mut selected = Vec::new();
    let decisions = config
        .commands
        .iter()
        .map(|command| {
            let mut reasons = Vec::new();
            if !plan.fallback_reasons.is_empty() {
                reasons.extend(plan.fallback_reasons.iter().cloned());
            }
            for file in command.files.iter().filter(|f| touched.contains(*f)) {
                reasons.push(format!("command_input:{file}"));
            }
            for file in touched {
                for pattern in patterns.matching(&command.id, file) {
                    reasons.push(format!("command_input_pattern:{pattern}:{file}"));
                }
            }
            for o in plan
                .required_obligations
                .iter()
                .filter(|o| o.obligation.commands.contains(&command.id))
            {
                reasons.push(format!("relation:{}", o.relation_id));
                reasons.push(format!("property:{}", o.property_id));
                reasons.push(format!("obligation:{}", o.obligation.id));
                if let Some(r) = plan.relations.iter().find(|r| r.id == o.relation_id) {
                    reasons.push(format!("contract:{}", r.contract_id));
                }
            }
            let chosen = !reasons.is_empty();
            if chosen {
                selected.push(command.clone());
            } else {
                reasons.push("no_path_from_changes_or_contract_property_relationships".into());
            }
            CommandDecision {
                id: command.id.clone(),
                selected: chosen,
                reasons,
            }
        })
        .collect();
    (selected, decisions)
}

fn valid_case_id(id: &str) -> bool {
    !id.trim().is_empty() && !id.chars().any(char::is_control)
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct CommandReport {
    schema: String,
    cases: Vec<CommandCase>,
}

fn read_report(path: &Path, config: &CommandReportConfig) -> Result<CommandReportResult> {
    const MAX_BYTES: u64 = 8 * 1024 * 1024;
    let metadata = fs::symlink_metadata(path).context("missing command report")?;
    if !metadata.is_file() || metadata.len() > MAX_BYTES {
        bail!("command report must be a regular file of at most 8 MiB");
    }
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_BYTES {
        bail!("command report exceeds 8 MiB");
    }
    let report: CommandReport =
        serde_json::from_slice(&bytes).context("invalid command report JSON")?;
    if report.schema != "concord.command-report.v1" {
        bail!("unsupported command report schema");
    }
    let mut ids = BTreeSet::new();
    let mut counts = [0; 3];
    for case in &report.cases {
        if !valid_case_id(&case.id) || !ids.insert(case.id.as_str()) {
            bail!("invalid or duplicate command case ID");
        }
        counts[match case.status {
            CommandCaseStatus::Passed => 0,
            CommandCaseStatus::Failed => 1,
            CommandCaseStatus::Skipped => 2,
        }] += 1;
    }
    for required in &config.required_cases {
        if !ids.contains(required.as_str()) {
            bail!("missing required case: {required}");
        }
    }
    Ok(CommandReportResult {
        sha256: git::hash(&bytes),
        passed: counts[0],
        failed: counts[1],
        skipped: counts[2],
        cases: report.cases,
    })
}

pub fn execute(
    root: &Path,
    commands: &[VerificationCommand],
    directory: &Path,
) -> Result<Vec<CommandResult>> {
    let mut results = Vec::new();
    for command in commands {
        let dir = directory
            .join("commands")
            .join(git::hash(command.id.as_bytes()));
        let cfg = RunnerConfig {
            kind: RunnerKind::Commands,
            program: command.program.clone(),
            args: command.args.clone(),
            version: String::new(),
            timeout_seconds: command.timeout_seconds,
        };
        let start = Instant::now();
        let report_path = dir.join("report.json");
        let mut report = None;
        let mut report_error = None;
        let (status, exit_code) = match runner::execute_with_report(
            root,
            &cfg,
            &[],
            &dir,
            "command",
            command.report.as_ref().map(|_| report_path.as_path()),
        ) {
            Ok(p) if p.stopped => ("interrupted_or_timeout", None),
            Ok(p) if p.exit.is_some() => {
                let mut status = if p.exit == Some(0) {
                    "passed"
                } else {
                    "failed"
                };
                if let Some(config) = &command.report {
                    match read_report(&report_path, config) {
                        Ok(value) => {
                            if value.failed != 0 || value.skipped != 0 {
                                status = "failed";
                            }
                            report = Some(value);
                        }
                        Err(error) => {
                            status = "invalid_report";
                            report_error = Some(format!("{error:#}"));
                        }
                    }
                }
                (status, p.exit)
            }
            Ok(_) => ("terminated", None),
            Err(e) => {
                fs::create_dir_all(&dir)?;
                fs::write(dir.join("error.log"), format!("{e:#}"))?;
                ("start_failed", None)
            }
        };
        results.push(CommandResult {
            id: command.id.clone(),
            exit_code,
            status: status.into(),
            stdout: dir.join("command.stdout.log").to_string_lossy().into(),
            stderr: dir.join("command.stderr.log").to_string_lossy().into(),
            elapsed_ms: start.elapsed().as_millis(),
            report,
            report_error,
        });
        if runner::INTERRUPTED.load(std::sync::atomic::Ordering::SeqCst) {
            break;
        }
    }
    Ok(results)
}
