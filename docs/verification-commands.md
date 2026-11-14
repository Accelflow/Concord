# Contract・Propertyからの検証選択と任意コマンド

コードの依存解析は選択の入口の一つである。Contract・Property・Relationに登録した関係から、コードのimportでは到達しない統合テストやE2Eも選択する。

## 選択の規則

1. 変更ファイルと、そのファイルを直接・間接に利用するファイルを求める。
2. Property／Relationの`implementations`、検証のテストファイル、検証コマンドの`files`・`input_patterns`との対応からRelationを選択する。
3. 選択したRelationと同じContractに属するすべてのRelationを選ぶ。同一Contractの別Propertyの検証も含む。
4. 選ばれたPropertyを`depends_on`に挙げる上位PropertyのRelationを選ぶ。そのContract全体にも広げ、変化がなくなるまで繰り返す。
5. 選んだRelationの検証責務を実行する。`contributes_to`ではProperty自身の統合／E2E責務も加える。

同じPropertyを共有するだけの別Contractには自動で広げない。別Contractの振る舞いも影響を受けるなら、上位Propertyの`depends_on`で関係を明示する。Propertyの`implementations`を変更した場合は、そのPropertyを使う全Relationが直接の対象になる。旧`property_ids`もRelationに変換したうえで同じ規則を適用する。

コマンドモードでは未変更のtsconfig・Vite/Vitest設定・環境ファイルの存在だけを理由に全実行へ戻さない。これらの設定ファイル自体、package/Cargo manifest、lockfile、pnpm workspace、concord.toml、concord/定義の変更では全登録コマンドへ広げる。この共有設定変更はfilesやimplementationsへの登録、non_runtime_pathsの指定でも除外しない。

コマンドモードにはコード依存グラフがないため、宣言されていないTS/JSを含む未知入力は、ほかに実行対象が見つかっていても全実行へ戻す。non_runtime_pathsはレビュー済みの非実行入力だけを完全一致で指定する。ネイティブランナーでは引き続き設定の存在による保守的なfallbackを維持する。未知入力・依存解析の不確実性・定義／設定の変更では全テストと全登録コマンドに広げる。適用条件の自然言語`applicability.condition`は記録する前提であり、条件式として自動評価して検証を除外しない。

## コマンドの登録

`concord.toml`で実行内容を登録する。Vitest／Cargoの設定と併用できる。コードの依存解析や既存ランナーを使わず、登録だけで運用する場合は`runner.kind = "commands"`を指定する。

```toml
schema_version = 1

[runner]
kind = "commands"

[[commands]]
id = "document-recovery-e2e"
program = "pnpm"
args = ["exec", "playwright", "test", "e2e/document-recovery.spec.ts"]
files = ["e2e/document-recovery.spec.ts"]
timeout_seconds = 300
```

`program`と`args`はリポジトリのルートを作業ディレクトリとしてそのまま実行する。暗黙のシェル展開は行わない。必要なサービス起動・終了処理や複数コマンドは、リポジトリ内のスクリプトでまとめて登録できる。`files`には実在するリポジトリ相対ファイルを完全一致で指定する。

`input_patterns = ["e2e/recovery/", "tests/**/*.test.ts"]` を追加すると、対象範囲のファイルを変更・追加・削除した場合もコマンドを選択する。末尾が `/` の指定はそのディレクトリ配下全体に一致する。globの `*` と `?` はそれぞれ同じ階層内の任意の文字列と1文字に一致し、独立した `**/` はゼロ階層以上に一致する。末尾の `/**` は配下のファイルに一致する。現在一致するファイルがない指定も有効で、将来の追加を検出できる。`files` と `input_patterns` は併用でき、どちらも省略したコマンドはContract・Propertyの関係に応じて選択する。選択理由には `command_input_pattern:tests/**/*.test.ts:tests/recovery.test.ts` のように指定とファイルを記録する。

Relationまたは合成Propertyの`obligations`からIDで参照する。

```yaml
obligations:
  - id: O-DOCUMENT-RECOVERY
    method: e2e
    commands: [document-recovery-e2e]
```

同じ責務に`tests`と`commands`の両方を登録でき、その場合は両方の成功を要求する。複数の責務から参照されたコマンドは1回だけ実行する。

