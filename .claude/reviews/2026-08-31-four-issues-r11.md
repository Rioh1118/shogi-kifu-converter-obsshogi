# レビュー four-issues ラウンド11

- 日付: 2026-09-02
- 範囲: `git diff main...HEAD`
- 対象コミット: 開始時 `c6bcb44`（ラウンド10の修正 `efbc34b` 〜 `0b0f55c` が対象）
- 走らせた reviewer: rust / spec / robustness / comment / architecture
- 重複を潰した結果: **BLOCK 0 / HIGH 3 / MEDIUM 15**
- **17件を修正、1件（R11-06）を記録に回した。** コミット `be43ce3` / `1e4d994` / `ccb5501` / `285c4c1` / `55c217b`

**11ラウンドで初めて BLOCK が0件。** ラウンド10が持ち込んだ退行は3件（R11-01 / R11-03 / R11-06）で、
残りは `main` 由来か、doc と名前の問題。

## ラウンド10の検証（robustness の実測）

- コーパス注入 **33綴り × 29,793点 × 4リビジョン ≒ 393万パース**。
  `73ca7ad → HEAD` の「`Ok` なのに手数が少ない」**0件**、「`Ok` なのに盤面図が消えた」**0件**
- **R10-03 の利得が本体**: 盤面図の単文字変異1,842通りで「`Ok` のまま盤面が消える」が
  **1,568件 → 0件**、`Err` 件数は `main`（1,614）とほぼ同じ 1,609 に戻った
- GAP-020 の1 の回帰値は悪化していない（KIF 13 据え置き、KIF+変化 16→13、KI2 6→5）
- 200,000反復の fuzz と、空 / `0xFF` / NUL / 切れた Shift_JIS / BOM 各種で **panic 0件**
- 速度: `main` 142.9ms → HEAD **154.5ms（+8%）**。8倍の入力で 7.5倍なので**線形**
- 公開面は `main` と**完全一致（43項目、diff 0行）**、`Cargo.toml` バイト同一、
  `src/` は production 1.283 倍で**増分の 84% が `parser/` の3ファイル**

## 所見

### R11-01 [HIGH] `a_move_follows_the_number` が移動元を見ず、数字で始まる散文がファイルごと `Err` になる

- 場所: `src/parser/kif.rs:195-204`、`src/parser/kakinoki.rs:326-335`
- 指摘: rust
- **R10-05 が持ち込んだ。** 移動先と駒種だけを見るので `2同銀と取れば` は「指し手行」と数えられるが、
  実際に読む `move_line` は移動元（`打` か `(11)`〜`(99)`）を要求するので**誰も消費できない**
- 実測（本譜の下に1行）:

  | 置いた行 | `73ca7ad` KIF / KI2 | **HEAD** |
  | --- | --- | --- |
  | `2同銀と取れば` | `Ok` / `Ok` | **`Err` / `Err`** |
  | `1同歩` | `Ok` / `Ok` | **`Err` / `Err`** |
  | `2２六歩が本筋` | `Ok` / `Ok` | **`Err` / `Err`** |

- `opens_a_branch_header` の doc がこの故障を「a read wider than the count refuses records the
  format has always allowed」と名指ししているが、**入ったのはその逆向き（count > read）**
- 直し方: 移動元の**形**（`打` か `(<数字><数字>)`）を足す。`move_from` をそのまま `recognize`
  しないこと——`(00)` の `Failure` が `.is_ok()` で `false` に潰れ、`1７六歩(00)` が散文に落ちる
- **結果**: 直した（`be43ce3`）: 移動元の形（`(77)` / `打` / 同 / 成・不成）か、行を埋める終局語を要求する。`   1 序盤の課題` は散文へ戻り、`   1 ７六歩(77)` と `   2 パス` は指し手のまま

### R11-02 [HIGH] `手合割： 香落ち`（値の前の余白）で駒落ちが平手に化け、全手番が反転する

