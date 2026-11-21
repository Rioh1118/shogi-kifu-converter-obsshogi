# 設計: モジュールの責務と公開 API の整理

## 目的

ファイル・型・trait の置き場を「1つのモジュールに1つの変わる理由」にそろえ、依存の向きを一方向にする。公開 API は、consumer（obs-shogi）が書式ごとに書いている振り分けを、クレート側の1か所に持てる形にする。
用語は `.claude/plans/readable-rules/glossary.md`、仕様は `research/`。

## 今の形と問題（現物で数えた）

依存（テスト以外の `crate::` 参照）:

```
jkf ──▶ notation ──▶ jkf          （データの型が書式の語を知っている。循環）
error ──▶ notation, jkf
normalizer ──▶ handicap, notation, jkf, error
converter/ki2 ──▶ normalizer      （書き手が正規化の内部 infer_relative_from_position を呼ぶ）
csa.rs ──▶ normalizer             （CSA の読み手が ORIGIN_UNSTATED を使う）
parser/ki2 ──▶ normalizer
```

| 問題 | 根拠 | 原則 |
| --- | --- | --- |
| データの型に書式の語 | `jkf.rs` の `kif_word` など5つ | 責務の分離・依存の向き |
| 正規化が5つの仕事 | 下の表 | 責務の分離 |
| 書き手が正規化に依存 | `converter/ki2.rs:202` | 依存の向き |
| CSA の読み手が外 | `src/csa.rs` が `parser/` の外 | 置き場の一貫性 |
| `parser.rs` が3つの仕事 | 文字コード・BOM・振り分け | 責務の分離 |
| テストが本体の3倍 | `parser/kakinoki.rs` | 読みやすさ |

`normalizer.rs`（テスト以外 995 行）の中身:

- 7-97 行: 駒・持ち駒の操作（`Kind::promoted`、`Hand::increment`）。本来はデータの型（`jkf`）
- 98 行: 移動元の印 `ORIGIN_UNSTATED`。読み手と正規化の取り決め
- 117-175 行: 正規化の選択（`Normalization`）
- 176-372 行と 818-995 行: 公開の正規化 API と手の走査（`Board`）
- 375-817 行: 相対位置の規則（R-NOT-004）の両方向。本来は表記の規則（`notation`）

`parser/kakinoki.rs` は本体 838 行に対しテストが 2,643 行ある。

相対位置の規則は、読む向き（`calculate_from`: 相対位置から移動元を決める）と書く向き（`infer_relative_from_position`: 移動元から相対位置を決める）が `candidates_reaching`・`by_motion`・`by_file`・`suffix_for` を共有している。KI2 の書き手が正規化を呼ぶのはこのため。

## 目標の形

依存は上から下へだけ向く。

```
層0 データ      jkf（型と、駒・持ち駒の純粋な操作）, error
層1 規則        notation（駒の語・数字・空白・終局語の表・相対位置の規則）
                handicap（手合割の表）, shogi_core（型の変換）
層2 正規化      normalizer（盤で手順を追って埋める）
層3 書式        parser/{kif, ki2, kakinoki, csa, jkf, text}
                converter/{kif, ki2, kakinoki, csa, usi}
```

### ファイル

- `notation.rs` → `notation/mod.rs`。下の2つを子に持つため
- `jkf.rs` の終局語の5メソッド → `notation/special.rs`。データの型から書式の語を外す
- `normalizer.rs` の相対位置の規則 → `notation/relative.rs`。読み手・書き手・正規化が共有する表記の規則
- `normalizer.rs` の駒・持ち駒の操作 → `jkf.rs`。データの型の操作
- `csa.rs` → `parser/csa.rs`。CSA の読み手を読み手の並びへ
- `parser.rs` の文字コード・BOM → `parser/text.rs`。全書式が共有するテキストの決め方
- 長いテスト → `<モジュール>/tests.rs`。本体を読む人がテストを飛ばせる。部分実行のパス（`parser::kif::tests::…`）は変わらない