リポジトリのルートで `concord check` を実行すると、YAMLの形式・関係と、`concord.toml` がある場合はコマンドID、完全一致の入力ファイル、パターンの書式を照合する。テスト収集・実行や状態の書き込みは行わない。`concord check concord` はYAMLだけを検査する。定義と設定を明示する場合は `concord check concord --config concord.toml` を使う。入力パスは設定ファイルの親ディレクトリから解決し、`scan`／`plan` では完全一致の入力がGitの変更検知対象に含まれることも確認する。

## plan・verifyと結果

`plan`には`selected_commands`と、理由を付けた`command_decisions`を出力する。`concord explain <command-id>`で保存した選択理由を確認できる。`verify`はテストに加えて選択したコマンドを登録順に直列実行し、`command_results`に終了コード・状態・経過時間・標準出力／標準エラーのログパスを保存する。ログはGit管理対象外の`.concord/runs/`に置く。

- `report`を宣言しないコマンドは、終了コード0で成功とする。
- 非0は失敗、起動不能・signal終了・タイムアウト・中断は未検証として非0で終了する。Unixではタイムアウト／中断時に子プロセスグループを停止する。
- 実行前後でリポジトリ入力が変わった場合はstaleとして非0で終了する。
- 存在しないコマンド参照・不正な登録は実行前に拒否する。

## 任意コマンドのケース結果を提出する

コマンドの実行結果をケース単位で検証する場合は、登録時に必須ケースのIDを宣言する。IDはテストの完全名など、実行ごとに変わらない名前を使う。

```toml
[[commands]]
id = "document-recovery-e2e"
program = "python3"
args = ["scripts/verify-recovery.py"]

[commands.report]
required_cases = ["document/save", "document/recover"]
```

スクリプトはテストランナーの元の結果から実行ケースを取り出し、Concordが毎回指定する `CONCORD_COMMAND_REPORT` のファイルに次のJSONを書き込む。

```json
{
  "schema": "concord.command-report.v1",
  "cases": [
    {"id": "document/save", "status": "passed"},
    {"id": "document/recover", "status": "passed"}
  ]
}
```

- ケースの状態は `passed`、`failed`、`skipped`。IDはコマンド内で一意とする。登録した必須IDはすべて報告する。追加の実行ケースも同じ形式で報告できる。
- 終了コード0、必須ケースの存在、報告された全ケースの成功を要求する。必須ケース欠落・重複・欠損・不正な報告は結果不整合（終了コード3）とする。整合する報告に失敗やskipがある場合、またはコマンドが非0で終了した場合は成功にならない。
- `command_results[].report`にケース、件数、元JSONのSHA-256を保存する。`obligation_results[].command_ids`から対応するコマンドを参照できる。複数の責務が同じコマンドを参照する場合も、実行と報告は一度だけ行う。
- トップレベルの `cases` と件数はネイティブランナーの結果。任意コマンドの件数は `command_results` 内に保持する。報告ファイルは通常ファイルで上限8 MiB。

スクリプトがVitest・Playwright等の結果を変換する場合は、実際の実行結果を使う。設定の必須IDをそのまま成功ケースとして書き出すと、未実行を検出できなくなる。

## 計画IDを指定して実行する

同じ環境・ソース・比較元で `plan --base REF --json` の `plan_id` を取得し、`verify --base REF --expect-plan ID --json` に渡す。全体実行では両方に `--all` を付ける。再計画したIDが一致しなければテストと登録コマンドの実行前に終了する。計画には環境のfingerprintが含まれるため、環境が変わる場合は実行環境で計画を作る。

## このリポジトリでの利用

`P-VERIFICATION-WORKFLOW`は定義整合性・変更選択・実行結果などの下位Propertyに依存する。下位の変更から`C-WORKFLOW`の間接検証へ伝播し、`tests/commands.rs`と、書式、定義、選択検証に関するコマンドを選択する。コマンドから`verify`自身は再帰実行しない。

`scripts/verify-self.py`は、実際のテストソース変更について選択plan／verifyを行い、そのあと全体verifyと結果を照合する。選択対象、間接Property、コマンド成功、全Relationの検証済み、終了コード0を必須にする。
