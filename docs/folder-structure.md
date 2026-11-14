# 推奨フォルダ構成

Contract・Property・関係を独立した文書として管理するための推奨構成を示す。導入先プロジェクトの構成を中心に、末尾にConcord本体の開発リポジトリの構成を示す。

Standalone CLI 0.3.1から、この分割構成の読込みとファイル間の参照チェックに対応する。このリポジトリ自身も`concord/`配下に分割した定義を配置する。実際の使い方は[利用ガイド](usage.md)を参照する。

## 導入先プロジェクトの構成

```text
my-repository/
├── concord.toml                 # Concordのランナー設定
├── concord/                    # Git管理対象：定義をGitHubで共有する
│   ├── requirements/           # 製品の目的・要求
│   │   └── R-001.yaml
│   ├── contracts/              # Contractを独立して定義
│   │   ├── C-001.yaml
│   │   └── C-002.yaml
│   ├── properties/             # 複数Contractから共有するPropertyを定義
│   │   └── P-001.yaml
│   └── relations/              # ContractとPropertyをIDで結ぶ関係を定義
│       ├── REL-001.yaml
│       └── REL-002.yaml
├── docs/
│   └── concord/
│       └── model.md             # 定義の背景・用語・判断理由（必要な場合）
├── src/                        # 導入先リポジトリの実装コードを配置する
├── tests/
│   ├── unit/                   # 単体テストを配置する
│   └── integration/            # 統合テストを配置する
├── package.json
├── pnpm-lock.yaml               # 利用するパッケージマネージャーのlockfile
├── .gitignore
└── .concord/                    # Git管理対象外：CLIのローカル状態・実行履歴
    ├── state.sqlite
    ├── lock
    ├── operations/
    └── runs/
        └── <run-id>/
            ├── plan.json
            ├── result.json
            └── vitest.json      # Vitestの結果。Cargoランナーではcargo.jsonを生成
```

## Git管理とGitHubへの同期

**`concord/`はGit管理対象、`.concord/`はGit管理対象外とする。**

| ディレクトリ | Git管理 | GitHubへの同期 | 理由 |
| --- | --- | --- | --- |
| `concord/` | 対象 | 同期する | Requirement・Contract・Property・関係の定義を、コードと一緒に変更履歴へ残し、レビュー・共有するため |
| `.concord/` | 対象外 | 同期しない | 実行端末・作業ツリーに依存するSQLite、排他制御用ファイル、実行結果、生ログを保存するため |

導入先リポジトリのルートの`.gitignore`へ、次を追加する。

```gitignore
/.concord/
```

設定・定義・ソース・テスト・補足文書はGit管理する。`.concord/`内の生ログには機密情報が含まれる可能性があるため、検証証跡を共有する際は必要なファイルを選び、内容を確認してから共有する。`.concord/`全体をGitHubへ同期しない。

## 実装とテストの配置

`src/`と`tests/`の分け方は一例。既存プロジェクトにテスト配置の規約がある場合は、その配置を維持してYAMLから参照する。対応範囲は単一Gitリポジトリの単一Vitestプロジェクト、または単一Cargoパッケージ。複数Vitestプロジェクト・複数Cargoパッケージworkspaceは対象外。Rustは[Rustのplan・verify](rust-runner.md)を参照する。

## Contract・Property・関係を分ける

一つの定義につき一つのYAMLファイルを基本とし、文書の種類ごとにディレクトリを分ける。ファイル名には定義のIDを使い、定義間の参照にもIDを用いる。ファイル名の変更が、別の概念を指すためのID変更にならないようにする。各ファイルは`schema_version: 2`と、一つの定義を格納する`definition`を持つ。型は配置先ディレクトリで決まる。拡張子は`.yaml`または`.yml`、各分類フォルダの直下だけをファイル名順で読み込む。サブフォルダ・シンボリックリンク・その他の拡張子はエラーになる。

| ディレクトリ | 記述する内容 |
| --- | --- |
| `concord/requirements/` | 製品の目的・要求 |
| `concord/contracts/` | 約束、適用範囲、前提条件、責任境界、版、承認状態 |
| `concord/properties/` | 成立すべき性質、版、承認状態、Property間の`depends_on`、共通実装・上位検証の責務 |
| `concord/relations/` | ContractとPropertyの多対多関係、適用条件、関係固有の実装と検証責務 |