テストを外へ出すのは、テストが 500 行を超える5ファイル（`parser/kakinoki`、`normalizer`、`parser/ki2`、`parser/kif`、`converter/ki2`）だけ。短いものは本体の隣が読みやすい。

### 公開 API

consumer が呼んでいるもの（`~/src/github.com/Rioh1118/obs-shogi/src-tauri/src` を grep）: `parse_{kif,ki2,csa,jkf}_str`、`parse_{kif,ki2,jkf}_file`、`try_to_{kif,ki2,csa}_owned`、`normalize`、`jkf` の型、`ParseError` の全バリアント（`search/read/diagnosis.rs` が網羅的に match）。

- **足す**: `Format` enum と、`parser::parse_str(format, s)`・`JsonKifuFormat::try_to_string_in(format)`。consumer の拡張子ごとの振り分け（`workspace/record.rs:89-91` など）を1か所に持てる。書式が増えても呼び手は変わらない
- **非推奨にする**: `normalize_with_options`・`normalize_with_color_correction`。真偽値の引数で、consumer は使っていない。`normalize` と `populate_relative` で足りる
- **残す**: 既存の `parse_*`・`ToKif` など。consumer が使っている

この PR でしないこと（破壊的変更。consumer と同時に上げる必要がある）:

- `to_*_owned`（非推奨）の削除
- `ConvertError` の重複バリアント（`InitialBoardNoDataWithPresetOTHER` と `UnknownPreset`）の統合
- `ParseError` の形の変更（consumer が網羅的に match している）
- 移動元の印 `(0,0)` を JKF に載せない内部表現（GAP-033 の根本。別の設計が要る）
- consumer が持つ文字コードの追加判定（EUC-JP・UTF-16・ISO-2022-JP、`search/read/encoding.rs`）と CSA の補修（`search/read/csa.rs`）をクレートへ移すこと。consumer 側の変更が要る

## 共通化の判断

- 相対位置の規則の読む向きと書く向き → 1モジュールにまとめる。同じ規則の写しで、片方を変えると必ずもう片方も変わる
- 3つの書き手 trait（`ToKif`・`ToKi2`・`ToCsa`） → 別のまま。公開 API で、統合すると consumer の `use` が壊れる。振り分けだけを `try_to_string_in` で1か所に
- 書き手3つの `# Errors` の文書 → 別のまま。trait ごとに rustdoc に出て、書式ごとの差（KI2 の `UnspellableMove`）がある

## 壊しうるもの

- 移動だけの変更でも、`pub(crate)` の可視性を誤ると別のモジュールから呼べなくなる → コンパイルで止まる
- テストを外へ出すとき、`use super::*` の参照先が変わる → コンパイルで止まる。テスト件数 193 が変わらないことを確かめる
- 新しい公開関数: 書式と拡張子の対応の誤り → テストで全書式の往復を確かめる

## テストを掛ける所

- 振る舞いを変えない変更: `cargo test` の件数 193 と、`.claude/plans/readable-rules/checks/` の3検査の出力が変更前と一致すること
- `Format` と `parse_str`・`to_string_in`: 全書式で既存の関数と同じ結果になるテスト

## 判断ログ

- 書式ごとのディレクトリ（`kif/{read,write}`）ではなく、今の読み手・書き手の分け方（`parser/`・`converter/`）を保つ。公開モジュールの名前で、変えると consumer の `use` が全て壊れる。上流とも同じ形
- 終局語の表と相対位置の規則は `notation` に置く。読み手と書き手の両方が使う「どう書くか」の規則で、データの型でも正規化でもない
- 移動元の印 `ORIGIN_UNSTATED` は、この PR では `normalizer` に残す。読み手と正規化の取り決めで、置き場を変えるより内部表現を変える（申し送り）方が筋
- 公開 API は足すだけにする。破壊的変更はタグを指す consumer の同時更新が要り、この整理と混ぜると戻しにくい

## 申し送り

- 破壊的変更の5つ（上の「この PR でしないこと」）。次の minor（0.7.0）で consumer と同時に
- `error.rs` が `notation` の表示用の型（`Coordinate`・`MoveText`）を使う。層0が層1を読む向きだが、表示だけなので残す
