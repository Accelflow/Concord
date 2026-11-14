# Rustのplan・verify

Standalone 0.4.0から、単一Gitリポジトリ内の単一Cargoパッケージについて、Rustテストの収集・計画・実行と、Contract/Propertyの関係別結果に対応する。

## 設定と操作

対象リポジトリのルートに次の`concord.toml`を配置する。このConcordリポジトリにも同じ設定を配置済み。

```toml
schema_version = 1
non_runtime_paths = ["README.md"]
always_test_files = []

[runner]
kind = "cargo"
program = "cargo"
args = []
version = "1.97.1"
timeout_seconds = 600
```

`kind`の省略時は従来の`vitest`。CargoアダプターはCargoとrustcの両方が1.97.1であることを確認する。ランナーの追加引数は受け付けず、既存のCargo.lockを`--locked`で使う。timeoutは各外部プロセスへ適用する。

```sh
mise exec -- cargo run --locked -- check concord --json
mise exec -- cargo run --locked -- scan --json
mise exec -- cargo run --locked -- plan --base HEAD^ --json
mise exec -- cargo run --locked -- verify --base HEAD^ --json
mise exec -- cargo run --locked -- verify --all --base HEAD --json
```

`HEAD^`は直前commitとの比較例。実際の変更に応じて`main`や比較したいcommitを指定する。比較元が見つからない場合はエラーにし、自動fetchしない。変更がなく選択対象もなければnoopとなるため、全体を確認したい場合は`verify --all`を使う。

## 収集と実行の単位

Cargo metadataでリポジトリルートの単一パッケージであることを確認し、`cargo test --locked --tests --no-run --message-format=json`でテストをコンパイルする。成功したcompiler-artifactから実行ファイルとtargetのソースパスを取得し、各実行ファイルの`--list --format terse`でテスト名を収集する。収集自体もコンパイルやbuild script等を実行するため、信頼するcheckoutで使う。

`Test.file`はCargo targetのルートソースファイル。Integration Testでは通常`tests/<target>.rs`、ライブラリ内のUnit Testでは通常`src/lib.rs`となる。別モジュールに書かれたUnit Testも、所属するlib/bin targetのファイルとして扱う。`Test.name`にはlibtestが返す完全名を使い、モジュール区切りは`::`のまま記録する。

```yaml
# 関係のdefinition.obligations内に置く参照例
- id: O-RUST-API
  method: integration
  tests:
    - file: tests/api.rs
      name: api
```

実行単位はCargo test target（コンパイル済みテスト実行ファイル）で、選択したtarget内の全ケースを実行する。各targetはlibtestのpretty形式・色なし・1 threadで実行し、出力と最終集計の整合性を照合する。テスト実行ファイルは収集時のchecksumと一致することも確認する。

## Rustの変更影響選択

0.5.0では、Cargoが生成したcompiler-artifactに対応するrustcのdep-info（`.d`）を読み、コンパイラが実際に使用したRustモジュール・`#[path]`・`include_str!`/`include_bytes!`等のファイルからtargetへの依存関係を作る。cfg/feature/OSで有効なコンパイル入力を対象にし、独自のRust構文推測で依存を省略しない。

共有ライブラリはcrate単位で扱い、ライブラリを使うtest/bin targetへ保守的に依存を張る。ライブラリ内部の一つのモジュール変更でも、全ライブラリ利用先を選ぶ場合がある。`CARGO_BIN_EXE_<name>`を使うテストには、そのバイナリtargetへの依存を追加する。通常バイナリのdep-infoはコンパイル出力と公開実行ファイルの内容hashを照合して特定し、同じ内容に対応する候補が複数あれば依存を合併する。

### 実行時入力の確認

コンパイラのdep-infoは、テスト実行中の`read_to_string`・fixtureディレクトリのコピー・外部コマンド等が読むファイルを網羅しない。選択を有効にするには、各テストtargetが追加で読むリポジトリ内の入力をレビューし、`concord.toml`へ宣言する。

```toml
[rust.runtime_inputs]
"src/lib.rs" = []
"tests/api.rs" = ["fixtures/api/"]
"tests/other.rs" = []
```

