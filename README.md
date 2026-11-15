# Concord

[日本語](docs/ja/README.md)

Concord is a standalone CLI that uses Git changes and relationships between
requirements, contracts, properties, implementation, and tests to select and
explain verification. It supports single-project TypeScript/JavaScript with
Vitest, single-package Rust with Cargo, and registered verification commands.

See the [quickstart](QUICKSTART.md) to try one rule in your own repository.

## Build and try the demo

Build from source with Rust 1.97.1 and the locked dependencies:

```sh
cargo build --locked
```

Follow the [search and detail API demo](examples/search-detail/README.md) to run
`check`, compare a selected run with a full run, and trace a failed test with
`explain`. The demo uses Node.js 24.22.1 and pnpm 10.34.8 via Corepack.

## Verify this checkout

With the same Rust, Node.js and pnpm versions:

```sh
(cd tests/fixtures/vitest && corepack pnpm install --frozen-lockfile --ignore-scripts)
cargo fmt --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --all-targets --locked -- --test-threads=1
cargo run --locked -- check --json
```
