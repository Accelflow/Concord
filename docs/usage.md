# Concord user guide

[日本語](ja/usage.md)

Concord is a standalone Rust CLI for selecting and explaining verification from Git changes. It supports one TypeScript or JavaScript project using Vitest 4.1.12, one Rust package using [Cargo and libtest](rust-runner.md), and [registered commands](verification-commands.md). Install the target project's runner and dependencies before use. Concord itself requires no cloud account, language model, or background service. Test collection in `scan` and `plan` can execute project configuration and test code. Run them in a checkout you trust.

## Concepts

| Term | Role |
| --- | --- |
| Requirement | The goal or behavior to support. |
| Contract | An agreement with a scope, assumptions, preconditions, responsibility, version, and approval status. |
| Property | A specific behavior the software should satisfy. |
| Relation | A link between a Contract and a Property, with applicability, implementation paths, and verification obligations. |
| Verification obligation | A check required for the Relation or Property. It can reference named tests, registered commands, or both. |
| Result | Evidence tied to a commit, input fingerprint, environment, and run. |

Contracts and Properties form a [many-to-many model](many-to-many.md). For a change affecting one Relation, Concord includes the other Relations of that Contract and follows `depends_on` upward to dependent Properties and their Contracts. Changing a shared Property implementation affects every Relation that uses it. People review the natural-language assumptions and conditions. The run records evidence from the configured checks.

## Configure a project

Put `concord.toml` in the repository root. This is a minimal Vitest runner configuration.

```toml
schema_version = 1
non_runtime_paths = ["README.md"]
always_test_files = []

[runner]
program = "node"
args = ["node_modules/vitest/vitest.mjs"]
version = "4.1.12"
timeout_seconds = 120
```

Concord runs `program` and `args` without a shell. Omit Vitest filters and configuration overrides from `args`. `non_runtime_paths` matches only exact paths reviewed as non-runtime inputs.

For a new setup, use one `schema_version: 2` definition per YAML file under `concord/requirements/`, `contracts/`, `properties/`, and `relations/`. See the [recommended layout](folder-structure.md) and the runnable [search and detail demo](../examples/search-detail/README.md) for real definitions and test names. The single-file `concord/contracts.yaml` schema 2 format also works, including the compatible `property_ids` representation. Use one layout per repository.

Use repository-relative paths and exact collected test names. Vitest joins nested suite names with ` > `. Supported methods are `test`, `unit`, `api`, `integration`, and `e2e`. Authors choose the method and review whether the test covers the stated behavior. Commit the settings, definitions, source, and tests. Ignore `/.concord/`.

Approval status is `proposed` (the default), `approved`, or `deprecated`. Review implementation references, tests, and conditions before setting `approved`. Concord reports unknown or duplicate IDs, invalid schema fields, unsafe paths, and invalid references as input errors. Missing implementations, tests that cannot be collected, and incomplete obligations can leave affected Properties unverified.

## Commands and output

| Command | Output |
| --- | --- |
| `concord check --json` | Validate definitions and, when discovered in the repository root, configuration. |
| `concord scan --json` | Collect definitions, tests, dependencies, and the definition source. |
| `concord plan --base main --json` | Record affected Properties, Contracts and Relations, required obligations, gaps, and reasons for selection or exclusion. |
| `concord explain <test-id> --json` | Look up selection reasons and related definitions in the most recently saved plan. Registered command IDs work here too. |
| `concord verify --base main --json` | Replan and execute against the present inputs. Report case, obligation, and Property results. |
| `concord verify --all --json` | Run the full configured verification suite. |

CLI result documents use schema version 3. `--json` prints compact JSON on one line. The default prints the same data formatted for reading. A test ID comes from `tests[].id` in a scan or plan, or from `cases[].test_id` for a matching native case in a verify result. Earlier results and cases without a plan match may omit that field. The explanation describes the saved plan and its inputs.

Selection compares the working tree against the merge base of the specified ref and HEAD, including relevant staged, unstaged and untracked inputs. Concord expects the base ref to exist locally. It considers both paths of a rename and expands to a full run for deletions when necessary.

## Read verification results

Property result rows are keyed by Relation. The approval states of the relevant Contract and Property remain separate from the test results.

| Status | Meaning |
| --- | --- |
| `verified` | Every required check succeeded for these inputs and the Contract and Property are approved. |
| `unverified` | Required evidence is missing, a check was skipped, or a definition is awaiting approval. |
| `failed` | A required test or command failed. |
| `stale` | Inputs changed during execution, so the result cannot be applied to the current input. |

For example, successful unit tests leave a Property unverified when a required API check has yet to run. Prior runs remain associated with their original inputs. Compare fingerprints or verify again when applying results to new inputs.

| Exit code | Meaning |
| --- | --- |
| 0 | Selected verification passed, or the plan was explicitly a `noop`. |
| 1 | A test or command failed. |
| 2 | Input, configuration, Git, collection, or state error. |
| 3 | Interrupted or timed-out execution, inconsistent results, or inputs changed during the run. |
| 4 | Property verification gap. |

Skipped tests do not count as successes. Vitest can omit skipped or todo cases during collection. Concord records such cases when the execution report locates them within the selected files. Unexpected executed cases, missing required cases, duplicates, and out-of-scope results invalidate verification.

## Compatibility and selection limits

The old schema 1 `concord/guarantees.yaml` is supported for verification when no canonical definitions are present. Concord retains the old IDs, creates compatible Contracts and obligations in memory, and reports `legacy_schema: true`. The translated definitions are proposed because the old schema lacks explicit approval and Contract conditions. Review and migrate them to schema 2 before verifying as approved. Existing SQLite history is preserved. Version 3 snapshots, plans, and runs use separate namespaces from older records.

Vitest selection analyzes supported static relative imports and exports. Dynamic or unresolved dependencies, external imports outside supported handling, shared configuration and lockfile changes, or unknown changes can trigger a full run. Vite, Vitest, workspace and TypeScript configuration files and `.env*` inputs can trigger fallback by their presence because their semantics are outside the native analyzer. Multiple projects, symlinks, and submodules are outside the supported native analysis. Compare against a full run when adopting selection.

Concord stores local state in `.concord/state.sqlite` and run artifacts in `.concord/runs/<run-id>/`. It fingerprints inputs without storing the entire environment. Review raw test logs before sharing them.

## Check definitions while editing

```sh
concord check --json
concord check concord --json
concord check concord --config concord.toml --json
concord check drafts/contracts.yaml --json
```

With no definition path, `check` discovers split definitions or the single schema 2 definitions file. It also validates command references, exact input paths, and pattern syntax when it discovers `concord.toml`. An explicit definition path checks YAML alone unless you add `--config`. Point to the split definition root to validate cross-file references together.

The check reads YAML structure, IDs, references, enum values, test reference formats, and safe relative paths. It requires neither Git nor a runner and writes no state. It reports `status: valid` with exit code 0, or `status: invalid` with diagnostics and exit code 2. It also reports counts, warnings, and `verification_performed: false`. Warnings cover proposed or deprecated definitions, empty verification references, and empty documents. Use `scan`, `plan`, and `verify` to check actual implementations, available tests, and results. The authoring check targets schema 2.