- 場所: `src/parser/kakinoki.rs:625-647` `information_value_preset`、`src/handicap.rs:160`
  `is_a_known_name`
- 指摘: spec
- R10-04 が持駒の値の前、R10-09 がキーの余白を直した。**preset の値の前だけ入っていない**
- 実測:

  | 入力 | 結果 |
  | --- | --- |
  | `手合割：香落ち` + `   1 ３四歩(33)` | `Ok` `PresetKY` `[White, Black]` |
  | **`手合割： 香落ち`** / `手合割：　香落ち` / `手合割：\t香落ち` | **`Err failed to normalize: Invalid move: ３三→３四 at ply 1`** |
  | `手合割： 平手` | `Ok`。`header["手合割"]=" 平手"`、preset は既定の平手 |

- **`is_a_known_name(" 香落ち")` が false になるので D16 のゲートも外れ**、
  書き戻すと preset 行が消える。tsshogi は `readHandicap` で値を `trim()` する
- 直し方: `information_value_preset` の先頭に `padding` を置き、
  `is_a_known_name` も trim して聞く。**片方だけだと D16 のゲートが外れたまま**
- **結果**: 直した（`1e4d994`）: `information_value_preset` が値の前の余白を消費する。`手合割： 香落ち` / `手合割：\t香落ち` が `PresetKY` に戻った

### R11-03 [HIGH] 空ブロックの線引きが引く `D17` は実在の表の行を指さず、ユーザー判断が `research/` に無い

- 場所: `src/parser/kakinoki.rs:221` / `:1735`、`src/parser/ki2.rs:496`、`src/parser/kif.rs:517`
- 指摘: comment
- 3点とも外れる:
  1. **D17 の表に `変化：2手を参照` の行は無い**（表は4行で、`( 0:01)` / `▲有利` /
     `（まで先手良し）` / `変化：２手`）。`grep "を参照" research/*.md` は0件
  2. **R10-02 のユーザー判断（2026-09-02）が `research/95-decisions.md` に書かれていない**。
     D19 で終わっている。ラウンド10が書き戻したのは GAP-020 と GAP-031 だけ
  3. **`kif.rs:519` の「Both are declarations … to tsshogi」は KIF について偽。**
     tsshogi は KIF では `変化：N手` を読まない（D3 規則1）
- **次に D17 を読んだ人は「D17 はこんなことを言っていない」と結論して
  `a_branch_header_is_all_the_line_says` を消し、R10-02（勝者が入れ替わる BLOCK）を戻す**
- 直し方: **D20** としてユーザー判断を記録し、4箇所の `D17` を差し替える。
  `kif.rs` から `and to tsshogi` を落とす
- **結果**: 直した（`ccb5501`）: **D20 として `research/95-decisions.md` に記録**し、4箇所の参照を D17 から差し替えた。kif.rs の tsshogi についての1文（KIF では偽）を落とした

### R11-04 [MEDIUM] `|` `+` の拒否が、盤面図の届かない場所にまで及ぶ

- 場所: `src/parser/kakinoki.rs:500`
- 指摘: rust / robustness（spec は「正当な綴りが見つからず tsshogi も `/^\|/` を無条件に
  盤面行として扱う」として所見にしない）
- **盤面図は `parse_without_moves` でしか読まれない。** 本譜が1手でも読まれた後に現れる
  `|` / `+` の行は盤面図の残骸ではありえないので、そこで拒む取引は利得を1つも生まない
- 実測（`main` / `73ca7ad` はすべて `Ok`）: `|先手|後手|` / `+123`（評価値）/ `+7776FU` が
  **`Err`**。コーパス注入で `OK→ERR` **1,928/29,793**
- **R10-03 の利得（silent 1,568 → 0）は全て本譜より前の領域で生じている**（robustness が実測）
- 直し方: 除外を「本譜より前の読み飛ばし」に限る。
  併せて **GAP-007 が今も「盤面図が崩れていると黙って平手になる」のままで、HEAD の挙動と
  正典が食い違っている**ので、取引の両側を書き直す
