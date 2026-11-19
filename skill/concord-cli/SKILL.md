---
name: concord-cli
description: Set up Concord, define Requirements, Contracts, Properties, and Relations, register verification commands, or use check, scan, plan, verify, and explain to inspect verification and review requirements for potential conflicts.
---

# Concord CLI

Concord uses Git changes, implementation references, and verification obligations to select checks and explain the selection. Its CLI works with the target repository's existing runners.

## Identify the target and runner

- Confirm the target Git repository, existing test entry point, and comparison ref. Check the executable with `concord --version` and `concord --help`.
- Use an installed executable or build from an available Concord checkout with the pinned Rust toolchain and `cargo build --release --locked`. Use the executable's absolute path when switching repositories.
- Native support covers a single Vitest 4.1.12 project or a single Cargo package using Cargo/rustc 1.97.1 and standard libtest. Use registered commands for other workflows.
- Run collection and verification only in a trusted checkout. `scan` and `plan` can execute target-project configuration and test code.

## Connect definitions to verification

Put `concord.toml` at the Git root and use one schema 2 definition per file under `concord/requirements/`, `contracts/`, `properties/`, and `relations/`. Ignore generated `/.concord/`. An existing single-file `concord/contracts.yaml` is a separate supported layout.

For a new definition, read the [minimal setup](references/minimal-example.md). Translate the requested behavior into acceptance conditions and link each condition to a test or command that actually checks it. Use the exact `file` and `name` from `scan`. Cargo references use the target root such as `src/lib.rs` or `tests/api.rs`. Review and declare extra per-target reads in `[rust.runtime_inputs]`. An empty entry records a reviewed absence of extra repository inputs.

- Put Contract-specific obligations on its `requires` Relation. Reuse a shared Property ID across Contracts.
- For `contributes_to`, define an independent integration or E2E check on the composed Property. List the lower-level Property IDs in its `depends_on`.
- Treat approval as a review decision about the definition, implementation, and tests. A passing check or test does not grant approval.

## Register commands

In `[[commands]]`, set `id`, `program`, `args`, and any needed timeout and input declarations. Use `[runner] kind = "commands"` for command-only projects. Reference command IDs from the relevant obligations. Concord runs the arguments directly from the repository root. Use a repository script for setup, shell operations, and cleanup.

Use `files` for exact paths and `input_patterns` for reviewed directories or globs. Include shared runner scripts and report adapters in the input declarations when they affect the command. When an obligation requires specific cases, declare stable IDs in `[commands.report].required_cases` and convert actual runner results into `CONCORD_COMMAND_REPORT`.

```json
{"schema":"concord.command-report.v1","cases":[{"id":"document/save","status":"passed"}]}
```

Require a successful exit, a complete report, and successful reported cases. Commands whose success is fully described by exit status can omit the report.

## Validate, run, and explain

```sh
concord check --json
concord scan --json
concord plan --base main --json
concord verify --base main --json
concord verify --all --base HEAD --json
concord explain document-tests --json
```

Use a locally available base ref. Inspect `configuration_checked` when validating the runner configuration. `concord check concord --json` checks definitions alone. To check both explicitly, use `concord check concord --config concord.toml --json`.

To pin execution to a plan, pass its `plan_id` to `verify --expect-plan ID` with the same base and environment. Match `--all` on both commands for a pinned full run.

Compare representative changes against expected selections and a full run when adopting selection. A change affecting one Relation can expand within its Contract and to dependent Properties. A change to a shared Property implementation can select all its Relations. Uncertain impact or shared configuration changes can trigger full verification.

Read `mode`, selected files and commands, affected Relations, and fallback reasons in the plan. Read `status`, input integrity, case and command results, and per-Relation obligations and Property results in the run. A `noop` can exit 0 without executing tests. Exit codes are 1 for failed tests or commands, 2 for input or collection errors, 3 for interrupted or inconsistent verification, and 4 for a verification gap. Use a failed native case's `test_id` or a registered command ID with `explain`. Review run artifacts in `.concord/runs/<run-id>/` before sharing them.

## Review requirements for conflicts

Use `concord check` to establish that definitions and references are valid. A passing check does not establish that natural-language specifications are compatible.

1. Confirm the revision and follow each relevant Requirement to its Contracts. Compare Relations that share a Property, or Contracts that describe the same operation or refer to the same implementation or test. An overlap identifies a candidate to inspect.
2. Read each Contract's command, scope and preconditions and each Relation's `applicability.condition`. Find a concrete input, environment and state where both apply, then compare the required outcomes. Different conditions can make both requirements satisfiable. If their scope is unclear, report the ambiguity.
3. Compare the wording with the linked obligations, tests and implementation. If needed, reproduce the behavior in a temporary repository with minimal inputs and record the command and result. For example, run different CLI commands against the same legacy definition to distinguish compatibility loading from format validation.
4. Report the definition IDs and files, the relevant wording, the overlapping conditions and the observed result. Distinguish incompatible requirements, differences between specification and implementation, and ambiguous scope. When a compatible version can generate the network HTML, include selection URLs from the graph for that revision. Otherwise, cite the source files and IDs.
