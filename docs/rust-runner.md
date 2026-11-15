# Plan and verify Rust tests

Concord supports collecting, selecting, and running standard Rust tests for one
Cargo package in one Git repository. The Cargo runner reports verification by
Contract–Property Relation.

## Configure and run

Place this configuration in the repository's `concord.toml`.

```toml
schema_version = 1
non_runtime_paths = ["README.md"]
always_test_files = []

[runner]
kind = "cargo"
program = "cargo"
args = []
version = "1.97.1"
timeout_seconds = 600
```

The default runner kind is `vitest`. The Cargo adapter checks both Cargo and rustc
for version 1.97.1, uses the existing lockfile with `--locked`, and accepts no
extra runner arguments. The timeout applies to each external process.

```sh
mise exec -- cargo run --locked -- check concord --json
mise exec -- cargo run --locked -- scan --json
mise exec -- cargo run --locked -- plan --base HEAD^ --json
mise exec -- cargo run --locked -- verify --base HEAD^ --json
mise exec -- cargo run --locked -- verify --all --base HEAD --json
```

`HEAD^` compares against the previous commit. Supply an existing ref for your
change. Use `verify --all` to run the suite even when the change selects nothing.

## Collection and execution

The adapter checks package metadata and compiles tests with
`cargo test --locked --tests --no-run --message-format=json`. It uses executable
and target paths from compiler artifacts, then collects names with
`--list --format terse`. Collection can run builds and build scripts, so use
a checkout you trust.

`Test.file` is the target root, typically `tests/<target>.rs` for integration
tests or `src/lib.rs` for library unit tests. Use the complete libtest name,
including its module separators `::`, in `Test.name`.

```yaml
# Put this reference in the Relation definition.obligations.
- id: O-RUST-API
  method: integration
  tests:
    - file: tests/api.rs
      name: api
```

Concord executes all cases in each selected Cargo test target. It checks the
executable checksum recorded during collection and reconciles the single-threaded,
colorless pretty output with the final libtest summary.

## Impact selection

Concord 0.5.0 reads rustc dep-info associated with Cargo artifacts. It traces
actual compiler inputs, including Rust modules, `#[path]`, and inputs used by
`include_str!` and `include_bytes!`. The active configuration, features, and OS
determine those inputs. Shared library dependencies are tracked at crate
granularity and can select every consumer. Tests that use `CARGO_BIN_EXE_<name>`
depend on that binary. Concord identifies matching binary dep-info by artifact
hash and combines dependencies for multiple matching candidates.

### Declare runtime inputs

Dep-info covers compiler inputs. Review additional reads such as
`read_to_string`, fixture copies, and files read by external commands. Declare
them per target.

```toml
[rust.runtime_inputs]
"src/lib.rs" = []
"tests/api.rs" = ["fixtures/api/"]
"tests/other.rs" = []
```

Keys are collected Cargo target root paths. Values are repository-relative
files or directory prefixes ending in `/`. Directory prefixes include indexed
tracked or untracked inputs, including additions. An empty array records a
review that found no extra repository inputs. Update the entry when runtime
reads change. For unrepresentable external inputs, omit the declaration so
Concord falls back to all targets.

Concord reports `rust_runtime_inputs_unreviewed` if any target lacks a
declaration. Unknown targets and unsafe paths are errors. Missing or ambiguous
dep-info, inputs outside the index, unknown binary dependencies, package build
scripts, and Cargo configuration files can also force full verification.

### From changes to targets

Concord follows the graph backward from changed files and adds obligations
required by Contract–Property Relations. Cargo manifests and lockfiles, Concord
settings and definitions, unknown changes, unmapped Rust files, and uncertain
additions or deletions can trigger full verification. A change to an exact
`non_runtime_paths` entry can yield `noop` when no dependencies or other
fallback reasons apply.

For example, `tests/split_definitions.rs` reads other test sources at runtime.
The repository declares those inputs so a change to
`tests/definition_check.rs` can select both targets.

## Results and supported scope

Run artifacts include `cargo.json`, per-target stdout and stderr, and
Relation results in `result.json`. The original libtest exit code 101 is
preserved in `cargo.json` and maps to Concord exit code 1. Ignored cases do
not count as passes. Missing, duplicate, unknown, or inconsistent outcomes,
interruption, timeouts, and changed inputs or executables invalidate the run.
A run with only ignored cases cannot satisfy verification. An unapproved
definition can still produce exit code 4 when the tests succeed.

The supported scope is one Cargo package, default features, and host-native
unit and integration tests using standard libtest. Workspaces with multiple
packages, path dependencies, `harness=false`, and benchmarks are rejected.
Doctests, example or benchmark tests, cross-compilation, and arbitrary feature
or target selection are outside this runner's scope.

See the root [README](../README.md) for repository checks, the [Cargo test
reference](https://doc.rust-lang.org/cargo/commands/cargo-test.html) and
[libtest reference](https://doc.rust-lang.org/rustc/tests/) for runner behavior,
and [Cargo's dep-info documentation](https://doc.rust-lang.org/cargo/reference/build-cache.html#dep-info-files)
for compiler inputs.