- **結果**: 直した（`1e4d994`）: `Position` を `not_move_line` に渡し、`|` `+` の保護は最初の指し手より前だけに効く

### R11-05 [MEDIUM] `opens_a_numbered_line` が行末を越えて次の行を見る

- 場所: `src/parser/kakinoki.rs:331-333`
- 指摘: spec
- `head` は行ではなく**残り入力全体**で、`is_padding` は `LINE_ENDS` を除くので
  `trim_start_matches` は `\n` で止まる。**下の行の中身**が「番号が指しているもの」として数えられる
- 実測: `  55` は `Ok`（GAP-020 の表どおり）。**`  55 `（行末に空白1つ）は `Err`**。
  `  55　` / `  55\t` / `   3   ` / ` 12  ` も同じ。`  55 ` が**ファイル末尾**なら `Ok`
- **R10-03 は「行末の空白はエディタや貼り付けで普通に付く」を理由に盤面図の行末を通したばかり**
- 直し方: 判定をその行だけに対して行う（`head.split(LINE_ENDS).next()`）
- **結果**: 直した（`be43ce3`）: 行に切ってから判定する

### R11-06 [MEDIUM] `attach_branch` が置き場の無さを報告しない。R10-01 の報告書が「直した」と書いた後半が入っていない

- 場所: `src/parser/ki2.rs:401` / `:404` / `:413` の裸の `return`
- 指摘: robustness
- ラウンド10の報告書 R10-01 は直し方に2つ挙げ、**結果欄は前半しか書いていない**。
  後半（`attach_branch` の戻り値）は入っていない
- **R10-02 が入口を広げた分だけ、この無言の落とし穴に落ちる綴りが増えている**:

  | header | main | `73ca7ad` | **HEAD** |
  | --- | --- | --- | --- |
  | `変化：2手目`（在る ply） | Ok 分岐2 | **Ok 本譜4手に化ける** | **Ok 分岐2 ✅** |
  | `変化：3手目`（**置き場なし**） | Ok 2手消える | Ok 本譜に化ける | **Ok 2手消える** |

- `main` と同値なので**退行ではない**。落ちる先も GAP-018 が記録済み。
  **新しいのは「入口を広げたのに、同時に入れると宣言した出口の報告が無い」という組み合わせ**
- 直し方: `attach_branch` の戻り値を `bool` にし、偽なら `broken_line` にする。
  **GAP-018 の「未決」に踏み込むので、そこはユーザー判断。少なくとも報告書の「直した」から
  後半を落として GAP-018 へ書き戻すこと**
- **結果**: **記録に回した**（`55c217b`）。`main` と同値で退行ではないが、`Err` にすると `main` が受理していた綴りを拒む。**GAP-018 に実測付きで書き戻し、ユーザー判断に出す**。ラウンド10の報告書 R10-01 の「直した」からも後半を落とした

### R11-07 [MEDIUM] 壊れた盤面図のエラーが、壊れていない行を名指す

- 場所: `src/parser/kakinoki.rs:842` `board_row` / `:855` `board`
- 指摘: robustness
- 実測（12行目の段見出しだけを `九` → `1` に壊す）:

  ```
  HEAD : at line 4, in cannot read this: +---------------------------+
  main : at line 3, in cannot read this:   ９ ８ ７ ６ ５ ４ ３ ２ １
  ```

  どちらも**壊れていない行**を指す。段見出し・枠線・マス数のどれを壊しても盤面図の先頭付近が出る
- **R10-03 が `|` `+` を読み飛ばさないようにした理由は「D1 が読めなかった行を報告する」ことだった。
  報告はされるが内容が届いていない**
