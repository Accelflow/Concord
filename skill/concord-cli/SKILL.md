---
name: concord-cli
description: Concordの導入、Contract・Property・Relationの定義、任意検証コマンドの登録、check・plan・verify・explainを扱う。既存リポジトリの検証をConcordで選択・実行するときや、基本操作を案内するときに使う。
---

# Concordの基本操作

Concordは、Contract・Propertyの関係と変更の影響から、必要なテストと登録コマンドを選択・実行するStandalone CLI。導入先の実装コードへのimportは不要。Gitと、実行する検証に必要な環境を使う。

## 導入先を確認する

- 対象Gitリポジトリ、変更内容、比較元のref、既存の検証コマンドを確認する。既存のテスト配置を尊重する。
- 使用するCLIの`concord --version`と`concord --help`を確認する。配布された実行ファイルなど、利用者が用意したCLIを使う。npmパッケージ・自動初期化・自動依存インストールの存在を推測しない。
- CLIをソースから用意する場合は、入手済みのConcordソースで、指定されたRust環境を使って`cargo build --release --locked`を実行し、その`target/release/concord`を使う。対象リポジトリでの実行時はCLIの絶対パスに置き換えられる。
- `scan`や`plan`もテストの収集・コンパイルを行い、対象コードを実行し得る。任意コマンドも含め、内容を確認したリポジトリで使う。

## 設定と定義を置く

```text
my-repository/
├── concord.toml             # 検証の実行設定
├── concord/                 # Git管理対象
│   ├── requirements/        # 目的・要求
│   ├── contracts/           # 条件・責任・約束
│   ├── properties/          # 成立を確認する性質
│   └── relations/           # ContractとPropertyの多対多関係・検証責務
└── .concord/                # Git管理対象外の実行状態・結果・ログ
```

ルートの`.gitignore`へ`/.concord/`を追加する。定義は各分類フォルダ直下に一件ずつ置き、各YAMLを`schema_version: 2`と`definition`で構成する。単一`concord/contracts.yaml`は互換形式であり、分割定義と混在させない。新規作成ではContract・Property・Relationを分ける。

定義を新規作成・変更するときは[最小構成の例](references/minimal-example.md)を読む。ContractとPropertyはIDで結び、共有PropertyをContractごとに複製しない。

機能追加・仕様変更・不具合修正では、依頼から確認可能な受入条件を整理する。各条件を対応するRequirement・Contract・Propertyに反映し、その条件を確かめるテストまたは検証コマンドを`obligations`で紐づける。既存テストが実際に条件を検証しているかを確認し、足りない条件のテストを追加する。

- `requires`：Contract固有の検証をRelationの`obligations`に置く。
- `contributes_to`：Relationの検証に加え、Property自身の独立したIntegration／E2E検証を要求する。
- 上位Propertyの`depends_on`へ下位PropertyのIDを置く。下位の変更から、間接的に必要になる上位の検証を選択する。
- Contract／Propertyの承認状態は内容と実装・検証の対応をレビューして決める。形式チェックやテスト成功だけで`approved`へ変更しない。

## テストと任意コマンドを登録する

任意コマンドは`concord.toml`の`[[commands]]`にID・`program`・`args`と、必要に応じて`files`・`input_patterns`・`timeout_seconds`を登録し、検証責務の`commands`からIDで参照する。コマンドだけで運用する場合は`[runner] kind = "commands"`。既存のVitest／Cargoランナーと併用する場合はその`[runner]`を使う。

`program`と`args`はリポジトリルートで実行する引数配列。シェル展開が必要な準備・実行・終了処理はリポジトリ内スクリプトにまとめる。`files`は実在するリポジトリ相対ファイルの完全一致。`input_patterns`は対応版で利用でき、`["e2e/recovery/", "tests/**/*.test.ts"]` のように末尾が `/` のディレクトリやglobを指定する。一致するファイルの追加・変更・削除が選択理由に残る。現在一致が0件のパターンも、将来の追加を検出できる。

共有ランナー、結果変換スクリプト、CLIのmanifestなどは、それらを使う各コマンドの入力に登録する。`input_patterns`はConcordの変更検知に使う。実行するテストファイルは各ランナーの収集設定で決まる。

専用のテスト収集・結果照合・コード依存解析を使う場合、実装で確認されている版はVitest 4.1.12とCargo/rustc 1.97.1。Vitestは単一プロジェクト、Cargoは単一パッケージ・既定feature・標準libtestのUnit／Integrationが対象。テスト参照は`scan`で得た`file`と`name`を使う。Rustの`file`はモジュール個別のファイルではなく、`src/lib.rs`や`tests/api.rs`などのtargetルートになる。実行時の追加入力は`[rust.runtime_inputs]`でtargetごとにレビューして宣言し、未確認の入力を空配列で隠さない。

