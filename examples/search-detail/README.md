# Verify a shared access rule with Concord

Search excludes private documents the viewer cannot read. Detail returns HTTP
404 for the same documents. Both use the access check in `src/access.ts`.
The Concord definition registers it once on `P-PRIVATE`. Each API has its own
Contract, Relation and Vitest test.

## Set up the demo

Use Git, Node.js 24.22.1, Corepack with pnpm 10.34.8, and a Concord executable
with native Vitest case IDs. To build Concord from this checkout, run
`cargo build --locked` with Rust 1.97.1 at the Concord repository root.
From that root, run the following commands in a shell. If you already have
the executable, set `CONCORD` to its absolute path instead.

```sh
CONCORD="${CONCORD:-$PWD/target/debug/concord}"
demo_dir="$(mktemp -d)"
cp -R examples/search-detail/. "$demo_dir/"
cd "$demo_dir"
git init -b main
git add .
git -c user.name=Concord -c user.email=demo@example.invalid commit -m "Demo baseline"
base="$(git rev-parse HEAD)"
corepack pnpm install --frozen-lockfile --ignore-scripts
"$CONCORD" check --json
"$CONCORD" verify --all --base "$base" --json
```

The baseline passes both tests. Each patch below applies to the same baseline.
Run the scenarios in order, or reverse the current patch before trying another.

## Change only the search implementation

```sh
git apply changes/search-only.patch
"$CONCORD" plan --base "$base" --json
"$CONCORD" verify --base "$base" --json
"$CONCORD" verify --all --base "$base" --json
git apply -R changes/search-only.patch
```

The plan selects `tests/search.test.ts` with reason
`relation:REL-SEARCH-PRIVATE`. The selected run passes one test. The full run
passes both tests.

## Introduce a regression in the shared access check

```sh
git apply changes/shared-regression.patch
"$CONCORD" plan --base "$base" --json
"$CONCORD" verify --base "$base" --json
```

The plan selects both test files. The patch grants support staff access to other
users' documents. The search case uses a regular viewer and passes. The detail
case uses a support viewer and fails because it receives 200 instead of 404.
In `property_results`, `REL-SEARCH-PRIVATE` is `verified` and
`REL-DETAIL-PRIVATE` is `failed`.

The verify command exits with code 1. Copy the failed case's ID from
`cases[].test_id`, then run the commands below with that ID. The explanation
shows the selection reasons, `REL-DETAIL-PRIVATE`, `C-DETAIL` and `P-PRIVATE`.
The full verify also exits with code 1 and reports the same failing test.

```sh
"$CONCORD" explain YOUR_TEST_ID --json
"$CONCORD" verify --all --base "$base" --json
git apply -R changes/shared-regression.patch
```

## Change a Concord definition

```sh
git apply changes/definition-change.patch
"$CONCORD" plan --base "$base" --json
"$CONCORD" verify --base "$base" --json
git apply -R changes/definition-change.patch
```

A definition change expands verification to the full suite. The plan reports
`mode: "all"` and lists the changed requirement in `fallback_reasons`. Both
tests pass.
