# Concord 利用ガイド

[日本語ドキュメント](README.md) · [English](../usage.md)

## 前提と動作

Rustは0.4.0からCargo/libtestに対応する。[Rustのplan・verify](../rust-runner.md)を参照する。以下のランナー設定・解析説明はTS/JS＋Vitestについてのもの。共通の定義・承認・終了コード・保存形式はRustにも適用する。

Concord本体はRust製CLI。単一GitリポジトリのTypeScript/JavaScriptと、インストール済みVitest **4.1.12** を対象にする。テストケースを収集し、実行対象をテストファイル単位で選ぶ。Cloud・アカウント・LLM・常駐サービスは不要。Gitと対象プロジェクトのNode.js・Vitestを利用し、依存を自動取得しない。

`scan` と `plan` の収集でも対象の設定・テストファイルを読み込んでコードを実行する。信頼するcheckoutで利用する。

現行の多対多定義と関係ごとの結果は[多対多モデル](../many-to-many.md)を参照する。以下のproperty_ids形式は互換入力として利用できる。

## 概念

```text
Requirement → Contract ⇄ Property → Verification Obligation → Test / Command → Result
```

- Requirement: 目的・要求。
- Contract: どの条件で何を約束するか。適用範囲、前提条件、責任境界、版、承認状態を持つ。
- Property: 成立すべき個別の性質。自然言語で記述でき、形式的証明を必須にしない。
- Verification Obligation: Propertyについて実施が必要な検証責務。一つのPropertyにunitとapi等を複数設定できる。
- Test: 検証責務を確認する具体的なVitest／Cargoのケース。
- Command: 検証責務を確認する登録済みの任意コマンド。詳細は[検証コマンド](../verification-commands.md)を参照。
- Result: commit・入力fingerprint・環境・実行条件に結び付いた結果。

ContractとPropertyは多対多。変更に関わるRelationを起点に、同じContractの全Relationの検証責務へ広げる。さらにProperty.depends_onを逆方向にたどり、上位PropertyとそのContractの検証を追加する。同じPropertyを共有するだけの別Contractには広げない。

Contractのscope・assumptions・preconditions・responsibilityは、人間がレビューする条件・責任の記述である。CLIがこれらの自然言語を実行して前提成立を証明する機能はない。検証結果は、記載した前提のもとで指定の検証を実行した結果として読む。

導入先の配置と定義・生成結果の分け方は[推奨フォルダ構成](../folder-structure.md)を参照する。

## 設定ファイル

対象Gitリポジトリのrootへ `concord.toml` を置く。このランナー設定のschema_versionは1のまま。

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

`non_runtime_paths` は明示した完全一致のパスだけを非実行入力とみなす。runnerのprogram/argsはshellを介さず実行し、argsにVitestのフィルター・設定上書きオプションを含めない。

新規導入では、[推奨フォルダ構成](../folder-structure.md)に従って`concord/requirements/`・`contracts/`・`properties/`・`relations/`へ一つずつ定義を置く。各ファイルは`schema_version: 2`と`definition`を持つ。分類フォルダから読み込み、全体の参照を検証する。

以下の単一ファイル **`concord/contracts.yaml`、schema_version: 2** も互換形式として使える。分割定義との混在はエラーになる。

```yaml
schema_version: 2
requirements:
  - id: R-PRICE
    description: 注文金額を計算できる
contracts:
  - id: C-PRICE
    description: 注文金額計算の約束
    version: "1"
    scope: 注文金額計算API
    assumptions: [呼出し側が数量を渡す]
    preconditions: [数量は0以上]
    responsibility: 金額の計算を担当し、決済処理は担当しない
    approval_status: approved
    requirement_ids: [R-PRICE]
    property_ids: [P-PRICE]
properties:
  - id: P-PRICE
    description: 金額は数量の2倍になる
    approval_status: approved
    implementations: [src/price.ts]
    obligations:
      - id: O-PRICE-UNIT
        method: unit
        tests:
          - file: tests/price.test.ts
            name: price > doubles
      - id: O-PRICE-API
        method: api
        tests:
          - file: tests/price-api.test.ts
            name: price API > returns total
```