コマンドがテストを実行し、責務を満たすためのケースが決まっている場合は、構造化報告に対応する版で `[commands.report]` の `required_cases` に安定したケースIDを宣言する。スクリプトはテストランナーの元の結果から、Concordが指定した `CONCORD_COMMAND_REPORT` へ次のJSONを書き出す。

```json
{"schema":"concord.command-report.v1","cases":[{"id":"document/save","status":"passed"}]}
```

終了コード0に加え、必須ケースが揃い、報告されたすべてのケースが成功していることを求める。設定から成功ケースを生成せず、実行結果から変換する。書式検査など終了コードで成否が決まるコマンドは、報告を宣言せず登録できる。

## チェック・選択・実行する

```sh
concord check --json
concord scan --json
concord plan --base main --json
concord verify --base main --json
# 変更がない場合も全体を実行したいとき
concord verify --all --base HEAD --json
# 保存済みplanのテストIDまたは登録コマンドID
concord explain document-tests --json
```

`main`は比較例。存在するrefを目的に合わせて指定する。Concordは比較元を自動fetchしない。計画IDで実行を固定できる版では、同じソース・環境で `plan --base REF --json` の `plan_id` を取得し、`verify --base REF --expect-plan ID --json` へ渡す。全体検証では両方に `--all` を付ける。再計画したIDが異なれば検証の実行前に停止する。利用中のCLIの `plan --help` と `verify --help` で対応を確認する。旧版では `verify --all` で全体を実行し、run内のplanを確認する。

設定照合に対応する版では、リポジトリルートの `concord check --json` がYAML定義に加え、`concord.toml` のコマンド参照、完全一致の入力ファイル、パターンの書式を照合する。`configuration_checked: true` で設定も検査したことを確認する。`concord check concord --json` はYAMLのみを検査する。両方のパスを明示する場合は `concord check concord --config concord.toml --json` を使う。実装・テストの対応と実行結果は `scan`・`plan`・`verify` で確認する。

変更に関係するRelationを起点に、同じContractの全Relationと、Propertyの`depends_on`を逆方向にたどった上位の検証を選ぶ。同じPropertyを共有するだけの別Contractには自動で広げない。Property共通の`implementations`の変更は、そのPropertyを使う全Relationの対象になる。不明な変更や解析の不確実性、定義・共有設定の変更では全検証へ広げる。

選択設定を導入・変更したときは、影響範囲の異なる代表的な実装・テストの差分ごとに、必要なテストやコマンドを先に特定し、`concord plan`の選択結果と照合する。必要な検証が漏れず、無関係な検証が選ばれないことを確認する。差があれば選択理由を調べ、実装・テストのパス、依存関係、登録コマンドの粒度を見直す。全体検証への切り替えが仕様上必要な差分では、その理由を確認する。

## 結果を読む

- planの`mode`、`selected_files`、`selected_commands`、`affected_relations`、`fallback_reasons`と選択理由を確認する。
- runの終了コードに加え、`status`、`input_unchanged`、`result_integrity`、`command_results`、`obligation_results`、関係ごとの`property_results`、`plan.property_gaps`を確認する。
- `noop`の終了コード0は検証実行の成功件数ではない。必要な検証が未実行なら完了扱いにしない。
- 終了コードは0＝選択範囲成功またはnoop、1＝テスト／コマンド失敗、2＝設定・収集等のエラー、3＝中断・タイムアウト・結果不整合・入力変化、4＝Propertyの検証不足。
- 構造化報告を宣言したコマンドでは、終了コードと `command_results[].report` のケースID・成否・件数を確認する。`obligation_results[].command_ids` で責務に対応するコマンドをたどる。トップレベルのケースと件数はネイティブランナーの結果。
- 構造化報告を宣言しないコマンドは終了コードで判定する。必要件数やskipの検査が必要で、構造化報告に対応しない版を使う場合は登録スクリプトで検査し、条件不足なら非0を返す。
- 自然言語の`applicability.condition`は記録する前提であり、自動評価して条件成立を証明するものではない。

結果は`.concord/runs/<run-id>/`に保存する。結果を共有するときは対象revision・比較元・実行範囲・終了コード・未実行範囲を添え、ログの内容を確認する。過去の成功を変更後の入力に転記しない。
