# Recommended repository layout

Keep each Requirement, Contract, Property, and Relation in its own YAML file. Concord loads split definitions and checks references across files. See the [user guide](usage.md) for runner setup.

```text
my-repository/
├── concord.toml                 # Runner configuration
├── concord/                     # Version-controlled definitions
│   ├── requirements/
│   │   └── R-001.yaml
│   ├── contracts/
│   │   ├── C-001.yaml
│   │   └── C-002.yaml
│   ├── properties/
│   │   └── P-001.yaml
│   └── relations/
│       ├── REL-001.yaml
│       └── REL-002.yaml
├── docs/concord/model.md        # Optional notes for readers
├── src/
├── tests/
├── package.json
├── pnpm-lock.yaml
├── .gitignore
└── .concord/                    # Local state and verification history
    ├── state.sqlite
    ├── lock
    ├── operations/
    └── runs/
        └── <run-id>/
            ├── plan.json
            ├── result.json
            └── vitest.json      # The Cargo runner writes cargo.json
```

## Version control

Track the settings, definitions, sources, tests, and explanatory documents in Git. Add this rule at the repository root.

```gitignore
/.concord/
```

The generated directory contains checkout-specific state, locks, results, and raw logs. Review logs for sensitive data before sharing selected evidence. Keep your project's existing source and test layout. Concord supports one Vitest project or one [Cargo package](rust-runner.md) per Git repository.

## Split definitions

| Directory | Contents |
| --- | --- |
| `concord/requirements/` | Product goals and requirements |
| `concord/contracts/` | Agreements, scope, assumptions, preconditions, responsibilities, versions, and approval |
| `concord/properties/` | Required behavior, version, approval, dependencies, shared implementation, and independent verification |
| `concord/relations/` | Contract-to-Property links, applicability, and relation-specific implementation and verification |

Use definition IDs for filenames and cross-references. A filename change does not require changing an ID. Each file contains schema version 2 and one `definition`. The parent directory determines its type. Concord reads immediate `.yaml` and `.yml` files in filename order and rejects nested directories, symlinks, and other extensions in definition directories.

```yaml
# concord/properties/P-001.yaml
schema_version: 2
definition:
  id: P-001
  version: "1"
  description: Only authorized users can read private records.
  approval_status: proposed
  implementations: []
  depends_on: []
  obligations: []
```

If two Contracts require `P-001`, write one Property and two Relations. Put Contract-specific paths and obligations on the corresponding Relation. Put independent integration or E2E obligations for a composed behavior on the higher-level Property. See the [many-to-many model](many-to-many.md) and the [split definitions in the runnable demo](../examples/search-detail/concord/).

When classification directories exist, Concord reads the split definitions and validates duplicate IDs, references, and dependency cycles together. The older single-file `concord/contracts.yaml` format works on its own. Concord rejects mixtures of split and single-file definitions, including legacy `guarantees.yaml`. YAML includes are unsupported.

## Implementation paths and test names

Write `implementations` and `tests[].file` relative to the target Git root. Use the exact collected case name. Vitest joins nested suites with ` > ` and runs selected files. Declare the verification method explicitly, regardless of the test directory name.

Optional documents under `docs/concord/` explain decisions to readers. A change to an unclassified document can trigger a full run. For reviewed non-runtime files, list exact paths in `concord.toml` under `non_runtime_paths`.

## Validate and run

```sh
concord check concord --json
concord scan --json
concord plan --base main --json
concord verify --base main --json
```

`check` validates formats and references. `scan` and later commands need Git, runner configuration, and the configured runtime. When adopting selection, compare results with `concord verify --all --json`. Concord writes run artifacts to `.concord/` and records separate Property results for each Relation. See the [user guide](usage.md) for status codes.

## Concord's own repository

The CLI lives in `src/`, regression tests in `tests/`, and the Vitest fixture in `tests/fixtures/vitest/`. Concord's own definitions live in `concord/`. The `examples/` directory holds standalone examples that you can copy to a project. See the root [README](../README.md) for the contributor verification commands.
