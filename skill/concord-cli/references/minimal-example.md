# コマンド登録だけで始める最小構成

既存の`src/storage.js`と、それを確認する`tests/storage.test.mjs`を持つリポジトリの例。パス・条件・説明・検証内容は導入先に合わせる。この例の承認状態は`proposed`なので、形式チェックに成功しても、そのままではverifyの検証済み条件を満たさない。実装とテストの対応・責任・前提をレビューしたあとで、ContractとPropertyを`approved`へ変更する。

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

このコマンドは単体・統合・E2Eなど、実際の検証方法に置き換えられる。Node.jsの実行環境とテストは事前に用意する。テスト0件やskipを拒否する必要があれば、その判定も行うスクリプトを登録する。

## concord/requirements/R-DOCUMENT.yaml

```yaml
schema_version: 2
definition:
  id: R-DOCUMENT
  description: 保存した文書を再び読み出せる
```

## concord/contracts/C-STORAGE.yaml

```yaml
schema_version: 2
definition:
  id: C-STORAGE
  version: "1"
  description: 文書の保存と読出しを提供する
  scope: 文書保存API
  assumptions: [保存先へアクセスできる]
  preconditions: [文書IDと文書本文が有効である]
  responsibility: 指定したIDの文書を保存し同じ本文を読み出せるようにする
  approval_status: proposed
  requirement_ids: [R-DOCUMENT]
```

## concord/properties/P-DOCUMENT.yaml

```yaml
schema_version: 2
definition:
  id: P-DOCUMENT
  version: "1"
  description: 保存した本文と読み出した本文が一致する
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
    condition: 有効な文書を保存して同じIDで読み出す
  implementations: [src/storage.js]
  obligations:
    - id: O-STORAGE-ROUNDTRIP
      method: integration
      commands: [document-tests]
```

`src/storage.js`の変更からRelationを選び、`document-tests`を実行できる。実際のテストが記述した性質と適用条件を確認していることは、作成者がレビューする。

上位Propertyに間接的なE2Eを結ぶ場合は、別のPropertyの`depends_on`へ`P-DOCUMENT`を指定し、その上位PropertyをContractへRelationで結ぶ。上位Relationの検証責務、または`contributes_to`で要求するProperty自身のIntegration／E2E責務に、実行コマンドや収集済みテストを登録する。依存の記述だけでテストが生成されるわけではない。
