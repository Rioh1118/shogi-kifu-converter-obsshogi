# レビュー ラウンド13 — issue #6 / #7 / #8 / #9

対象: `fix/silent-failures-and-normalize-promote`（`git diff main...HEAD -- src/`）。
重点はラウンド12で入った修正（`ea3a257` / `7820662` / `4bc8244` / `1dabde7`）。

観点5本（rust / spec / robustness / comment / architecture）を並列で実行。

- 重複を潰した結果: **BLOCK 2 / HIGH 4 / MEDIUM 12**
- **このラウンドから進め方を変えた。** 所見が揃った時点で**実装プランを立てる段を1つ挟む**
  （ユーザーの指示）。1件ずつ潰すと穴が隣へ移るだけになっていたため——
  reviewer 5本のうち4本が、独立に**同じ2つの根**を指した
- コミット `2f4a1ca` / `22e8589` / `edb42cd` / `c0a31e2` / `ce431d1` / `9ee9370` /
  `b1a531e` / `635887b`

## 根は2つだった

### 根1 — 「数える述語」と「消費するパーサ」が別々に書かれている

`a_move_follows_the_number`（`bool`）と `kif::move_line` がそれぞれ綴りを持ち、
**乖離の被害は一方向**——読み手が取れる行を述語が数えないと、読み飛ばしが行ごと持っていく。
R9-05 / R10-02 / R10-05 / R11-01 / R12-02 / R12-03 と**6ラウンド連続**で同じ形。

### 根2 — 「盤面図はまだ来うるか」を5箇所が別々の式で導いている

`kif.rs` の定数と `main.iter().any(..)`、`ki2.rs` の `out.is_empty()` と定数2つ。
R12-01 が塞いだのは1つだけで、KI2 の沈黙は2経路残っていた。

## 所見

### R13-A [BLOCK] 走りの先頭の終局行が、時計の綴りや注記1文字で黙って消える（**退行**）

- 場所: `src/parser/kakinoki.rs:452-460`（終局語の腕が `eof` を要求）、消費側は
  `src/parser/kif.rs:195-218`（`ends_here` = D17 を通す）
- 指摘: rust（BLOCK）/ robustness（BLOCK）/ architecture（HIGH）/ spec（HIGH）— **4本**
- 実測（`手数----` の直下 = 走りの先頭。`main` = `dbc436a`）:

  | 入力 | main | R12 終了時 | いま |
  | --- | --- | --- | --- |
  | `1投了 ( 0:03)`（R-KIF-008 の省略形） | Ok 投了 | **Ok 0手** | Ok 投了 |
  | `1中断 （封じ手）` | Ok 中断 | **Ok 0手** | Ok 中断 |
  | `1投了+`（`data/tests/kif/everyday_20211107.kif` に実在） | Ok 投了 | **Ok 0手** | Ok 投了 |
  | `変化：2手` + `2投了 ( 0:03)` | Ok 分岐1 | **Err（理由も嘘）** | Ok 分岐1 |
  | `.ki2` の `3投了+` | Err | **Ok・消える** | Err |

  終局語6種 × 行末10通りの総当たりで、**`.kif` は読めるのに `.ki2` は黙って捨てる組が 42/60**
  （`main` は60すべて loud）。
- **結果**: 直した（`22e8589`）。`kakinoki::numbered_line` を1本置き、
  `opens_a_numbered_line` は**その判定**になった（自前の綴りを持たない）。
  `kif::move_line` は戻り値から `MoveFormat` を組み立てるだけで**テキストを2度読まない**。
  `move_from` / `move_move` を共有側へ移し、`kif.rs` 側だけに綴りを足すことは
  **コンパイルできなくなった**。**D21 として記録**

### R13-B [BLOCK] 壊れた盤面図の `.ki2` が、まだ2経路で黙って平手になる（**退行**）

- 場所: `src/parser/ki2.rs:296-302`（`out.is_empty()` はコメントや `まで…` でも倒れる）、
  `:545-547`（3箇所目の読み飛ばしが定数のまま）
