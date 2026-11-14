# Concord quickstart

Use a Concord executable built for your operating system and architecture. A
source build needs Rust 1.97.1. The TypeScript demo needs Git, Node.js 24.22.1,
and pnpm 10.34.8 via Corepack.

## Try the search and detail demo

Follow the [demo walkthrough](examples/search-detail/README.md). It includes
a baseline, a search-only change, a shared authorization regression, and a
definition change. The shared regression shows how to use the failed
`cases[].test_id` with `concord explain`.

## Try one rule in your repository

1. Pick one requirement with an existing test. Add a Requirement, a Contract,
   a Property, and a Relation. Put the implementation and the exact test
   reference on the Relation. The [minimal example](skill/concord-cli/references/minimal-example.md)
   also shows how to register an existing test command.
2. Configure the runner in `concord.toml` for your existing test setup. The
   native runners support a single Vitest 4.1.12 project or a single Cargo
   package using Cargo/rustc 1.97.1. See the [runner and setup guide](docs/usage.md).
3. Commit the definitions as a baseline. Make a small implementation change,
   then run the commands below with a Git ref that identifies your baseline.

```sh
base=YOUR_BASELINE_REF
concord check --json
concord plan --base "$base" --json
concord verify --base "$base" --json
concord explain YOUR_TEST_ID --json
```

`check` validates definitions and configuration. `plan` shows what will run
and why. `verify` executes the selected tests and commands. `explain` looks up
a test ID or command ID in the latest saved plan.

An unselected test has no execution result for that run. A failed test was
executed and reported a failure. A `noop` plan runs no checks. Concord expands
to a full run when it cannot safely limit impact, such as after a definition
or shared configuration change. Inspect `fallback_reasons` to see why. To
request a full run yourself, use `concord verify --all --base "$base" --json`.