- 既存テストのアサーションは `is_err()` だけで、**行番号を4に固定する変異でも通る**
- 直し方: 筋見出しと最初の枠線が読めた時点で「これは盤面図だ」と確定させ、以降の失敗を
  `broken_line(その行, …)` にする（`information_line_hands` が既に採っている形）
- **結果**: 直した（`55c217b`）: 筋見出しと上の枠線が読めた時点で盤面図と確定させ、以降は `broken_line` で行を名指す。`at line 3` → `at line 12`。段見出しと閉じ枠の2通りをメッセージと行番号で固定し、**変異を当てて落ちることを確認**

### R11-08 [MEDIUM] `a_branch_header_is_all_the_line_says` の名前と doc が挙動と違い、同義の名前の関数と逆の答えを返す

- 場所: `src/parser/kakinoki.rs:214-229` と `:296-301` `a_branch_header_fills_the_line`
- 指摘: comment / architecture
- 実測（下にブロック無し）: `変化：2手 別案` と `変化：2手（本命）` は **`Err`**（＝`true`）。
  名前は「ヘッダがこの行の言うことの全部」と主張するのに、「別案」とも言っている
- `変化：2手（メモ）` に対して `fills_the_line` は `false`、`is_all_the_line_says` は `true`。
  **70行離れた同じファイルに、英語として同義に読める2つの名前**
- 取り違えると `変化：2手 別案` + 空ブロックが `Err` から `Ok` に変わり、D1 の診断が黙る
- 直し方: 名前を判定の中身に合わせ、doc に分かれ目（数の直後が余白・注記の印・行末か）を書く
- **結果**: 直した（`ccb5501`）: `an_empty_block_here_is_worth_reporting` に改名し、`a_branch_header_fills_the_line` との差を doc で対比させた

### R11-09 [MEDIUM] `a_move_follows_the_number` の doc の1段落目が2段落目を否定し、挙げた例がこの関数を通らない

- 場所: `src/parser/kif.rs:178-194`
- 指摘: comment
- 「a line the skip declines is a line `move_line` can take」は偽（`   2 パス` と
  `   1 ７六歩(00)` は declines されるが `move_line` は取れない。**それが意図した設計**で、
  2段落目と `opens_a_numbered_line` の doc が正しく書いている）
- `   1 ７六歩(00)` は**この関数を通らない**（余白の枝で先に `true` になる）。
  届くのは `1７六歩(00)` のような無余白の綴りだけ
- **結果**: 直した（`ccb5501`）

### R11-10 [MEDIUM] テストの冒頭コメントが、自分の assert と正反対のことを言っている

- 場所: `src/parser/kakinoki.rs:1459-1464` と同テストの `:1496-1501`
- 指摘: comment
- 「数える集合＝読める集合」と宣言した直後に、**数えられるが読めない**2件を固定している。
  この2件が `Err` になるのは D1 / D8 の意図した取引（GAP-020 の1 に記録済み）
- **ラウンド10の報告書が「途中でそう書いてこのテストに捕まった」と記録しているとおり、
  実際に踏まれた道**
- **結果**: 直した（`ccb5501`）

### R11-11 [MEDIUM] ラウンド10の追加行に「直す前はこうだった」が4箇所（**6回目**）

- 場所: `src/parser/kakinoki.rs:250-254` / `:575-577` / `:1461-1464` / `:1743-1744`
- 指摘: comment
- **6回とも直した本人の説明文として入っている。** とくに `:252` の
  「the one that consumed it did not」は**同じ段落の1文目「Starts past the indentation」と矛盾する**
- **結果**: 直した（`ccb5501`）: 4箇所とも落とした（**6回目**）

### R11-12 [MEDIUM] ラウンド10の報告書の「`parser.rs` 側も同様」が事実でない（doc の付き替わり **8回目**）

- 場所: `src/parser.rs:649-661`、記録は `.claude/reviews/2026-08-31-four-issues-r10.md:172`
- 指摘: comment
- `0b0f55c` は `src/parser.rs` を**1行も触っていない**（`git log --stat` で確認）
- **「直した」という記録のほうが偽になっており、次のラウンドがこの報告書を根拠に飛ばす**。
  R4-20 / R7-08 と同じ失敗の3回目
