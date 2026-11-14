# ContractとPropertyの多対多関係・検証モデル

This document describes the many-to-many verification model used by Concord.

以前も同じProperty IDを複数Contractのproperty_idsへ書けたため、参照関係は多対多だった。ただし、Property単位で検証結果を一つにまとめていた。今回、関係の種類・適用条件・検証責務と、その関係ごとの結果を独立したデータへ改める。

## 第一級の関係

ContractとPropertyは独立した定義とし、relationで結ぶ。0.3.1からは分類ディレクトリへ一つずつ配置できる。[推奨構成](folder-structure.md)を参照する。既存の単一ファイル形式では独立した配列をルートのrelations配列で結ぶ。[記述例](../examples/many-to-many.yaml)は、二つのContractが同じPropertyをそれぞれ要求する形式である。

```yaml
relations:
  - id: REL-SEARCH-PRIVATE
    contract_id: C-SEARCH
    property_id: P-PRIVATE
    relation: requires
    applicability:
      condition: 検索リクエストのユーザーが対象商品の閲覧権限を持たない
    implementations: [src/search.ts]
    obligations:
      - id: O-SEARCH-PRIVATE
        method: api
        tests:
          - file: tests/search.test.ts
            name: search > hides private data
```

| 関係 | 意味 | 責務の置き方 |
| --- | --- | --- |
| Contract requires Property | 各Contractが独立にPropertyを満たす | relationのobligationsにContract固有の検証を定義 |
| Contract contributes_to Property | Contractが上位Propertyの成立へ寄与する | relation自身の責務に加え、上位Property自身のintegration/e2e責務を定義 |
| Property depends_on Property | 上位Propertyが下位Propertyに依存する | 上位Propertyのdepends_onへ下位IDを列挙 |

一つのContractから複数Propertyへも、一つのPropertyから複数Contractへも結べる。relation IDは一意で、同じProperty IDを複製しない。Contract固有の実装パスはrelation.implementationsに置く。Property.implementationsは共通実装の変更で全適用先へ影響させる場合に使う。

新しい明示的relations形式ではProperty.versionを指定する。Contract.property_idsは互換読込み用で、明示的relationsと混在させない。requiresの検証責務はrelationに置き、Property.obligationsへ置いたまま無視される定義は入力エラーにする。

## 独立した要求の結果

同じP-PRIVATEでも、検索は成功、詳細は失敗という結果を記録できる。各property_results行はPropertyノードの状態ではなく、一つのrelationに対する結果である。

```text
REL-SEARCH-PRIVATE / C-SEARCH / P-PRIVATE → passed / verified
REL-DETAIL-PRIVATE / C-DETAIL / P-PRIVATE → failed / failed
```

一方のContractの失敗・未承認だけで、他方のContractの結果を書き換えない。ただしrunner結果の件数・集合・終了状態が整合しない場合や入力が変化した場合は、各関係の結果をverifiedにしない。

各行はrelation_id、contract_id、property_id、関係種類、applicability、Contract/Propertyの版、Contract.scope、commit SHA、run ID、環境fingerprint、入力fingerprintを持つ。resultはpassed / failed / inconclusive、statusはverified / failed / unverified / staleを保持する。obligation_resultsとrequired_obligationsもrelation_idで区別する。

applicability.conditionは人間が記述する適用条件であり、現版は条件式を実行しない。applicability_evaluatedはfalseで、passedはその記載条件に対する指定テストの結果を意味する。任意の条件式、AND/ORの組合せ規則、条件が成立したことの自動証明は後続の設計対象。

## 協調と上位Property

変更に関わるRelationを選んだ後は、そのContractのすべてのRelationへ検証を広げる。そこで選ばれたPropertyから上位依存へ伝播し、Contract内展開と合わせて変化がなくなるまで繰り返す。コードの参照がない検証も対象になる。

上位Propertyはdepends_onで下位Propertyを参照する。下位実装・関係の変更を検出すると、依存の逆方向を上位へ辿り、その上位の関係と検証責務を選ぶ。循環・未知の依存先はcheck/scanでエラーにする。

contributes_toの各ContractのUnit Testだけが成功しても、上位Propertyをverifiedにしない。上位Property.obligationsに、連携全体を確認する独立したintegration/e2e責務が必要。空欄・未実施・skip・失敗は区別する。下位Propertyの成功を上位の成功へ自動転記しない。

## 互換性と保存

定義YAMLはschema_version: 2を継続し、独立したrelationsとPropertyのversion/depends_onを追加した。以前のContract.property_ids形式は、メモリー上でcompat:<Contract IDとProperty IDのJSON配列のSHA-256>のrequires関係へ正規化し、旧IDと既存の検証責務を保持する。旧形式でProperty.versionが省略された場合は互換規則として1を用い、元の版を復元したという意味にはしない。旧YAMLを自動変更しない。

結果の粒度が変わるため、scan/plan/run/check/explainの結果JSONはschema_version: 3。SQLiteの旧v2レコードは変更せず、新しい結果はsnapshot:v3 / plan:v3 / run:v3へ保存する。過去のProperty単位の結果を、関係ごとの成功へ転記しない。

Contract全体の責務とProperty依存による間接検証、および任意コマンド登録の現行仕様は[検証コマンド](verification-commands.md)を参照。
