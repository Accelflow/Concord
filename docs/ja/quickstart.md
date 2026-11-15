# Concord クイックスタート

[日本語ドキュメント](README.md) · [English](../../QUICKSTART.md)

対象のOS・アーキテクチャ用にビルドしたConcordを使います。ソースからビルドする場合はRust 1.97.1を使い、Concordのルートで `cargo build --locked` を実行します。TypeScriptのデモにはGit、Node.js 24.22.1、Corepack経由のpnpm 10.34.8が必要です。

## 検索・詳細APIのデモ

[共通のデモ手順（英語）](../../examples/search-detail/README.md)を上から実行します。手順はベースラインをコミットし、同じベースラインから3種類のパッチを試します。各パッチは手順末尾の `git apply -R` で戻してから、次へ進みます。

| 変更 | 期待する結果 |
| --- | --- |
| 検索の実装だけを変更 | 検索テスト1件が選ばれ、成功する。全件実行では2件が成功する。 |
| 共通の閲覧権限判定に回帰を入れる | 検索と詳細が選ばれ、詳細だけが失敗する。`verify` の終了コードは1。失敗行の `cases[].test_id` を使って `concord explain` で詳細側のRelationと選定理由を確認する。 |
| 要件定義だけを変更 | `plan` が `mode: "all"` を返し、両テストが成功する。 |

## 自分のリポジトリで1要件を試す

1. 既存テストで確認できる要件を一つ選び、Requirement、Contract、Property、Relationを定義します。Relationに実装パスとテストの参照を置きます。[最小構成の例（英語）](../../skill/concord-cli/references/minimal-example.md)では、既存の検証コマンドを登録できます。
2. `concord.toml` に検証方法を設定します。ネイティブランナーは、単一プロジェクトのVitest 4.1.12または単一CargoパッケージのCargo/rustc 1.97.1に対応します。詳しくは[利用ガイド](usage.md)を参照してください。
3. 定義をベースラインとしてコミットします。実装を少し変更し、比較元のrefとテストIDを実際の値に置き換えて実行します。

```sh
base=YOUR_BASELINE_REF
concord check --json
concord plan --base "$base" --json
concord verify --base "$base" --json
concord explain YOUR_TEST_ID --json
```

`check` は定義と設定を検査し、`plan` は選定範囲と理由を示し、`verify` は選ばれたテスト・コマンドを実行します。`explain` は直近の保存済み計画からテストIDやコマンドIDの選定理由を表示します。

未選択のテストには今回の実行結果がありません。失敗は実行したテストが失敗を返した状態です。定義・共有設定の変更や、影響範囲を安全に絞れない場合は全件実行へ広がります。理由は `fallback_reasons` で確認できます。全件実行を指定するときは `concord verify --all --base "$base" --json` を使います。