`name` はVitest収集結果の完全名。ネストしたsuiteは ` > ` で結ぶ。methodは `test` / `unit` / `api` / `integration` / `e2e`。分類は定義の作成者が指定し、現在はいずれもVitestケースへ結び付ける。テスト内容がAPI統合テストであるかをCLIが自動証明する意味ではない。

設定・契約定義・ソース・テストをGit管理し、`.gitignore` へ `.concord/` を追加する。ContractとPropertyのapproval_statusは `proposed` / `approved` / `deprecated`、省略時はproposed。人間がレビューしてapprovedを設定し、CLIは文言や承認状態を書き換えない。

Contractから不明なRequirement/Propertyへの参照、重複ID、Contractから参照されないProperty、空の必須メタデータ、repo外のパス、未知のschema/状態/methodは入力エラーになる。検証責務やそのtestsが空、対応するテストが未収集、実装参照が欠ける状態は、影響Propertyの検証不足として報告する。

## 操作と出力

| コマンド | 結果 |
| --- | --- |
| `concord scan --json` | 定義・テスト・静的依存を保存。definitionsにContract/Property/検証責務、definition_sourceに出典を出力 |
| `concord plan --base main --json` | affected_properties、affected_contracts、required_obligations、property_gaps、選択/除外・理由を保存 |
| `concord explain <test-id> --json` | 直近の保存planについてContract/Property/検証責務と選択・除外理由を表示 |
| `concord verify --base main --json` | 現在入力で再scan・再計画して実行。property_resultsとobligation_resultsを出力 |
| `concord verify --all --json` | 全テストと全Propertyの検証責務を確認 |

結果JSONのschema_versionは3。`--json` は一行、未指定は同じ構造の整形JSON。test-idはscan/planのtests[].idで、従来のファイル・完全名に基づくIDを維持する。explainは保存planの説明であり、現在入力の検証済みを意味しない。

比較はmerge-base(base,HEAD)から作業ツリーまでを使い、ステージ済み・未ステージ・未追跡の対象入力を含む。base欠落で自動fetchしない。renameは旧・新パスを扱い、削除は保守的に全実行へ広げる。

## 承認状態と検証状態

Propertyのapproval_statusと、property_results[].statusは独立する。各結果行はrelation_idで区別する。

| 検証状態 | 意味 |
| --- | --- |
| unverified | 責務の欠落・skip・未実行・未承認などで確認が揃わない |
| verified | 指定commit/入力/条件で全責務が成功し、当該Propertyと当該関係のContractが承認済み |
| failed | 当該Propertyの責務に対応するテストが失敗 |
| stale | 実行前後で入力が変化し、結果を現在入力に適用できない |

Unitだけ通ってAPIが未実施ならPropertyはunverified。テストが通ってもproposed/deprecatedをapprovedへ変えない。個々のobligation_resultsは検証手段の結果を保持し、Propertyを検証済みと判定するにはすべての責務と定義の条件が必要となる。

過去のrunは、過去commitに対する不変の履歴である。checkout変更に合わせて過去のverifiedを上書きしない。現在入力へ適用する場合はfingerprintを照合し、再verifyする。

| 終了コード | 意味 |
| --- | --- |
| 0 | 選択範囲の成功、または明示的なnoop |
| 1 | テスト失敗 |
| 2 | 設定・参照・Git・収集・DB等のエラー |
| 3 | 実行中断/timeout・無効な結果・入力変更・実行集合の不整合 |
| 4 | Propertyの検証不足（property_gap） |

ネイティブテストの `cases[]` は `file`・`name`・`status` を保持し、計画に対応するケースには `plan.tests[].id` と同じ `test_id` を出力する。失敗ケースのIDは `concord explain <test_id> --json` に渡せる。計画に対応しないケースや、以前のバージョンが保存した結果では `test_id` が省略される。

skipをpassに加えない。Vitestのlistはskip/todoを省くため、実行レポートにだけ現れる未実行ケースは、選択ファイル内であることを確認して記録する。未知の成功/失敗ケース、必要な実行ケースの欠落、重複・範囲外の結果を成功扱いにしない。

