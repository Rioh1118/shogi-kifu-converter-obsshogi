# レビュー ラウンド12 — issue #6 / #7 / #8 / #9

対象: `fix/silent-failures-and-normalize-promote`（`git diff main...HEAD -- src/`）。
重点はラウンド11で入った修正（`be43ce3` / `1e4d994` / `ccb5501` / `285c4c1` / `55c217b`）。

観点5本（rust / spec / robustness / comment / architecture）を並列で実行。

- 重複を潰した結果: **BLOCK 2 / HIGH 1 / MEDIUM 12**
- **このラウンドは退行を2件出した。** どちらもラウンド11 の修正が持ち込んだもので、
  `main` が正しく扱えていた入力が壊れている。**reviewer 5本のうち4本が独立に
  R12-01 を見つけた**
- 15件を修正。コミット `ea3a257` / `7820662` / `4bc8244` / `1dabde7`

## 所見

### R12-01 [BLOCK] KI2 の盤面図の守りが1つも効いていない。図が1文字崩れると局面ごと黙って平手になる（**退行**）

- 場所: `src/parser/ki2.rs:306-308`（`move_run` の中の読み飛ばしが `PastTheOpeningBlock` を定数で持つ）、
  迂回される守りは `:457`、判定は `src/parser/kakinoki.rs:669`
- 指摘: rust（BLOCK）/ spec（BLOCK）/ robustness（BLOCK）/ comment（BLOCK）— **4本が独立に**
- `moves` の前置きループは `WhereABoardCouldStill` で `|` `+` を拒むが、そこで break した行は
  そのまま `move_run` に渡り、`move_run` は**本譜の1手目より前でも** `PastTheOpeningBlock` を使う
- 実測（筋見出しか上の枠線を1文字壊した記録。`main` = `dbc436a` / `r10` = `c6bcb44`）:

  | 入力 | main | r10 | R11 終了時 |
  | --- | --- | --- | --- |
  | `.kif` | Err | Err | Err |
  | **`.ki2` 同内容** | **Err** | **Err** | **`Ok`・`data=None` / `preset=PresetHirate`** |

  コーパス609件を `to_ki2` して同じ壊れた図を注入した集計: silent が `main` 54 / r10 **0** /
  R11 終了時 **54**。R10-03 が塞いだ穴が KI2 についてだけ再び開いていた
- obs-shogi は保存のたび `to_ki2_owned` を通す（R-REQ-002）ので、**詰将棋・駒落ち・任意局面の
  `.ki2` を開いて保存すると原本が平手で上書きされる**。D4 が最優先で潰した経路
- **結果**: 直した（`ea3a257`）: `move_run` に `where_it_starts` を渡し、その run がまだ何も
  読んでいない間だけそれを使う。`a_board_is_read_or_reported_but_never_skipped` に D18 の腕を
  足した。**変異を当てて落ちることを確認**

### R12-02 [BLOCK] 走りの先頭の終局行が、消費時間ごと黙って消える（**退行**）

- 場所: `src/parser/kakinoki.rs:415-436`（`a_move_follows_the_number` の終局語の腕）
- 指摘: architecture（BLOCK）/ robustness（HIGH）
- 述語が `pair(padding, eof)` を要求する一方、消費する `kif::move_line` は
  `opt(move_time)` を許す。**数える集合が読める集合より狭い**
- 実測（`手数---` の直下 = 走りの先頭）:

  | 入力 | main | R11 終了時 |
  | --- | --- | --- |
  | `1投了 ( 0:03/00:00:03)` | Ok 投了 | **Ok 0手・結末なし** |
  | `1中断 ( 0:00/00:00:00)` | Ok 中断 | **Ok 0手** |
  | `1千日手 ( 0:00/00:00:00)` | Ok 千日手 | **Ok 0手** |
  | `変化：2手` + `2投了 ( 0:03/…)` | Ok 分岐1 | **Err**（下に行はあるのに） |
  | `.ki2` の `3投了 ( 0:03/…)` | Err | **Ok・その行が消える** |

  コーパス609件へ `3投了 ( 0:03/00:00:03)` を注入すると **608ファイルが `main` より手数・
  終局・分岐を失う**
- **結果**: 直した（`7820662`）: `move_time` を消費側の隣から共有側へ移し、終局語の腕が
  同じ形を引く。`an_outcome_at_the_head_of_a_run_keeps_its_clock` で6綴り + 分岐 + KI2 を固定

### R12-03 [HIGH] 手数の直後に余白が無い誤った綴りの指し手行が、黙って消える（**退行**）