- **結果**: 直した（`285c4c1`）: 報告書の「`parser.rs` 側も同様」を事実に直し、宙に浮いたコメントを `the_extension_chooses_which_encoding_is_tried_first` の頭へ戻した

### R11-13 [MEDIUM] 共通側が KIF を参照する必要は無い

- 場所: `src/parser/kakinoki.rs:334` `super::kif::a_move_follows_the_number`
- 指摘: architecture
- **循環も再帰も退行も無い**（architecture が呼び出しグラフを辿って確認）。
  ただし `kif.rs` に置く理由になっているのは `move_special` 1本だけで、
  `padding` / `move_to` / `piece_kind` は既に共通側にある
- 結果として **「KI2 が読めるか」の答えが `parser/kif.rs` の中の配列で決まる**
  （`2投了のあと` が KI2 で `Err` になるのは `KIF_SPECIAL_WORDS` に `投了` があるから）
- 直し方: `KIF_SPECIAL_WORDS` と `move_special` を共通側へ移せば `kakinoki → kif` の矢印が消える
- **結果**: 直した（`ccb5501`）: `KIF_SPECIAL_WORDS` / `move_special` / `a_move_follows_the_number` を共通側へ移し、**`grep -c "super::kif" src/parser/kakinoki.rs` が 0**

### R11-14 [MEDIUM] D17 の「語がどこで終わるか」の合成が2箇所に手書きになった

- 場所: `src/parser/kakinoki.rs:225-227`（R10-02 が新設）と `src/parser/ki2.rs:218`
- 指摘: architecture
- 表（`NOTE_MARKERS` / `LINE_ENDS`）は1本化済みだが、**合成のほうが2本ある**。
  **同じ1文字の追加が別のバグとして出る**（片方は終局語の切れ目、片方は空ブロックの報告）
- **結果**: **半分しか直していない**（`ccb5501`）。`ends_a_word` は作ったが呼び手は1つで、`ki2.rs` の終局語の切れ目は手書きのまま残った。ラウンド12 の R12-04 で指摘され、`4bc8244` で `ki2.rs` が呼ぶようにした

### R11-15 [MEDIUM] `colon` が「コロンとは何か」の唯一の家だと doc で名乗るのに、キーの規則がそれを呼ばない

- 場所: `src/parser/kakinoki.rs:133` と `:743-751`（R10-06）
- 指摘: architecture
- 全角の腕は `colon` を1度も通らず、`：` を `is_not` / `tag` / `char` リテラルで**3回**書いている
- `colon` に3つ目の文字が入ったとき、`is_not` がその文字をキーに含めてしまい
  **そのヘッダ行が丸ごと消える**
- **結果**: 直した（`ccb5501`）: `COLON` / `COLONS` / `FULL_WIDTH_ONLY` / `EITHER_COLON` を1箇所に

### R11-16 [MEDIUM] `変化` の綴りが2箇所に増え、doc の「by construction」が1行で外れている

- 場所: `src/parser/kakinoki.rs:211`（R10-13 の速度ガード）と `:263`
- 指摘: architecture
- 今日は一致しているので実害は無いが、`branch_header_ply` の受理集合を広げると
  **ガードが静かに絞り込んで届かなくなる**。それは R9-05 / R10-02 / R10-05 が
  3ラウンド連続で踏んだ「数える集合 ≠ 読める集合」そのもの
- **結果**: 直した（`ccb5501`）: `BRANCH_KEYWORD` に

### R11-17 [MEDIUM] 見出しブロックが `line_ending` を要求し、改行で終わらないファイルの最後のヘッダ行が黙って消える

