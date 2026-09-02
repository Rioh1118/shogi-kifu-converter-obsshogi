# レビュー ラウンド14 — issue #6 / #7 / #8 / #9

対象: `fix/silent-failures-and-normalize-promote`（`git diff main...HEAD -- src/`）。
重点はラウンド13で入った構造変更（D21 / D22）。

観点5本（rust / spec / robustness / comment / architecture）を並列で実行。

- 重複を潰した結果: **BLOCK 2 / HIGH 3 / MEDIUM 13**
- **ラウンド13で私が入れた退行が1件**（R14-A）。D21 の3値の線引きを、`map_res` の
  失敗が回復可能な `Error` になることを見落として崩していた
- コミット `0d46cfd` / `ed7653f` / `1553ddc` / `37578f4`

## 所見

### R14-A [BLOCK] 読み始めた行を「自分の行ではない」と答え、指し手が黙って消える（**退行**）

- 場所: `src/parser/kakinoki.rs` `move_origin` の `map_res` と `numbered_line` の手数
- 指摘: rust（BLOCK）/ robustness（BLOCK）— 2本が独立に
- D21 は `Err(Error)` を「自分の行ではない」＝**数えない**と決めた。`map_res` は
  変換に失敗すると `Error` を返すので、`(999)` や20桁の手数がそこへ落ちた
- 実測:

  | 3手目の綴り | main | R12 終了時 | R13 終了時 | いま |
  | --- | --- | --- | --- | --- |
  | `3２二角(999)` | Err | Err | **Ok・その手が消える** | Err |
  | `3２二角(８８)` / `(88` / `(ab)` / `()` / `(八八)` / `(8)` | Err | **Ok・消える** | **Ok・消える** | Err |
  | 20桁の手数 | Err | Err | **Ok 0手** | Err |
  | `2同銀と取れば` | Err | 注記 | 注記 | 注記 |

- **R13-A′ の不変条件テストは、この穴を原理的に検出できなかった**（`move_line` が
  `Err` を返した行を `continue` で捨てていた）
- **結果**: 直した（`0d46cfd`）。`(` まで読んだ時点で指し手行と確定させ、以降は
  `broken_line`。テストは `Err(Failure)` の行も数えられていることを主張するようにし、
  壊れた本体10種と20桁の手数を直積に足した（**6,336通り**）。
  メッセージも `in Digit:` / `in Tag:` から
  `this move's origin square cannot be read` に変わった

### R14-B [BLOCK] `to_ki2` が局面を見失った先の駒打ちから `打` を落とす

- 場所: `src/converter/ki2.rs` の `_ => mv.relative`
- 指摘: spec
- KIF / CSA / tsshogi 由来の JKF は駒打ちを `from: None` だけで表す（R-JKF-003）ので
  `relative` は空。局面追跡は**指せない手**で止まり、反則手の記録は正常な入力（R-RULE-002）
- 実測: `bug_mega.kif` で **11手が「打ち」でなくなる**。書き出しは `Ok` を返し、
  読み直した JKF は `to_kif` が丸ごと `Err`（`Invalid (file, rank) for Square: (0, 0)`）
- **`main` も同じ**（このブランチの退行ではない）
- **結果**: 直した（`ed7653f`）。局面が無いときは `from: None` を `打` として書く。
  R-KI2-003 は「不足すると復元できなくなる」と書いており、過剰な `打` は読み手が受ける。
  コーパスの差は `bug_mega.kif` の KI2 が **+33バイト**（11 × `打`）のみ。**変異確認済み**

### R14-C [HIGH] 失うものが無い持駒行で棋譜を丸ごと拒む（R13-D3 の行き過ぎ）

- 場所: `src/parser/kakinoki.rs` `parse_without_moves`
- 指摘: robustness（HIGH）/ spec（MEDIUM）
- 条件が「持駒行を読んだか」だけで、**持駒が空でも拒んで**いた。`手合割：平手` +
  `先手の持駒：なし` は preset が初期局面を完全に決めており、失われる情報が無い
- 実測: コーパス609件の先頭に `先手の持駒：なし` を足すと **609件すべてが `Err`**。
  同じファイルを tsshogi は開く（R-KIF-014 が名指しで警告している食い違い）
- **結果**: 直した（`1553ddc`）。条件を「持駒が空でない」に。R13-D3 が狙った沈黙は `Err` のまま

### R14-D [MEDIUM] その拒否がヘッダブロックの先頭を名指す（**4回目**）

- 指摘: rust / robustness / comment
- `先手：Aさん` にキャレットが立つ。R11-07 / R12-09 / R13-D2 で3回直した形
- **結果**: 直した（`1553ddc`）。`InformationData` の `saw_a_hand_line: bool` を
  `hand_line: Option<&str>` に。**真偽値だけ持って位置を失う形が型として書けなくなった**

### R14-E1 [MEDIUM] 「指し手を読んだか」の答えが4箇所にあり、1つだけ違う

- 場所: `kif::moves_with_index`（無条件で `after_a_move()`）
- 指摘: robustness / comment / architecture — 3本
- `   1 投了` だけの走りは指し手を読んでいないのに守りを外していた
- **結果**: 直した（`37578f4`）。`after(&[MoveFormat])` にして**判定を型の側へ**。
  呼び手は「何を指し手と数えるか」を書けない

