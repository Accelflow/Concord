# The many-to-many verification model

Define Contracts and Properties independently, then link them with Relations. Two Contracts can require the same Property and still have separate obligations and results. Use the [split layout](folder-structure.md) for new projects. The [single-file YAML example](../examples/many-to-many.yaml) illustrates the same model.

```yaml
relations:
  - id: REL-SEARCH-PRIVATE
    contract_id: C-SEARCH
    property_id: P-PRIVATE
    relation: requires
    applicability:
      condition: The viewer lacks permission to read the private document.
    implementations: [src/search.ts]
    obligations:
      - id: O-SEARCH-PRIVATE
        method: api
        tests:
          - file: tests/search.test.ts
            name: search > hides private data
```

| Link | Meaning | Where to put verification obligations |
| --- | --- | --- |
| `requires` | A Contract independently satisfies a Property. | Put Contract-specific checks on the Relation. |
| `contributes_to` | A Contract contributes to a composed Property. | Put its checks on the Relation and independent integration or E2E checks on the composed Property. |
| `depends_on` | One Property depends on another. | List lower-level Property IDs in the higher-level Property's `depends_on`. |

Give every Relation a unique ID and reuse the same Property ID. Put Contract-specific paths in the Relation's `implementations`. Put shared code in the Property's `implementations` so a change to it affects every applicable Relation. Explicit Relations require a Property version. A `requires` obligation belongs on its Relation. Concord rejects definitions that place those obligations only on the Property.

## Read results independently

For the same `P-PRIVATE`, search can pass while detail fails. Each `property_results` row describes one Relation.

```text
REL-SEARCH-PRIVATE / C-SEARCH / P-PRIVATE → passed / verified
REL-DETAIL-PRIVATE / C-DETAIL / P-PRIVATE → failed / failed
```

A failure or pending approval on one Contract leaves the other Contract's result independent. Concord withholds verification when execution results are inconsistent or inputs change. Rows record Relation, Contract and Property IDs and versions, applicability, Contract scope, commit, run ID, and environment and input fingerprints. `result` is `passed`, `failed`, or `inconclusive`. `status` is `verified`, `failed`, `unverified`, or `stale`. Obligation records also identify the Relation.

`applicability.condition` records a human-reviewed condition, with `applicability_evaluated: false`. A pass reflects the configured checks for that stated condition.

## Select checks for composed behavior

From each affected Relation, Concord includes every Relation of the same Contract. It follows reverse `depends_on` links to higher-level Properties and their Contracts, repeating until the selection stabilizes. This can select checks with no direct code dependency. Unknown dependency IDs and cycles are validation errors.

A Property composed through `contributes_to` needs its own integration or E2E obligation. Passing lower-level unit tests alone leaves that obligation unsatisfied. Concord distinguishes missing, unexecuted, skipped, and failed verification. See [verification selection and commands](verification-commands.md).

## Compatibility and storage

Schema 2 supports explicit Relations, versioned Properties, and Property dependencies. Legacy `Contract.property_ids` is normalized in memory to `requires` Relations. Their IDs use `compat:` plus the SHA-256 of the JSON array containing the Contract and Property IDs. Existing IDs and obligations are preserved, and an omitted legacy Property version defaults to 1. Use either the explicit or compatibility representation within a definitions file.

CLI result JSON uses schema version 3. Concord stores new snapshots, plans, and runs under `snapshot:v3`, `plan:v3`, and `run:v3`, preserving older SQLite records and their original result granularity.