## 旧版との互換性

旧 `concord/guarantees.yaml`（schema1）は、canonicalなcontracts.yamlがない場合だけ読み込む。既存のG-*等のIDをProperty IDとして維持し、`legacy:<ID>` のContractと検証責務をメモリー上で作る。`legacy_schema: true` を出力する。

旧版にはContract条件と明示的な承認情報がないため、変換したContract/Propertyはproposedとなる。scan/planは利用できるが、そのままverifyしても検証済みとはせずcode 4になる。内容を人間がレビューし、schema2へ移して必要な条件と承認状態を記載する。両ファイルがある場合は曖昧な入力としてエラーにする。CLIは元YAMLやIDを自動変更しない。

SQLiteの既存snapshot/plan/runレコードはそのまま保持する。新モデルのレコードはsnapshot:v3 / plan:v3 / run:v3という別namespaceへ保存し、古いレコードを新しい型で誤読しない。旧v2レコードも保持し、旧latest planを新モデルの説明へ転用せず、planを再作成する。状態ファイルを自動削除・履歴消去しない。

## 初期版の解析範囲

対応する静的な相対import/exportを構文解析する。不明な動的依存、未解決import、Vitest以外の外部import、特殊構文、共有設定・lockfile・分類不能変更では全実行へ広げる。

`vite.config.*` / `vitest.config.*` / `vitest.workspace.*` / `tsconfig*.json` / `.env*` があれば、設定の意味解析が未対応のため未変更でも全実行になる。複数projectの削減、symlink/submoduleは未対応。隠れた依存を静的解析だけで完全に証明しないため、導入時はverify --allと比較する。

`.concord/state.sqlite` と `.concord/runs/<run-id>/` に定義・計画・結果・ログを保存する。環境変数全体は保存せず、内容hashで照合する。テストの生ログには機密情報が入り得るので、共有前に確認する。

## 文書作成時の形式チェック

ContractとPropertyは、上記のスキーマに従ってYAMLに記述する構造化文書である。SkillのMDファイルの形式確認と同じ目的で、次の専用コマンドを使える。

```sh
concord check
concord check concord --json
concord check concord --config concord.toml --json
concord check drafts/contracts.yaml
concord check drafts/contracts.yaml --json
```

定義パスを省略すると、現在ディレクトリの`concord/`に分類フォルダがあれば分割定義を確認し、なければ`concord/contracts.yaml`を確認する。同じディレクトリに`concord.toml`があれば、検証責務が参照するコマンドID、完全一致の入力ファイル、入力パターンの書式も照合する。定義パスを明示するとYAMLだけを検査でき、`--config`も指定すると設定を合わせて検査できる。分割定義ではルートを指定して参照先も一緒に確認する。Git・Node.js・Vitestは不要で、SQLiteやファイルへの書き込みは行わない。

確認範囲はYAML構文、schema 2の型・必須項目・未知フィールド、状態/methodの列挙値、IDの重複、Requirement/Contract/Property間の参照、検証責務のテスト参照形式、repo相対パスの安全性。最初に見つかったエラーをdiagnosticsへ返す。テスト参照先や実装ファイルの存在・テスト名の実在は、実装前にも文書を書けるよう、このコマンドでは確認しない。

正常時は `status: valid`、終了コード0。構文・定義・参照不整合や読込み失敗時は `status: invalid`、終了コード2。JSONにはfile、counts、diagnostics、warnings、`verification_performed: false`を含める。`--json`未指定時は同じ結果を整形表示する。

proposed/deprecatedの定義、空の検証責務・テスト参照、空文書はwarningsとして表示する。下書きの形式を確認しただけでapprovedへ変えたり、Propertyをverifiedにしたりはしない。検証責務を満たせるかの確認にはscan/plan/verifyを使う。

旧schema1は既存CLIの互換読込み用。新しい文書を確認するcheckコマンドはschema2を対象とし、旧形式を自動変換して有効扱いにはしない。

Contract全体の責務とProperty依存による間接検証、および任意コマンド登録の現行仕様は[検証コマンド](../verification-commands.md)を参照。