キーは収集されるCargo targetのルートソースパス。値はリポジトリ相対のファイルパス、または末尾`/`のディレクトリ接頭辞。ディレクトリはGit管理・未追跡の索引対象ファイルへ展開し、新しいファイルの追加も選択へ反映する。`[]`は「追加のリポジトリ入力を読まないことを確認した」という宣言であり、自動解析の成功印ではない。テスト用に生成する一時プロジェクトのファイルは出力として扱う。

この宣言の完全性は作成者のレビューに依存する。コンパイル依存だけ、または実装との手動対応表だけで全利用先を網羅したとは主張しない。実行時のファイル読込みを追加・変更したときは、宣言も更新する。外部サービスや索引外の入力を使い、宣言で扱えない場合は、そのtargetの宣言を外して全実行へ戻す。

いずれかのテストtargetに宣言がなければ`rust_runtime_inputs_unreviewed`で全targetへ拡大する。不明なtarget名・安全でないパスはエラー。dep-infoの欠落/曖昧性・索引外のコンパイラ入力・不明なバイナリ依存・packageのbuild script・`.cargo/config(.toml)`の存在も、選択の根拠不足として全実行へ戻す。

### 変更からの選択

変更ファイルから依存グラフを逆に辿り、影響するtest targetを選ぶ。コンパイル入力と宣言した実行時入力の両方を使う。Contract/Propertyの関係で必要な検証責務も選択へ追加する。

Cargo.toml/Cargo.lock、Concord設定・定義、分類不能な変更、グラフに現れないRustソース、新規/削除等で根拠が不足する変更は全targetを選ぶ。グラフで追える新しい入力は対応するtargetを選べるが、未対応入力を黙って除外しない。`non_runtime_paths`に指定した非実行文書だけの変更では、runtime未確認の理由を適用せず、既知の依存もなければnoopになる。dep-info欠落等の他の不明理由や既知の利用先は維持する。

このリポジトリは、fixtureを読むtargetと、定義から実装・テストを検査するtargetの実行時入力を設定済み。`tests/definition_check.rs`の変更では、コンパイル上の利用先に加えて、実行時に同ファイルを読む`tests/split_definitions.rs`も選ぶ。二つのtargetの11ケースを選択し、全49ケースと結果を比較する標準検証を持つ。

## 結果と承認

`.concord/runs/<run-id>/cargo.json`に共通のケース結果と各Cargo targetの生の終了コード・集計を保存する。targetごとのstdout/stderrと、関係別の`result.json`も同じrunへ保存する。通常のテスト失敗に対するlibtestの終了コード101は、Concordの終了コード1へ正規化し、生の101はcargo.jsonへ残す。

ignoredは成功に加えず、必要な検証責務がignoredなら未検証とする。収集0件、結果の欠落・重複・未知ケース、集計不一致、途中終了、timeout、入力変更、実行ファイルの変更を成功扱いにしない。全ケースがignoredで実行済みケースが0件の場合も、成功にしない。

このリポジトリの自己検証では、終了コード0と全関係のverifiedを確認する。一般の未承認定義では、テストが成功しても承認不足によるunverified/終了コード4となる。適用条件の文章は記録するが自動評価しない。

## 対応範囲と標準検証

対応範囲は単一Cargoパッケージ・既定feature・ホスト上の標準libtestによるUnit/Integration Test。複数パッケージworkspace、path依存、`harness=false`、benchmarksは拒否する。doctest、examples/benchesのテスト、cross compilation、任意のfeature/target指定は対象に含めない。Cargoの`--tests`で収集する集合を、これらを含む全Rust検証と同一視しない。

公開ソースのビルドとテストは、ルートの[README](../README.md)を参照。

Cargoの成果物取得とtarget選択は[Cargo公式の外部ツール向け仕様](https://doc.rust-lang.org/cargo/reference/external-tools.html)と[cargo test仕様](https://doc.rust-lang.org/cargo/commands/cargo-test.html)、libtestの収集・出力は[Rust公式のテスト仕様](https://doc.rust-lang.org/rustc/tests/)に基づく。

コンパイラ入力の根拠は[Cargoのdep-info仕様](https://doc.rust-lang.org/cargo/reference/build-cache.html#dep-info-files)と[rustcのdep-info出力仕様](https://doc.rust-lang.org/rustc/command-line-arguments.html#--emit-specifies-the-types-of-output-files-to-generate)。dep-infoはコンパイル入力の情報であり、任意の実行時依存の証明ではない。