### R14-E2 [MEDIUM] BOM の規定が5入口のうち3つにしか貼られていない

- 指摘: architecture
- `parse_jkf_str` は BOM 付き JSON を拒む。**同じ棋譜が `.kif` なら読めて `.jkf` なら読めない**
- **結果**: 直した（`37578f4`）

### R14-E3 [MEDIUM] `MoveSpecial::ALL` 自身が手書きで、網羅を強制するものが無い

- 指摘: comment / architecture
- R13-E1 が塞いだはずの穴が、**塞いだ道具の中にあった**
- **結果**: 直した（`37578f4`）。網羅 `match` の `ordinal()` を対にし、
  バリアント追加がコンパイルエラーになる

### R14-F [HIGH] 反則勝ちの向きを逆に書いた doc が2箇所に複製された

- 場所: `NumberedBody` / `move_body` / `outcome_word`
- 指摘: comment
- 「反則勝ち accuses the player whose turn it is」は逆（**直前に指した側**が反則）。
  これを信じて組み立てを書き直すと**負けた側が逆の棋譜として保存される**。
  R13-C1 が `side_mark` から消した同じ文が、同じラウンドで新設した2箇所に複製されていた
- **報告書 R13-C1 の「直した」は不完全だった**（`c0a31e2` は `side_mark` だけ）
- **結果**: 直した（`37578f4`）

### R14-G [MEDIUM] doc の付き替わり（**11回目**）と経緯の混入（**8回目**）

- `jkf_keeps_a_drop_a_drop` の8行が BOM のテストに付き替わっていた
- 消えた `move_special` を「例外」として説明し続けるコメント
- `move_run` の doc が、その関数がいましていることを禁止していた
- `parse_without_moves` の要約が2つあり、`Err` を返すことを書いていなかった
- レビュー番号（`R10-04`。実際の判断は R9-14）、「the last five rounds」、
  「`main` refused three of these four」（配列は5件に増えていた）
- **結果**: すべて直した（`37578f4`）

### R14-H [MEDIUM] `research/` の記述3件が実装と食い違う

- **D21 の「引数に `&str` を取らない」が事実でない。** `move_line` は
  `ends_here` のために行を持っている。綴りの分裂を止めているのは**テスト1本**であって型ではない
- **D22 の「読み手にできるのは受け取って渡すことと `after_a_move()` だけ」**も、
  「指し手を読んだか」が4箇所にあった時点で成立していなかった
- **R-RULE-001 が D2・実装・tsshogi・`shogi_official_kifu` のいずれとも逆**。
  「到達可能」に自玉への王手除去を含めると、ピン局面で `直` が消えて外部2実装と食い違う
- **結果**: 3件とも書き戻した。D21 は「規約であって型の保証ではない」と明記し、
  効いているのが 6,336通りのテストであることを書いた。R-RULE-001 は実測付きで擬似合法に訂正

### R14-I [MEDIUM] `*` `&` `#` `まで` だけ表を持たない

- 指摘: architecture
- `SIDE_MARKS` / `NOTE_MARKERS` / `BRANCH_KEYWORD` / `COLONS` は表になっているのに、
  コメントとしおりとプログラム注記と `まで` は直書きが4/3/2箇所
- **現時点で再現する失敗は無い**（実測で確認済み）。片方が印を覚えて片方が覚えない形になれば沈黙する
- **記録に回した**。GAP-020 の並びに追記

### R14-J [MEDIUM] `ORIGIN_UNSTATED` を CSA が「駒台から」の判定に使っている

- 指摘: architecture
- **R4-20 として既出**で、`HIRATE_BOARD` の移動待ちだった。その受け皿は R13-E2 でできた
- **記録に回した**。値は同じでも問いが違うので定数を分ける話。挙動は変わらない

## 修正後の検証

- `bash .claude/verify.sh` — 通る（テスト186件。ラウンド13終了時は184件）
- コーパス609件（`~/Desktop/temp`）:
  - `main` との差は意図した分だけ: 読み取り 0件 / `to_kif` 0件 /
    `to_ki2` は `bug_mega.kif` の1件（**R14-B で +33バイト**）/ `to_csa` 33件（GAP-023）
  - ラウンド5終了時との差も **R14-B の33バイトだけ**
- R14-A / R14-B のテストは**変異を当てて落ちることを確認**

## research/ へ書き戻したもの

- **D21 / D22 の過大な主張を訂正**（何が型で、何がテストで守られているか）
- **R-RULE-001 を擬似合法に訂正**（tsshogi と `shogi_official_kifu` の実測付き）
- GAP-020 に R14-I（印の表が無い4種）

## 次ラウンドの対象

ラウンド14の修正。特に:

- `move_origin` / 手数の `Failure` 化（R14-A）——拒みすぎていないか
- `to_ki2` が局面なしで `打` を書くこと（R14-B）——過剰な `打` が KI2 の読み直しで悪さをしないか
- 持駒の条件（R14-C）と `hand_line` の伝搬（R14-D）
- `WhereABoardCouldBe::after` への一本化（R14-E1）