- 場所: 同上（`:424-433` の移動元の腕）
- 指摘: spec（MEDIUM）/ robustness（MEDIUM）
- **同じ内容が字下げの有無だけで loud と silent に分かれていた**:

  | 入力 | main | R11 終了時 |
  | --- | --- | --- |
  | `   3 ２二角不成(88)`（余白あり） | Err | Err |
  | **`3２二角不成(88)`（余白なし）** | **Err** | **Ok・その手が消える** |
  | 同・`.ki2` | Err | **Ok・消える** |

  コーパス注入（17,793点）で **`main` `Err` → HEAD `Ok` が 17,157点**
- **結果**: 直した（`7820662`）: 駒種の後ろの `成` / `不成` も指し手の形と数える。
  R-KIF-006 は KIF が `不成` を書かないと決めているが、**読み手が「正しい書き手が出すもの」
  だけを数えると他のソフトが書いたものを黙って落とす**。`2同銀と取れば` は駒種で終わるので
  注記のまま。`a_move_line_written_the_wrong_way_is_reported_not_dropped` で固定

### R12-04 [MEDIUM] `ends_a_word` の呼び手は1つしかない。R11-14 の「1本にした」が事実でない

- 場所: `src/parser/kakinoki.rs:200`（定義）、`src/parser/ki2.rs:217-219`（手書きの写し）
- 指摘: spec / comment / architecture — 3本が独立に
- `grep -rn "ends_a_word" src/` は定義と呼び出し1件のみ。`ccb5501` は `ki2.rs` の import 行しか
  触っていない。doc は「2つはずれるので1つの関数にする」と名乗っている
- **結果**: 直した（`4bc8244`）: `ki2.rs` が `ends_a_word` を呼ぶ。報告書 R11-14 の結果欄も直した
  （**報告書の虚偽の記述はこれで5件目**）

### R12-05 [MEDIUM] doc が持ち主から離れた2件（付き替わり **9回目 / 10回目**）

- 場所: `src/parser/kakinoki.rs:883-901`（`information_line_keyvalue` の17行が挿入された
  `const` に付き、関数は無 doc）、`src/parser/kif.rs:140-142`（移動した関数の見出し2行が
  `move_move` に残った）
- 指摘: rust / comment
- どちらも `ccb5501` が入れた。`cargo doc -D warnings` は素通しする
- **結果**: 直した（`4bc8244`）

### R12-06 [MEDIUM] `COLONS` の doc が「1つの集合」と名乗るのに、キーの規則は独立したリテラルを2本持つ

- 場所: `src/parser/kakinoki.rs:140-146` と `:901-903`
- 指摘: rust / comment
- 実測: `COLONS` に3つ目のコロン（`﹕`）を足しても `棋戦﹕竜王戦` は消えたまま。
  **doc が「起きない」と書いている故障がそのまま起きる**。R11-15 は置き場を1箇所にしただけ
- **結果**: 直した（`4bc8244`）: リテラルを述語に置き換え、`COLONS` と `LINE_ENDS` から導出する

### R12-07 [MEDIUM] 空の持駒行だけが改行を要求し続けていた（R11-17 の取りこぼし）

- 場所: `src/parser/kakinoki.rs:842-844`
- 指摘: robustness / comment
- 改行で終わらないファイルで `先手の持駒：金` と `先手の持駒：なし` は読め、
  **`先手の持駒：` だけが「this hand line cannot be read」**。読めており、無いのは改行だけ
- **結果**: 直した（`4bc8244`）: `peek(end_of_line)`。`the_header_block_reads_without_a_trailing_newline`
  に空の持駒を足した

### R12-08 [MEDIUM] `moves_with_index` の `Position` 分岐は到達不能

- 場所: `src/parser/kif.rs:322-329`
- 指摘: rust / comment / architecture
- `move_with_comments` が `?` で返るので `out` は必ず1要素以上。起きない場合の説明が、
  この関数が盤面を守れるかのように読める
- **結果**: 直した（`4bc8244`）

### R12-09 [MEDIUM] 途中で切れた盤面図のエラーが、ファイルに無い行を名指す

- 場所: `src/parser/kakinoki.rs:1057-1063`
- 指摘: robustness
- 9行のファイルで `at line 10`。`main`（`at line 3`）も r10（`at line 4`）も無傷の行を
  指していたので、どのリビジョンも正しくない
- **結果**: 直した（`4bc8244`）: 残りが空なら「the file ends inside the board」と言う。
  R11-07 のテストに切断の腕を足した

### R12-10 [MEDIUM] D20 の記述2箇所が実態と違う

- 場所: `research/95-decisions.md`（D20）
- 指摘: spec / comment
- 「`main` の受理集合を保ったまま」が偽（空ブロックの3通りで狭い）。実装欄が同じラウンドで
  改名された `a_branch_header_is_all_the_line_says` を指している
- **結果**: 直した。表そのもの（tsshogi は `変化[：:]` のあと接尾辞を見ない）は一次実装で
  裏が取れており正しいので触っていない

### R12-11 [MEDIUM] `not_move_line` の doc が declines する集合から `|` `+` を落としている