- 場所: `src/parser/kakinoki.rs:667` / `:691` / `:775` / `:620` / `:851` / `:863` / `:880`
- 指摘: rust
- **`main` も同じ。退行ではない。** ただし同じモジュールの `end_of_line` の doc が
  「A text file need not end with a newline … Requiring `line_ending` drops the last line」と
  規則を1回書いており、**6箇所がそれを守っていない**
- 実測: `…\n棋戦：竜王戦`（改行なし）→ **`Ok` `header={}`。行ごと消える**。
  `…\n先手の持駒：金`（改行なし）→ `Err`「this hand line cannot be read」——
  **R10-04 が入れたこのメッセージはこの場合に嘘をつく**（持駒の行は読めており、無いのは改行だけ）
- **結果**: 直した（`ccb5501`）: 6箇所を `end_of_line` に。棋戦 / 手合割 / 持駒 / 後手番 / 枠線の5通りをテストで固定

### R11-18 [MEDIUM] `opens_a_shared_line` の中で行頭余白の扱いが割れ、`kif.rs` に無用の trim とその古いコメントが残った

- 場所: `src/parser/kakinoki.rs:192-194` と `:207-212`、`src/parser/kif.rs:115-117`
- 指摘: rust
- `opens_a_shared_line("　変化：2手")` は `true`、`("　*コメント")` と `("　まで2手で投了")` は
  `false`。**同じ関数の中で契約が2つある**（今日の呼び手はすべて事前に trim しているので実害なし）
- `kif.rs:115-117` は「indentation included」と言った直後に**その indentation を落として**渡している。
  R10-01 で `branch_header_ply` が自分で消すようになったので無用
- **結果**: 直した（`ccb5501`）: 3節とも余白の向こうから見る。`kif.rs` の trim も落とした

## 見ていない範囲

- `.ki2` / `.csa` / `.jkf` の実コーパスが1件も無い
- CSA 経路はラウンド10の差分に無いため誰も読んでいない
- obs-shogi はソース読みと `node_modules/tsshogi` のみ。ビルドしていない
- 深い入れ子の `forks`（GAP-019）、`normalize` の冪等性、BOM（GAP-006）
- 盤面図の**行の中**（駒種の異体、桁ずれ）は単文字変異でしか触っていない

## 修正後の検証

- `bash .claude/verify.sh` — 通る（テスト177件。ラウンド10終了時は174件）
- コーパス609件（`~/Desktop/temp`）の読み書き:
  - **ラウンド5終了時（`771cd0f`）とバイト一致**（3回測って全て0差分）
  - `main` との差は意図した分だけ: 読み取り 0件 / `to_kif` 0件 /
    `to_ki2` 1件（`bug_mega.kif`）/ `to_csa` 33件（GAP-023）
- R11-07 のテストは**変異を当てて落ちることを確認**した（`board` の `map_err` を
  外すと `at line 3` に戻り、テストが落ちる）

## research/ へ書き戻したもの

- **D20 を新設**（`95-decisions.md`）: `変化：<数字>` は接尾辞に関わらず宣言。
  「行が数字で終わるか」は**空ブロックを報告するかどうかだけ**を決める。
  R10-02 のユーザー判断が正典に無いまま4箇所から D17 を引いていたのを解消
- **GAP-018 に KI2 側の節を追加**: D20 が入口を広げた分だけ `attach_branch` の
  無言の破棄に落ちる綴りが増えている。実測表と、`Err` にすると受理集合が
  狭まること、コーパスでは0件だが consumer では通りうることを記録（R11-06）

## 次ラウンドの対象

ラウンド11の修正。特に:

- `a_move_follows_the_number` が移動元を要求するようになったこと（R11-01）——
  拒みすぎていないか
- `Position` が `not_move_line` に渡るようになったこと（R11-04）
- 見出しブロックが `end_of_line` になったこと（R11-17）——行を跨いで読んでいないか
- `board` が手書きループになったこと（R11-07）
- 共通側へ移した3項目（R11-13）の置き場