例えば、C-001とC-002が同じP-001を要求する場合、P-001の文書は一つだけ作成する。REL-001はC-001とP-001を結び、REL-002はC-002とP-001を結ぶ。一つのContractが複数Propertyを要求する場合も、関係の文書を追加する。Contractの文書内へPropertyの定義を複製しない。関係の種類は`requires`または`contributes_to`を使う。

Contract固有の検証責務は対応する関係の文書へ、複数Contractの協調で成立する上位PropertyのIntegration/E2E検証責務は上位Propertyの文書へ置く。概念と検証の扱いは[多対多モデル](many-to-many.md)を参照する。[既存サンプルYAML](../examples/many-to-many.yaml)は現行CLIの単一ファイル形式であり、分割構成のサンプルではない。

## 分割定義の読込み

`concord/`配下の分類フォルダが存在する場合、CLIはそこにある分割定義を読み込む。全ファイルを読み込んだ後、IDの重複・参照先・Property依存の循環などをまとめて確認する。

```yaml
# concord/properties/P-001.yaml の形式例
schema_version: 2
definition:
  id: P-001
  version: "1"
  description: 成立すべき性質を記述する
  approval_status: proposed
  implementations: []
  depends_on: []
  obligations: []
```

既存の単一ファイル形式`concord/contracts.yaml`も引き続き読み込める。分割定義と単一ファイル形式・旧`guarantees.yaml`の混在はエラーにし、どちらかを暗黙に優先しない。YAMLのincludeには対応していない。

## パスとテスト名の記述

YAMLの`implementations`と`tests[].file`には、YAMLファイルの置き場所からではなく、導入先Gitリポジトリのルートからの相対パスを記述する。

`src/`配下には導入先リポジトリのコードを配置する。本書では具体的な実装ファイル名を指定しない。実装を作成した後、その実際のパスを`implementations`へ、対応するテストファイルの実際のパスを`tests[].file`へ記述する。ContractやPropertyと実装ファイルが一対一である必要はない。

`name`はVitestが収集するテストケースの完全名で、ネストしたsuite名を` > `で結ぶ。テストの実行単位はファイル。`tests/unit/`や`tests/integration/`というフォルダ名から検証手段を推測せず、`method`に明記する。

`docs/concord/model.md`は人間向けの補足文書で、CLIはそこから定義を読み込まない。また、補足文書の変更も、分類できない変更として全テスト実行へ広がる場合がある。実行に影響しないと確認した文書だけ、`concord.toml`の`non_runtime_paths`へ完全一致のパスで登録できる。ディレクトリ名やglobによる一括指定は行わない。

## 定義のチェックと実行結果

分割定義を使う場合は、導入先リポジトリのルートで実行する。

```sh
concord check concord --json
concord scan --json
concord plan --base main --json
concord verify --base main --json
```

`check`は文書の形式・参照を確認する。実装ファイルやテストケースの実在、Propertyの成立は確認しない。`scan`以降はGit、`concord.toml`と、設定したランナーに必要なVitest/Node.jsまたはCargo/rustcを必要とする。初回導入時は`concord verify --all --json`でも全体を確認する。

検証結果は`.concord/`へ保存し、定義YAMLへ成功・失敗を書き戻さない。同じPropertyに対する関係ごとの結果は、各runの`result.json`にある`property_results`で区別する。適用条件は記録するが、条件式の自動評価は行わない。

## Concord本体の開発リポジトリ

Concord自体を開発する場合は、次の既存構成を維持する。こちらの`examples/`は導入先向けの例であり、実行コマンドの自動読込み先ではない。

```text
Concord/
├── Cargo.toml
├── Cargo.lock
├── concord/                     # このリポジトリ自身の定義（Git管理対象）
│   ├── requirements/
│   ├── contracts/
│   ├── properties/
│   └── relations/
├── src/                         # Rust CLI・解析・計画・実行・文書チェック
├── tests/                       # CLIの回帰テスト
│   └── fixtures/vitest/         # 実Vitestを使うテスト対象プロジェクト
├── examples/
│   └── many-to-many.yaml        # ContractとPropertyの多対多定義例
├── docs/                        # 設計・利用方法・検証結果
│   └── folder-structure.md      # 本書
├── scripts/                     # 標準検証・依存監査
├── .concord/                    # 開発端末内の生成状態（Git管理外）
└── target/                      # Rustビルド成果物（Git管理外）
```

詳細な設定・終了コードは[利用ガイド](usage.md)を参照する。
