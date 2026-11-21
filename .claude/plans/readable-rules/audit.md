# 読みやすさの監査と、直す順番

用語は [glossary.md](glossary.md)、公開関数は [api.md](api.md)、検査は [logic-check.md](logic-check.md)。数字は 2026-11-21 の `main`（`b19a57e`）で数えたもの。

## 1. 何が読みにくさを生んでいるか

### 1-1. コメントの参照先が無い（一番重い）

コメントは要件 ID（`R-…`、45 種）、判断（`D1`〜`D23`、20 種・延べ 214 回）、ずれ（`GAP-…`、20 種）を引いて理由を説明している。
その参照先の `research/` が失われていた（`.gitignore` されていて、git の履歴にも無い）。理由の半分が読めない状態だった。

- 要件 ID: 一次資料から `research/` を作り直した。ただし KI2・局面図・変化・手合割の配置は一次資料が無く、確信度は低い
- 判断と GAP: コメントから要旨だけ復元した（`research/95-decisions.md`、`research/90-gaps.md`）。決めた理由は失われたまま
- **このまま `.gitignore` にしておくと、また失われる**（確認事項1）

### 1-2. 同じ概念に複数の語

[glossary.md](glossary.md) にまとめた。重いものは次の5つ。

- 分岐が `branch`（243 回）と `fork`（119 回）
- 終局が `outcome`（143 回）と `special`
- 相対位置が `suffix`（77 回、`Suffix` 型もある）と `relative`（`Relative` 型）
- 移動元が `origin`（99 回）と `from`
- 先手・後手の軸と上手・下手の軸の混同（`jkf.rs` で先手を「下手」とも「上手」とも書いている。元は KIF の仕様書自身の食い違い）

### 1-3. コメントの粒度がファイルごとに違う

テスト以外の行で、コード1行あたりのコメント行（`///` を含む）:

| ファイル | 比 | 最長の塊 |
| --- | --- | --- |
| `parser.rs` | 1.35 | 21 行 |
| `parser/kakinoki.rs` | 1.33 | 28 行 |
| `notation.rs` | 0.74 | 35 行 |
| `parser/ki2.rs` | 0.74 | 38 行 |
| `normalizer.rs` | 0.51 | 48 行 |
| `converter/kif.rs` | 0.38 | 14 行 |
| `csa.rs` | 0.08 | 5 行 |
| `shogi_core/*.rs` | 0.04 | 3 行 |

偏りの元は、レビューの往復で直した箇所ほど説明が積み上がったこと。コメントに、過去に起きたことを語る文が残っている（例: `parser.rs:146`「79% of `bug_big.kif` went missing」、`parser.rs:173`、`jkf.rs:363`、`notation.rs:46`）。CLAUDE.md の「変更の経緯を書かない」に反する。全数はまだ数えていない

決める粒度（案）
- `///`: 何を返すか、エラーと panic の条件、守る規定の ID。1〜5行
- `//`: 仕様に根拠がある判断に ID を1つ。コードから読めない「なぜ」だけ。1〜3行
- 長い理由は `research/` に置き、コメントは ID で指す。10 行を超えるコメントは `research/` へ移す候補

### 1-4. 名前の形がそろっていない

- 真偽値を返す関数 22 個が4つの形（`a_*` 8個、`is_*` 3個、動詞で始まる 9個、名詞で始まる 2個）
- 書式ごとの語を返す関数が `kif_word` / `ki2_phrase` / `csa_word` / `move_word` / `board_word` / `relative_word`。`phrase` だけ違うのは KI2 が句を返すからで、それ自体は正しいが、説明が無い
- 略語 `pk2k`、`calculate_from`（何を計算するかが分からない）
- `handicap.rs` の `Handicap` 型と `HANDICAPS` 表が平手を含む
- `type WriteResult as Result` を書き手の各ファイルで `Result` に読み替えている。標準の `Result` と見分けられない
- エラーの名前: 同じ「`その他` に盤が無い」に `InitialBoardNoDataWithPresetOTHER` と `UnknownPreset` の2つ。メッセージの先頭が大文字（`Failed to normalize`）と小文字（`failed to normalize`）で揺れる

### 1-5. 部品の組み合わせが公開関数の約束と合っていない箇所