- 場所: `src/parser/kakinoki.rs:616-642`
- 指摘: comment
- 列挙で閉じているのに実装は `where_it_is` 次第で2つ多く拒む。引数の意味が doc に無い。
  **R12-01 は実際にその選択を1箇所間違えたもの**
- **結果**: 直した（`4bc8244`）

### R12-12 [MEDIUM] `Position` はこのクレートでは局面を指す

- 場所: `src/parser/kakinoki.rs:602-614`
- 指摘: comment
- `shogi_core::Position` / `csa::Position` / `converter` の3箇所で局面の意味。
  `WhereABoardCouldStill` は文が途中で切れている
- **結果**: 直した（`1dabde7`）: `WhereInTheRecord::{BeforeTheFirstMove, PastTheOpeningBlock}`

### R12-13 [MEDIUM] `pub(super)` がモジュール外に呼び手の無い6項目に付いている

- 場所: `COLON` / `COLONS` / `ends_a_word` / `BRANCH_KEYWORD` / `KIF_SPECIAL_WORDS` /
  `a_move_follows_the_number`
- 指摘: architecture
- 「兄弟モジュールが引く共有の答え」の意味で使われているのに誰も引いていない。
  次に読む人は「共有されているのだから片方だけ直せば済む」と誤読する
- **結果**: 直した（`4bc8244`）: 5項目を private に。`ends_a_word` は R12-04 を直したので
  `pub(super)` のまま（**初めて共有になった**）

### R12-14 [MEDIUM] `handicap → parser` の矢印が新設されていた

- 場所: `src/handicap.rs:165`、`src/parser.rs:5-9`（そのための再輸出）
- 指摘: architecture
- `handicap.rs` の module doc は自分が葉であると名乗っている。`is_padding` の定義は
  `notation::LINE_ENDS` の上に書かれているのに置き場は形式別パーサの中だった
- **結果**: 直した（`1dabde7`）: `is_padding` を `notation.rs` へ。再輸出と矢印が同時に消えた

### R12-15 [MEDIUM] KIF の指し手行が `[<手番>]` を受けない。同じファイルを tsshogi は読む

- 場所: `src/parser/kif.rs:141-143`（`main` から）
- 指摘: spec
- R-KIF-005 の文法は `[<手番>]<移動先座標><駒>[<装飾子>]<移動元座標>`。tsshogi の
  `kakinoki.mjs:292` は `[▲△▼▽]?` を持つ。`   1 ▲７六歩(77)` は**ファイルごと `Err`**
  （`main` から。退行ではない）
- **結果**: 直した（`1dabde7`）: 印を受けて読み捨てる。手番は手数から決める（R-KIF-007。
  中断行が手数を1つ使うので偶奇は当てにならない）。marked / plain / 逆向き marked が
  同じ JKF になることをテストで固定

## 修正後の検証

- `bash .claude/verify.sh` — 通る（テスト180件。ラウンド11終了時は177件）
- コーパス609件（`~/Desktop/temp`）の読み書き:
  - **ラウンド5終了時（`771cd0f`）とバイト一致**（4回測って全て0差分）
  - `main` との差は意図した分だけ: 読み取り 0件 / `to_kif` 0件 /
    `to_ki2` 1件（`bug_mega.kif`）/ `to_csa` 33件（GAP-023）
- R12-01 のテストは**変異を当てて落ちることを確認**した

## 反省

ラウンド11 は「1所見1コミット」を守って17件を直したが、**そのうち2件が退行**だった。
どちらも同じ形をしている——**述語が数える集合と、パーサが消費する集合の乖離**
（R9-05 / R10-02 / R10-05 / R11-01 に続いて5ラウンド連続）。
R12-01 は `Position` を関数の中に定数で埋めたこと、R12-02 は `move_time` が別ファイルに
あったことが原因で、どちらも**「答えを持つべき場所」から離れた場所に判断を置いた**結果。

## research/ へ書き戻したもの

- **D20 の2箇所を訂正**（R12-10）
- GAP-020 の1 に「余白なし + 消費時間」「余白なし + 不成」の系列を追加

## 次ラウンドの対象

ラウンド12の修正。特に:

- `move_run` の `where_it_starts`（R12-01）——分岐側に渡した値が正しいか
- 終局語の腕が `move_time` を通すようになったこと（R12-02）。`1投了+` / `1投了もあった` は
  注記のままだが、`main` は終局として読む。この差が正しいか
- `成` / `不成` を指し手の形に足したこと（R12-03）——拒みすぎていないか
- コロンの述語化（R12-06）と `take_while1` への置き換え——空のキーの扱いが変わっていないか
- `side_mark`（R12-15）——`▲` を読み捨てることで KI2 側の判定が変わっていないか
- `is_padding` の移動（R12-14）
