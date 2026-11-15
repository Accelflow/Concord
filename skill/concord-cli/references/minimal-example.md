# Minimal setup using a registered command

This example assumes an existing `src/storage.js` and
`tests/storage.test.mjs`. Replace the paths and conditions with ones from the
target project. Definitions start as `proposed`. Review the conditions,
implementation, and verification mapping before setting them to `approved`.
A definition check can succeed while approval and verification are pending.

## concord.toml

```toml
schema_version = 1

[runner]
kind = "commands"

[[commands]]
id = "document-tests"
program = "node"
args = ["--test", "tests/storage.test.mjs"]
files = ["tests/storage.test.mjs"]
timeout_seconds = 120
```

## concord/requirements/R-DOCUMENT.yaml

```yaml
schema_version: 2
definition:
  id: R-DOCUMENT
  description: A saved document can be read back.
```

## concord/contracts/C-STORAGE.yaml

```yaml
schema_version: 2
definition:
  id: C-STORAGE
  version: "1"
  description: Provide document storage and retrieval.
  scope: Document storage API
  assumptions: [The storage backend is accessible.]
  preconditions: [The document ID and body are valid.]
  responsibility: Save and return the same document body under the supplied ID.
  approval_status: proposed
  requirement_ids: [R-DOCUMENT]
```

## concord/properties/P-DOCUMENT.yaml

```yaml
schema_version: 2
definition:
  id: P-DOCUMENT
  version: "1"
  description: The retrieved document body matches the saved body.
  approval_status: proposed
  implementations: []
  depends_on: []
  obligations: []
```

## concord/relations/REL-STORAGE-DOCUMENT.yaml

```yaml
schema_version: 2
definition:
  id: REL-STORAGE-DOCUMENT
  contract_id: C-STORAGE
  property_id: P-DOCUMENT
  relation: requires
  applicability:
    condition: A valid document is saved and retrieved under the same ID.
  implementations: [src/storage.js]
  obligations:
    - id: O-STORAGE-ROUNDTRIP
      method: integration
      commands: [document-tests]
```

The command assumes Node.js and the existing test are installed. Use a
script that checks the underlying report when zero executed or skipped
cases must cause failure.

A change to `src/storage.js` can select the Relation and execute
`document-tests`. Review the test against the Property and applicability
condition. For an independent check of a composed Property, declare
`depends_on` and attach the relevant integration or E2E test or command.