- 指摘: rust（BLOCK）/ spec（BLOCK）/ robustness（BLOCK）/ comment（BLOCK）— **4本**
- 実測:

  | 入力（壊れた図の `.ki2`） | main | R12 終了時 | いま |
  | --- | --- | --- | --- |
  | 指し手が1つも無い（詰将棋の図と持駒だけ） | Err | **Ok・平手に化ける** | Err |
  | 図の上に前局の `まで122手で先手の勝ち` | Err | **Ok・平手に化ける** | Err |
  | 盤面図の無い記録の、指し手の後の `|先手|後手|` | Ok | Ok | Ok |

- **結果**: 直した（`2f4a1ca`）。`WhereABoardCouldBe` はフィールドが private で
  **`parse_without_moves` だけが作れる**。読み手にできるのは受け取って渡すことと
  `after_a_move()` だけで、5つの導出式は**書けなくなった**。渡し忘れは守り続ける側
  （`Err` が増える）に倒れる。**D22 として記録**

### R13-A′ [HIGH] 不変条件をテストにした

- 指摘: rust / spec / robustness が揃って「レビューではなくテストで止めるべき」と書いた
- **結果**: `every_line_the_reader_takes_is_a_line_the_skip_counts`（`22e8589` / `edb42cd`）。
  手数3 × 区切り3 × 本体12 × 消費時間4 × 行末6 = **2,592通り**を回し、`move_line` が
  読めた行（500件超）すべてについて `opens_a_numbered_line` が真であることを主張する。
  **述語に `eof` を戻す変異で落ちることを確認**（`"1投了+\n" is read but not counted`）。
  **この1本があれば5ラウンド分の退行はレビューを待たずに止まっていた**

### R13-C1 [HIGH] `side_mark` の doc が手番の決め方を逆に説明している

- 指摘: comment
- `side_to_move_at_ply` はパリティそのもの（駒落ちの補正付き）。中断行を挟んでも正しいのは
  `move_line` の `known_side` の鎖。doc を信じて `known_side` を畳むと手番が全反転する
- **結果**: 直した（`c0a31e2`）

### R13-C2 〜 R13-C6 [MEDIUM] doc と命名

- `move_time` の要件 ID（R-KIF-005 → R-KIF-007 / R-KIF-008）
- **変更の経緯の混入（7回目）**。`used to come back Ok` は**私がラウンド12で足した**もの
- `move_run` の doc の1行目が「Reads one line」で本文と割れていた
- `notation.rs` の module doc が `LINE_ENDS` / `is_padding` を範囲外にしていた
- `skippable_line` / `skippable_line_except_a_branch_header` の名前と用途が逆
- `CLAUDE.md` の落とし穴が、既に存在しない `unimplemented!()` の panic を
  **BLOCK 級の重みで**書き続けていた（`grep` で0件）
- **結果**: すべて直した（`c0a31e2` / `635887b`）

### R13-D1 [MEDIUM] BOM が1行目に食い込み、駒落ちが平手になる

- 指摘: robustness
- GAP-006 の記述（「全内容が失われる／0手で `Ok`」）が実装と違っていた。実際は
  **「1行目だけが黙って化けた完全な棋譜」**で、そちらのほうが重い（気づけない）。
  Shogidokoro が BOM を書く
- **結果**: 直した（`ce431d1`）。KIF / KI2 / CSA の3経路で BOM を落とし、
  BOM の有無で `JsonKifuFormat` が完全一致することをテストで固定。**GAP-006 を解消印に**

### R13-D2 [MEDIUM] 盤面図の先頭2行が壊れると、壊れていない行を名指す

- 指摘: robustness
- `main` は壊れた行を指していた。R12-09 で直した「無い行を名指す」と同じ問題の別の入口
- **結果**: 直した（`ce431d1`）。筋見出しか枠線のどちらか一方が読めればそこは盤面図。
  `+7776FU` のように下に段が続かない `+` 行は今までどおり散文

### R13-D3 [MEDIUM] 持駒だけ書いてあって盤面図が無い記録が、駒を捨てて `Ok`

