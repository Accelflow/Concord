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

Run the Rust unit tests with the same Rust toolchain:

```sh
cargo fmt --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
```

Use the [public demo](examples/search-detail/README.md) for an end-to-end walkthrough.
