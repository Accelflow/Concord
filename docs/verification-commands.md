# Select and run verification commands

Concord combines code dependency analysis, Contract and Property Relations, and
declared command inputs. This can select integration and E2E checks even when
the source import graph has no path to them.

## Selection rules

1. Find changed files and their direct and indirect dependents.
2. Select Relations matched by Property or Relation `implementations`, test
   paths, or command `files` and `input_patterns`.
3. Include the other Relations of each selected Contract.
4. Follow reverse `depends_on` links to higher-level Properties and include
   their Contracts. Repeat until the selection stabilizes.
5. Run the obligations on the selected Relations. For `contributes_to`, include
   the Property's independent integration or E2E obligations.

Sharing a Property alone does not select a different Contract. Changes to the
Property's shared implementation do affect every Relation that uses it.
Legacy `property_ids` are normalized to Relations before selection.
`applicability.condition` is recorded for reviewers and never used as an
executable filter.

Command-only mode has no source dependency graph. Unknown inputs, including
undeclared TS or JS files, force a full command run even when some commands
are already selected. Existing Vite, Vitest, TypeScript, and environment
configuration files alone do not force command-only fallback. Changing those
files, manifests, lockfiles, pnpm workspace settings, or Concord definitions or
settings does. Exact `non_runtime_paths` entries cannot override shared
configuration changes. Native runners retain their own conservative fallback,
which includes all configured tests and commands when the impact is uncertain.

## Register a command

Configure commands in `concord.toml`. Commands can supplement the Vitest or
Cargo runner. Set the runner kind to `commands` to use registered commands alone.

```toml
schema_version = 1

[runner]
kind = "commands"

[[commands]]
id = "document-recovery-e2e"
program = "pnpm"
args = ["exec", "playwright", "test", "e2e/document-recovery.spec.ts"]
files = ["e2e/document-recovery.spec.ts"]
timeout_seconds = 300
```

Concord invokes `program` and `args` directly from the repository root.
Use a repository script for setup, teardown, or shell operations. List
existing repository-relative files in `files` for exact matches.

Use `input_patterns = ["e2e/recovery/", "tests/**/*.test.ts"]` to select the
command when matching files change, are added, or are deleted. A trailing slash
matches descendants. Within a path segment, `*` matches zero or more
characters and `?` matches one. A standalone `**/` matches zero or more
directory levels, and trailing `/**` matches descendants. A pattern may match
no current files. Exact files and patterns can be combined. With neither,
selection follows the Contract and Property relationships. A pattern match
appears in the reasons with the pattern and path.

Reference the command ID from the corresponding obligation.

```yaml
obligations:
  - id: O-DOCUMENT-RECOVERY
    method: e2e
    commands: [document-recovery-e2e]
```

An obligation can require both tests and commands. A command shared by several
obligations runs once. `concord check` in the repository root validates YAML
and, when present, command IDs, exact input paths, and pattern syntax in
`concord.toml`. Use `concord check concord` to check YAML alone or
`concord check concord --config concord.toml` to specify both.
Input paths resolve relative to the configuration file. `scan` and `plan`
also check that exact inputs are covered by Git change detection.

## Read command results

`plan` emits `selected_commands` and `command_decisions`. Use
`concord explain <command-id>` for the saved selection reasons. `verify`
runs selected commands serially in registration order and records exit codes,
statuses, durations, and stdout and stderr paths in `command_results`.
Logs are stored in `.concord/runs/`.

For commands without a report requirement, exit code 0 indicates success.
Nonzero exits, launch failures, signals, timeouts, and interruptions cannot
satisfy verification. On Unix, timeouts and interruptions stop the child
process group. Changed inputs make the run stale. Invalid commands and
unresolved command IDs are rejected before execution.

## Report individual cases

Declare stable case IDs when an obligation needs evidence from specific cases.

```toml
[[commands]]
id = "document-recovery-e2e"
program = "python3"
args = ["scripts/verify-recovery.py"]

[commands.report]
required_cases = ["document/save", "document/recover"]
```

Write the report to the path in `CONCORD_COMMAND_REPORT`, using the runner's
actual output.

```json
{
  "schema": "concord.command-report.v1",
  "cases": [
    {"id": "document/save", "status": "passed"},
    {"id": "document/recover", "status": "passed"}
  ]
}
```

Case IDs must be unique within the report. Include every required ID.
Statuses are `passed`, `failed`, and `skipped`. Extra cases use the same
format. Success requires exit code 0, every required case, and success for
all reported cases. Missing, duplicate, incomplete, or malformed reports
produce exit code 3. A valid report with failures or skips cannot satisfy
verification. Use a regular file no larger than 8 MiB.

`command_results[].report` records case information, counts, and the SHA-256
of the original JSON. `obligation_results[].command_ids` links an obligation
to its commands. Top-level `cases` and counts are for the native runner.
Keep command case counts in their own report. A Playwright or Vitest adapter
must convert the actual runner report so an unexecuted case stays detectable.

## Pin a plan

Use the same source, base, and execution environment to generate
`concord plan --base REF --json` and run
`concord verify --base REF --expect-plan ID --json` with its `plan_id`.
For a full run, pass `--all` to both. A different plan ID stops execution
before any selected test or command runs. If the environment changes, generate
the plan where verification will run.

## Avoid recursive verification

Commands invoked by `verify` should avoid recursively invoking `verify`.
