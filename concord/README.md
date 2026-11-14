# このリポジトリのConcord定義

- `requirements/`：このリポジトリの目的・要求
- `contracts/`：CLIが約束する振る舞いと条件・責任
- `properties/`：複数Contractが共有する性質
- `relations/`：ContractとPropertyの関係、対象実装、検証責務

リポジトリのルートで実行する。

```sh
mise exec -- cargo run --locked -- check concord --json
mise exec -- cargo run --locked -- plan --base HEAD --json
mise exec -- cargo run --locked -- verify --all --base HEAD --json
```

CLIによるplan・verifyの手順は[利用ガイド](../docs/usage.md)を参照する。Cargo/libtestでRustテストを収集・実行する。コンパイラ依存と確認済み実行時入力からtargetを選び、不明な変更は全targetへ拡大する。このリポジトリの自己検証ではverifyの終了コード0を要求する。新規・変更した定義の承認は再レビューで行う。

この`concord/`はGit管理対象。CLIのローカル生成状態を保存する`.concord/`はGit管理対象外。

Contract内の検証責務全体とPropertyのdepends_onから間接検証を選ぶ。P-VERIFICATION-WORKFLOWは下位Propertyの変更により、tests/commands.rsと書式・定義チェックの2コマンドを追加選択する。[任意コマンドと選択規則](../docs/verification-commands.md)を参照。

P-SELECTION-COMPLETENESSから検査処理自身のテストとselection-assuranceを選ぶ。検証手順は[リポジトリのREADME](../README.md)を参照。
