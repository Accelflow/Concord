use crate::{analysis, definitions, git, model::*, planning, runner, store::Store};
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    time::Instant,
};

pub struct Engine {
    root: PathBuf,
    config: Config,
    store: Store,
    operation: PathBuf,
}
impl Engine {
    pub fn open() -> Result<Self> {
        let root = git::root()?;
        let config = Config::load(&root.join("concord.toml"))?;
        for path in config
            .non_runtime_paths
            .iter()
            .chain(&config.always_test_files)
        {
            git::safe_path(path)?;
        }
        for (target, inputs) in &config.rust.runtime_inputs {
            git::safe_path(target)?;
            for path in inputs {
                git::safe_path(path)?;
            }
        }
        let store = Store::open(&root)?;
        let operation = root.join(".concord/operations").join(runner::id());
        Ok(Self {
            root,
            config,
            store,
            operation,
        })
    }
    pub fn scan(&mut self) -> Result<Snapshot> {
        let files = git::files(&self.root)?;
        let config_file = self.root.join("concord.toml").canonicalize()?;
        if !config_file.starts_with(&self.root) {
            bail!("configuration outside repository")
        }
        let version = runner::version(&self.root, &self.config.runner, &self.operation)?;
        let (definitions, definition_source, legacy_schema) = definitions::load(&self.root)?;
        crate::commands::validate(&self.config, &definitions, |path| files.contains_key(path))?;
        let tests = runner::collect(&self.root, &self.config.runner, &self.operation)?;
        let graph = if self.config.runner.kind == RunnerKind::Cargo {
            crate::rust_analysis::graph(
                &self.root,
                &files,
                &self.config.rust,
                &self.operation,
                &tests,
            )?
        } else if self.config.runner.kind == RunnerKind::Commands {
            Graph::default()
        } else {
            analysis::graph(&self.root, &files)?
        };
        for test in &tests {
            if !files.contains_key(&test.file) {
                bail!("collected test outside indexed input: {}", test.file)
            }
        }
        if files != git::files(&self.root)? {
            bail!("inputs changed during test collection")
        }
        let extra = self.extra(&version)?;
        let fingerprint = git::fingerprint(&self.root, &files, &extra)?;
        let snapshot = Snapshot {
            schema_version: 3,
            fingerprint,
            head: git::text(&self.root, &["rev-parse", "HEAD"])?,
            files,
            tests,
            graph,
            definitions,
            definition_source,
            legacy_schema,
            runner_version: version,
        };
        self.store.save_snapshot(&snapshot)?;
        Ok(snapshot)
    }
    fn extra(&self, version: &str) -> Result<String> {
        let env: BTreeMap<String, String> = std::env::vars().collect();
        Ok(git::hash(&serde_json::to_vec(&(
            &self.config,
            version,
            env,
            "tree-sitter-0.27.1/adapter-4/cargo-2/commands-1/schema-3",
        ))?))
    }
    pub fn plan(&mut self, base: &str, all: bool) -> Result<(Plan, Snapshot)> {
        // Resolve the requested base even when --all was requested.
        let (base_sha, merge_base_sha, head_sha, changed) = git::changes(&self.root, base)?;
        let old = self.store.snapshot()?;
        let snapshot = self.scan()?;
        let input_patterns = crate::commands::InputPatterns::compile(&self.config)?;
        if snapshot.head != head_sha {
            bail!("HEAD changed during planning");
        }
        // Explicit non-runtime inputs alone do not activate unknown Rust runtime
        // dependencies. Known graph edges can still select their consumers below.
        let non_runtime_only = self.config.runner.kind == RunnerKind::Cargo
            && !all
            && changed
                .iter()
                .all(|p| self.config.non_runtime_paths.contains(p));
        let mut fallback: BTreeSet<String> = snapshot
            .graph
            .uncertainties
            .iter()
            .filter(|reason| {
                !non_runtime_only || !reason.starts_with("rust_runtime_inputs_unreviewed:")
            })
            .cloned()
            .collect();
        if all {
            fallback.insert("explicit_all".into());
        }
        let mut touched = changed.clone();
        let mut reverse: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for graph in std::iter::once(&snapshot.graph).chain(old.as_ref().map(|s| &s.graph)) {
            for (file, deps) in &graph.dependencies {
                for dep in deps {
                    reverse.entry(dep.clone()).or_default().insert(file.clone());
                }
            }
        }
        let runtime_configuration = |path: &str| {
            let name = Path::new(path)
                .file_name()
                .unwrap_or_default()
                .to_string_lossy();
            name.starts_with("vitest.config.")
                || name.starts_with("vite.config.")
                || name.starts_with("vitest.workspace.")
                || name.starts_with("tsconfig") && name.ends_with(".json")
                || name.starts_with(".env")
        };
        // Native adapters retain conservative resolution/collection checks. Commands
        // declare their own inputs: an unchanged config's presence is not an impact.
        for path in snapshot.files.keys() {
            if self.config.runner.kind != RunnerKind::Commands && runtime_configuration(path) {
                fallback.insert(format!("runtime_configuration:{path}"));
            }
        }
        for path in &changed {
            let name = Path::new(path)
                .file_name()
                .unwrap_or_default()
                .to_string_lossy();
            let command_configuration = self.config.runner.kind == RunnerKind::Commands
                && (runtime_configuration(path)
                    || path.starts_with("concord/")
                    || [
                        "concord.toml",
                        "Cargo.toml",
                        "package.json",
                        "pnpm-workspace.yaml",
                        "pnpm-lock.yaml",
                        "package-lock.json",
                        "npm-shrinkwrap.json",
                    ]
                    .contains(&name.as_ref())
                    || name.ends_with(".lock")
                    || name.ends_with(".lockb"));
            // A configuration change is shared impact even if declared as a command
            // input or accidentally whitelisted as non-runtime.
            if command_configuration {
                fallback.insert(format!("configuration_change:{path}"));
            }
            if self.config.non_runtime_paths.contains(path) && !command_configuration {
                continue;
            }
            if self.config.runner.kind == RunnerKind::Commands {
                let pattern_mapped = self
                    .config
                    .commands
                    .iter()
                    .any(|command| input_patterns.matching(&command.id, path).next().is_some());
                let mapped = pattern_mapped
                    || self.config.commands.iter().any(|c| c.files.contains(path))
                    || snapshot
                        .definitions
                        .properties
                        .iter()
                        .any(|p| p.implementations.contains(path))
                    || snapshot
                        .definitions
                        .relations
                        .iter()
                        .any(|r| r.implementations.contains(path));
                if mapped
                    && (pattern_mapped || snapshot.files.contains_key(path))
                    && !command_configuration
                {
                    continue;
                }
                if !mapped {
                    // Unknown TS/JS cannot be treated as a known source graph here:
                    // command mode intentionally has no static code dependency graph.
                    fallback.insert(format!("unmapped_command_input:{path}"));
                }
            }
            let cargo_known = self.config.runner.kind == RunnerKind::Cargo
                && (snapshot.graph.dependencies.contains_key(path)
                    || snapshot
                        .graph
                        .dependencies
                        .values()
                        .any(|deps| deps.contains(path)));
            if self.config.runner.kind == RunnerKind::Cargo && path.ends_with(".rs") && !cargo_known
            {
                fallback.insert(format!("rust_unmapped_source:{path}"));
            }
            if name == "Cargo.toml"
                || path.starts_with("concord/")
                || name == "concord.toml"
                || path == "concord/guarantees.yaml"
                || path == "concord/contracts.yaml"
                || name == "package.json"
                || name.contains("lock")
                || name == "pnpm-workspace.yaml"
                || name.starts_with(".env")
                || if self.config.runner.kind == RunnerKind::Cargo {
                    !cargo_known
                } else {
                    !analysis::is_source(path)
                }
            {
                fallback.insert(format!("shared_or_unknown_input:{path}"));
            }
            if !snapshot.files.contains_key(path) {
                fallback.insert(format!("deleted_input:{path}"));
            }
        }
        let mut pending: Vec<_> = touched.iter().cloned().collect();
        while let Some(path) = pending.pop() {
            if let Some(parents) = reverse.get(&path) {
                for parent in parents {
                    if touched.insert(parent.clone()) {
                        pending.push(parent.clone());
                    }
                }
            }
        }
        let mut selected: BTreeSet<String> =
            self.config.always_test_files.iter().cloned().collect();
        let all_files: BTreeSet<String> = snapshot.tests.iter().map(|t| t.file.clone()).collect();
        for path in &selected {
            if !all_files.contains(path) {
                bail!("always_test_files contains uncollected test: {path}")
            }
        }
        for t in &snapshot.tests {
            if touched.contains(&t.file) {
                selected.insert(t.file.clone());
            }
        }
        let changed_commands: BTreeSet<_> = self
            .config
            .commands
            .iter()
            .filter(|command| {
                command.files.iter().any(|file| touched.contains(file))
                    || touched
                        .iter()
                        .any(|path| input_patterns.matching(&command.id, path).next().is_some())
            })
            .map(|command| command.id.as_str())
            .collect();
        let mut context = planning::apply_properties(
            &snapshot,
            &touched,
            !fallback.is_empty(),
            &mut selected,
            &changed_commands,
        );
        let command_reachable = !changed_commands.is_empty()
            || snapshot
                .definitions
                .resolved_relations()
                .iter()
                .filter(|r| context.2.contains(&r.id))
                .any(|r| {
                    planning::obligations(&snapshot.definitions, r)
                        .iter()
                        .any(|o| !o.commands.is_empty())
                });
        if selected.is_empty()
            && !command_reachable
            && changed
                .iter()
                .any(|p| !self.config.non_runtime_paths.contains(p))
        {
            fallback.insert("changed_input_without_reachable_test".into());
            context = planning::apply_properties(
                &snapshot,
                &touched,
                true,
                &mut selected,
                &changed_commands,
            );
        }
        if !fallback.is_empty() {
            selected = all_files.clone();
        }
        let (affected, affected_contracts, affected_relations, gaps) = context;
        let decisions = snapshot
            .tests
            .iter()
            .map(|test| {
                let chosen = selected.contains(&test.file);
                let mut reasons = Vec::new();
                if chosen {
                    if !fallback.is_empty() {
                        reasons.extend(fallback.iter().cloned());
                    }
                    if touched.contains(&test.file) {
                        reasons.push(format!("dependency_or_test_change:{}", test.file));
                    }
                    for relation in snapshot
                        .definitions
                        .resolved_relations()
                        .iter()
                        .filter(|r| affected_relations.contains(&r.id))
                    {
                        for o in planning::obligations(&snapshot.definitions, relation) {
                            if o.tests.iter().any(|t| t.file == test.file) {
                                reasons.push(format!("relation:{}", relation.id));
                                reasons.push(format!("contract:{}", relation.contract_id));
                                reasons.push(format!("property:{}", relation.property_id));
                                reasons.push(format!("obligation:{}", o.id));
                            }
                        }
                    }
                    if self.config.always_test_files.contains(&test.file) {
                        reasons.push("always_test_file".into());
                    }
                } else {
                    reasons.push(if self.config.runner.kind == RunnerKind::Cargo {
                        "cargo_dep_info_and_declared_runtime_inputs:no_path_from_changes_or_affected_properties".into()
                    } else { "complete_static_graph:no_path_from_changes_or_affected_properties".into() });
                }
                Decision {
                    test: test.clone(),
                    selected: chosen,
                    reasons,
                }
            })
            .collect();
        let mode = if !fallback.is_empty() {
            "all"
        } else if selected.is_empty() {
            "noop"
        } else {
            "selected"
        };
        let mut plan = Plan {
            schema_version: 3,
            plan_id: String::new(),
            fingerprint: snapshot.fingerprint.clone(),
            base_sha,
            merge_base_sha,
            head_sha,
            mode: mode.into(),
            changed_files: changed,
            selected_files: selected.clone(),
            excluded_files: all_files.difference(&selected).cloned().collect(),
            fallback_reasons: fallback,
            contracts: snapshot
                .definitions
                .contracts
                .iter()
                .filter(|c| affected_contracts.contains(&c.id))
                .cloned()
                .collect(),
            properties: snapshot
                .definitions
                .properties
                .iter()
                .filter(|p| affected.contains(&p.id))
                .cloned()
                .collect(),
            required_obligations: snapshot
                .definitions
                .resolved_relations()
                .iter()
                .filter(|r| affected_relations.contains(&r.id))
                .flat_map(|r| {
                    planning::obligations(&snapshot.definitions, r)
                        .into_iter()
                        .map(|obligation| RequiredObligation {
                            relation_id: r.id.clone(),
                            property_id: r.property_id.clone(),
                            obligation,
                        })
                })
                .collect(),
            relations: snapshot
                .definitions
                .resolved_relations()
                .into_iter()
                .filter(|r| affected_relations.contains(&r.id))
                .collect(),
            affected_relations,
            affected_properties: affected,
            affected_contracts,
            property_gaps: gaps,
            tests: snapshot.tests.clone(),
            decisions,
            selected_commands: Vec::new(),
            command_decisions: Vec::new(),
        };
        (plan.selected_commands, plan.command_decisions) =
            crate::commands::select(&self.config, &plan, &touched, &input_patterns);
        if plan.mode == "noop" && !plan.selected_commands.is_empty() {
            plan.mode = "selected".into();
        }
        plan.plan_id = git::hash(&serde_json::to_vec(&plan)?);
        self.store.save_plan(&plan)?;
        Ok((plan, snapshot))
    }
    pub fn explain(&self, test_id: &str) -> Result<Value> {
        let p = self
            .store
            .latest_plan()?
            .context("no saved plan; run plan first")?;
        if let Some(d) = p.command_decisions.iter().find(|d| d.id == test_id) {
            return Ok(
                json!({"schema_version":3,"plan_id":p.plan_id,"fingerprint":p.fingerprint,"command":d,"saved_plan":true}),
            );
        }
        let d = p
            .decisions
            .iter()
            .find(|d| d.test.id == test_id)
            .context("unknown test id in latest plan")?;
        let relations: Vec<_> = p
            .relations
            .iter()
            .filter(|r| {
                p.required_obligations.iter().any(|o| {
                    o.relation_id == r.id
                        && o.obligation.tests.iter().any(|t| t.file == d.test.file)
                })
            })
            .collect();
        Ok(
            json!({"schema_version":3,"plan_id":p.plan_id,"fingerprint":p.fingerprint,"test":d.test,"selected":d.selected,"reasons":d.reasons,"saved_plan":true,
                "relations":relations,
                "properties":p.properties.iter().filter(|prop|relations.iter().any(|r|r.property_id==prop.id)).collect::<Vec<_>>(),
                "contracts":p.contracts.iter().filter(|c|relations.iter().any(|r|r.contract_id==c.id)).collect::<Vec<_>>()
            }),
        )
    }
    pub fn verify(&mut self, base: &str, all: bool, expected_plan: Option<&str>) -> Result<Run> {
        let start = Instant::now();
        let started_ms = runner::now();
        let (plan, snapshot) = self.plan(base, all)?;
        if let Some(expected) = expected_plan
            && expected != plan.plan_id
        {
            bail!(
                "planned verification changed: expected {expected}, got {}",
                plan.plan_id
            );
        }
        let plan_ms = start.elapsed().as_millis();
        let run_id = runner::id();
        let dir = self.root.join(".concord/runs").join(&run_id);
        fs::create_dir_all(&dir)?;
        fs::write(dir.join("plan.json"), serde_json::to_vec_pretty(&plan)?)?;
        let mut run = Run {
            schema_version: 3,
            run_id,
            plan: plan.clone(),
            status: "noop".into(),
            exit_code: 0,
            runner_exit_code: None,
            passed: 0,
            failed: 0,
            skipped: 0,
            cases: Vec::new(),
            command_results: Vec::new(),
            property_results: Vec::new(),
            obligation_results: Vec::new(),
            input_unchanged: true,
            result_integrity: false,
            environment_fingerprint: self.extra(&snapshot.runner_version)?,
            host: std::env::var("HOSTNAME").unwrap_or_else(|_| {
                String::from_utf8_lossy(
                    &std::process::Command::new("hostname")
                        .output()
                        .map(|o| o.stdout)
                        .unwrap_or_default(),
                )
                .trim()
                .to_owned()
            }),
            os: std::env::consts::OS.into(),
            started_ms,
            elapsed_ms: 0,
            plan_ms,
            execution_ms: 0,
            result_hash: String::new(),
        };
        let extra = self.extra(&snapshot.runner_version)?;
        if git::fingerprint(&self.root, &git::files(&self.root)?, &extra)? != snapshot.fingerprint {
            run.status = "input_changed_before_execution".into();
            run.exit_code = 3;
            run.input_unchanged = false;
        } else if !plan.selected_files.is_empty() {
            let result_path = dir.join(if self.config.runner.kind == RunnerKind::Cargo {
                "cargo.json"
            } else {
                "vitest.json"
            });
            let mut args = vec![
                "run".into(),
                "--reporter=json".into(),
                format!("--outputFile={}", result_path.display()),
            ];
            // Full relative filenames are substring filters in Vitest. Verify the exact
            // returned file and case set, so accidental extra or missing execution fails.
            args.extend(plan.selected_files.iter().map(|p| format!("./{p}")));
            let exec_start = Instant::now();
            let result = if self.config.runner.kind == RunnerKind::Cargo {
                crate::cargo_runner::run(
                    &self.root,
                    &self.config.runner,
                    &plan.selected_files,
                    &self.operation,
                    &dir,
                    &result_path,
                )
            } else {
                runner::execute(&self.root, &self.config.runner, &args, &dir, "run")
            };
            run.execution_ms = exec_start.elapsed().as_millis();
            match result {
                Ok(p) if !p.stopped => {
                    run.runner_exit_code = p.exit;
                    match runner::parse_results(&self.root, &result_path) {
                        Ok((report, mut cases)) => {
                            run.passed = cases.iter().filter(|c| c.status == "passed").count();
                            run.failed = cases.iter().filter(|c| c.status == "failed").count();
                            run.skipped = cases.len() - run.passed - run.failed;
                            let expected: BTreeMap<_, _> = plan
                                .tests
                                .iter()
                                .filter(|t| plan.selected_files.contains(&t.file))
                                .map(|t| ((t.file.as_str(), t.name.as_str()), &t.id))
                                .collect();
                            let actual: BTreeSet<_> = cases
                                .iter()
                                .map(|c| (c.file.as_str(), c.name.as_str()))
                                .collect();
                            let expected_exit = if run.failed > 0 { 1 } else { 0 };
                            run.result_integrity = p.exit == Some(expected_exit)
                                && report["success"] == (run.failed == 0)
                                && report["numTotalTests"].as_u64() == Some(cases.len() as u64)
                                && expected.keys().all(|key| actual.contains(key))
                                && !cases.iter().any(|c| {
                                    !plan.selected_files.contains(&c.file)
                                        || matches!(c.status.as_str(), "passed" | "failed")
                                            && !expected
                                                .contains_key(&(c.file.as_str(), c.name.as_str()))
                                })
                                && actual.len() == cases.len()
                                && run.passed + run.failed > 0;
                            if !run.result_integrity {
                                run.status = "incomplete".into();
                                run.exit_code = 3;
                            } else if run.failed > 0 {
                                run.status = "failed".into();
                                run.exit_code = 1;
                            } else {
                                run.status = "passed".into();
                            }
                            for case in &mut cases {
                                case.test_id = expected
                                    .get(&(case.file.as_str(), case.name.as_str()))
                                    .map(|id| (*id).clone());
                            }
                            run.cases = cases;
                        }
                        Err(_) => {
                            run.status = "invalid_runner_result".into();
                            run.exit_code = 3;
                        }
                    }
                }
                Ok(_) => {
                    run.status = "interrupted_or_timeout".into();
                    run.exit_code = 3;
                }
                Err(error) => {
                    fs::write(dir.join("runner-error.log"), format!("{error:#}"))?;
                    run.status = "runner_start_failed".into();
                    run.exit_code = 3;
                }
            }
        }
        if run.input_unchanged && !plan.selected_commands.is_empty() {
            // Command cases are recorded per command, independently of native test counts.
            if plan.selected_files.is_empty() {
                run.result_integrity = true;
                run.status = "passed".into();
            }
            let command_start = Instant::now();
            run.command_results =
                crate::commands::execute(&self.root, &plan.selected_commands, &dir)?;
            run.execution_ms += command_start.elapsed().as_millis();
            let complete = run.command_results.len() == plan.selected_commands.len()
                && run
                    .command_results
                    .iter()
                    .all(|c| matches!(c.status.as_str(), "passed" | "failed"));
            run.result_integrity &= complete;
            if !complete {
                run.status = "command_incomplete".into();
                run.exit_code = 3;
            } else if run.command_results.iter().any(|c| c.status == "failed") && run.exit_code != 3
            {
                run.status = "failed".into();
                run.exit_code = 1;
            }
        }
        // Re-read both index state and file content after execution.
        run.input_unchanged = git::files(&self.root)
            .and_then(|f| git::fingerprint(&self.root, &f, &extra))
            .is_ok_and(|fp| fp == snapshot.fingerprint);
        if !run.input_unchanged {
            run.status = "input_changed".into();
            run.exit_code = 3;
        }
        let (properties, obligations, gaps) = planning::evaluate(&run);
        run.property_results = properties;
        run.obligation_results = obligations;
        if run.exit_code == 0 && !gaps.is_empty() {
            run.status = "property_gap".into();
            run.exit_code = 4;
        }
        run.plan.property_gaps = gaps;
        run.elapsed_ms = start.elapsed().as_millis();
        run.result_hash = git::hash(&serde_json::to_vec(&run)?);
        fs::write(dir.join("result.json"), serde_json::to_vec_pretty(&run)?)?;
        self.store.save_run(&run)?;
        Ok(run)
    }
}