- 読む4関数は「正規化済みの JKF を返す」が、`relative` があるかは書式で違う（KIF だけ無い）。返り値の型から分からない
- 正規化が `bool` 2つの引数で振る舞いを変える（`normalize_with_options(true, false)`）。呼び出しを読んでも意味が分からない
- `normalize_with_options` に、文書に無い前処理がある（駒落ちで本譜の手番を全て反転する。`normalizer.rs:169-184`）
- CSA の読み手だけ「最後まで読めなければ `Err`」が守れていない（外部クレートの制約、GAP-012）
- `Suffix`（`normalizer.rs:633-643`）と `Relative` が別の型。違いは `Suffix` の doc に書いてある（「書かない」「1つ書く」「書けない」の3通り）が、名前からは分からない
- 3つの書き手の `# Errors` の文書がほぼ同じ文で3回書かれている

## 2. 見つけた不具合の候補

| 所見 | 重さ | 根拠 |
| --- | --- | --- |
| CSA で4種の終局が消える | 中 | logic-check 検査2 |
| KIF の反則勝ちで勝者が反転 | 中 | logic-check 検査2 |
| 反則手の変化から `(0,0)` が漏れる | 低 | logic-check 検査3 |
| 最新の clippy で検証が落ちる | 中 | 下記 |
| 使っていない依存 `shogi_official_kifu` | 低 | cargo の警告 |

- `verify.sh` は `main` のままで落ちる。cargo 1.100.0 の clippy が `needless_range_loop` を2箇所（`converter/csa.rs:128`、`converter/kakinoki.rs:90`）で出す。ツールチェーンを固定していない（`rust-toolchain.toml` が無い）ので、Rust の更新だけでゲートが壊れる。**今はコミットできない状態**
- 五枚落ちと左五枚落ちの配置は一次資料で確かめられない（`research/40-handicap.md`）

## 3. テストの部分実行

測った結果（`src/parser/kif.rs` を1つ触った後）:

| コマンド | 時間 |
| --- | --- |
| 1モジュールのテスト（17件） | 1.2 秒 |
| 全テスト（193件） | 1.5 秒 |
| clippy | 3.6 秒 |
| `verify.sh` 全体 | 4.6 秒 |

1モジュールのテストは `cargo test --lib -- parser::kif`、全テストは `cargo test --all-features`。

テストの実行自体は全件で 0.1 秒。時間の大半はコンパイルで、絞っても縮むのは 0.3 秒程度。今の規模では、部分実行で得られるのは時間より**出力の読みやすさ**。

- 部分実行はモジュールのパスで絞る: `cargo test --lib -- parser::kif`、`converter::`、`normalizer::`
- テスト名に規定の ID を入れておくと、規定で絞れる（例 `cargo test -- r_kif_006`）。今は ID を含むテスト名が少ない
- 重い検査（全合法手の往復、約3分）は `cargo test` に入れない。入れるなら `#[ignore]` を付け、`cargo test -- --ignored` で明示的に走らせる
- 冷えた状態からのビルドは約 12 秒（依存のコンパイル）。CI と初回だけ

## 4. 直す順番と進み具合

ブランチ `refactor/readable` で、1つずつ別のコミットにした。各コミットで `verify.sh` を通し、振る舞いを変えないものは検査プログラム（終局語の表・変化・全合法手の往復）の出力が変更前と一致することを確かめた。

済んだもの
1. ゲート: clippy の2箇所を段（rank）と筋で回す形に直した（D26）
2. 参照先: `research/` をコミットした
3. 用語: 真偽値の関数名、fork、special、initial position、先手・後手と上手・下手の取り違え
4. コメント: 過去に起きたことを語る文を、今の理由に書き直した
5. 正規化の引数を `Normalization` で名前つきにし、駒落ちの手番反転を文書に書いた
6. 書き手の `Result` の別名をやめた
7. 終局語の表のテストで、勝者が入れ替わる行（GAP-032）に注を付けた

残すもの（この PR の範囲外）
- 10 行を超えるコメントを `research/` へ移す。理由の置き場を移す作業で、文の書き直しが多く、用語をそろえた後に別の PR でする
- 公開の名前（`ConvertError::InitialBoardNoDataWithPresetOTHER` と `UnknownPreset` の重複、エラーの文言の大文字小文字）。consumer がタグで参照する公開の型で、変えると破壊的変更になる
- 不具合: GAP-032（直し方は D25）、GAP-033、GAP-012（`csa` クレート）
- 決定表のうち未着手のもの（手合割×書式、ヘッダ×書式、消費時間、コメントの置き場）
- 使っていない依存 `shogi_official_kifu`