- 指摘: robustness
- `main` は4通りのうち3通りを**偶然**拒んでいた（改行を要求していたため）。R12-07 で
  改行の要否を揃えた結果、4通りとも沈黙する側に揃った
- **結果**: 直した（`9ee9370`）。コーパス609件に該当は0件

### R13-E1 [MEDIUM] KIF の終局語の表が3枚あり、網羅を強制するものが無い

- 指摘: architecture
- `kif_word()` に語を足すと `to_kif` は書き出すが `parse_kif_str` は読めない。
  テストが持っていた写しは**既に `不戦勝` / `不戦敗` が抜けていた**
- **結果**: 直した（`b1a531e`）。`MoveSpecial::ALL` を置き、両方向をテストで縛る

### R13-E2 [MEDIUM] `handicap` と `normalizer` が相互参照している

- 指摘: architecture
- `HIRATE_BOARD` は手合割の表の1行目（`PresetHirate`・除去なし）なのに、使う側にあった
- **結果**: 直した（`b1a531e`）。`handicap.rs` へ移し、循環を解いた

### R13-E3 [MEDIUM] `pub(super)` が共有でない2項目に付いている

- **結果**: 直した（`b1a531e`）。`end_of_line` / `NOTE_MARKERS` を private に

### R13-F [MEDIUM] `▼` / `▽` を tsshogi は読み、こちらは読まない

- 指摘: spec
- R-NOT-001 は `▲`/`△` の異体として `☗`/`☖`、`⛊`/`⛉`、`▼`/`▽` を挙げ、
  R-NOT-006 は読み取りで揺れを受けると決めている。tsshogi の指し手パターンは `[▲△▼▽]?`
- **記録に回した**。受理語彙を広げる判断で、**GAP-029（散文中の `▲` を KI2 だけが拒む）と
  同じ集合に触る**。GAP-029 はユーザー判断待ちなので、まとめて出す

### R13-G [HIGH] 作業ファイルが `examples/` に残り、`verify.sh` が赤かった

- 指摘: architecture
- 私がラウンドごとに置いたプローブ（`_bom.rs` / `_corpus3.rs` / `_probe.rs` / `_zz_spec13.rs`）。
  `.claude/verify.sh` は `--all-targets` を付けているので `examples/` を検証対象に入れる
- **結果**: 削除した。`git worktree prune` も実行。プローブは以後スクラッチへ置く

## 修正後の検証

- `bash .claude/verify.sh` — 通る（テスト184件。ラウンド12終了時は180件）
- コーパス609件（`~/Desktop/temp`）の読み書き:
  - **ラウンド5終了時（`771cd0f`）とバイト一致**（5回測って全て0差分）
  - `main` との差は意図した分だけ: 読み取り 0件 / `to_kif` 0件 /
    `to_ki2` 1件（`bug_mega.kif`）/ `to_csa` 33件（GAP-023）
- R13-A′ の不変条件テストは**変異を当てて落ちることを確認**

## research/ へ書き戻したもの

- **D21 を新設**: 「この行を数えるか」の答えは、その行を読む関数から導く。
  `Ok` / `Err(Failure)` / `Err(Error)` の3値が契約
- **D22 を新設**: `|` `+` を守るかどうかは「盤面図を読めたか」で決まる。記録内の位置では決めない
- **GAP-006 / GAP-001 を解消印に**（一覧の BLOCK 2行を削除）
- **GAP-020 の表を訂正**: ラウンド12で私が追記した行が `main` と HEAD の列を取り違えていた
- GAP-024 に `▼` `▽` の行（R13-F）

## 次ラウンドの対象

ラウンド13の修正。特に:

- `numbered_line` の `Error` / `Failure` の線引き（D21）——選び間違えていないか
- `WhereABoardCouldBe` の配り方（D22）——渡し忘れが `Err` 側に倒れることの確認
- 「持駒があって盤面図が無い」を `Err` にしたこと（R13-D3）——正当な入力を拒んでいないか
- BOM を落とすようになったこと（R13-D1）
- `MoveSpecial::ALL` と `HIRATE_BOARD` の移動（R13-E1 / R13-E2）
