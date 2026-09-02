# レビュー four-issues ラウンド8

- 日付: 2026-09-02
- 範囲: `git diff main...HEAD`
- 対象コミット: 開始時 `e10c2a9`（ラウンド7の修正 `cac49d2` 〜 `256432f` が対象）
- 走らせた reviewer: rust / spec / robustness / comment / architecture
- 重複を潰した結果: **BLOCK 1 / HIGH 3 / MEDIUM 7**
- **10件すべて修正。** 1所見1コミット（`bf2da21` 〜 `8b6a878`）。doc の4件だけ1コミットにまとめた

**ラウンド7が持ち込んだ退行は2件**（R8-02 / R8-03、どちらも R7-04 が `変化：` の数字を広げた副作用）。
残りは `main` 由来だが、**ラウンド7が KI2 側だけを直したことで非対称が開いたもの**が中心。

## ラウンド7の検証（robustness の実測）

R7-04 が直したことは実測で確認された。

- コーパス609件に1箇所ずつ綴りを注入（**29,834通り／綴り**）:
  `変化：２手`（全角）の `SAME→ERR` は `a402f71` で **26,758件**、**`HEAD` で 0件**
- `a402f71 → HEAD` は全綴りで `SAME→ERR` 0件、「`Ok` なのに元より短い」も0件
- 単文字変異 2,112通りで `a402f71` と `HEAD` の出力が**1文字も違わない**
- コーパス609件の無注入結果は `main` / `771cd0f` / `a402f71` / `HEAD` で**609行すべて同一**
- 80万パースで **panic 0件**。`ATTEMPTS=8` の速度は線形（41 KB が 42 µs、`771cd0f` は 11.6 秒）

## 所見

### R8-01 [BLOCK] 「行頭の余白の向こうを見る」が KIF 側に入っておらず、KIF だけが字下げで壊れる

- 場所: `src/parser/kakinoki.rs:378-383` `not_move_line` の述語（`' '` だけを直書きで除外）/
  `src/parser/kif.rs:150` `skippable_line_except_a_branch_header`（生の `input` を聞く）/
  `src/parser/kif.rs:276` `move_line` の `space0` / `src/parser/kif.rs:176` `skip_interruptions`
- 指摘: architecture（BLOCK）/ spec（HIGH）/ rust（MEDIUM）/ comment（MEDIUM）— 4本が独立に
- **R7-02 は KI2 側（`move_run` / `a_line_only_prose_opens`）だけを直した。** 共通モジュールと
  KIF 側は `is_padding` を引いていない。しかも除外されているのが **ASCII の半角空白1文字だけ**なので、
  壊れ方が「半角なら `Err`、タブ・全角・NBSP なら黙って消える」という説明のつかない形になっている
- 実測1（本譜の下に1行置く。`main` も同じ）:

  | 置いた行 | `.kif` | 同内容の `.ki2` |
  | --- | --- | --- |
  | `終わり` / `\t終わり` / `　終わり` / `\u{a0}終わり` | `Ok` | `Ok` |
  | `  終わり`（半角空白） | **`Err`** | `Ok` |
  | `  # メモ` / `  *コメント` / `  &しおり` / `  まで2手で投了` | **`Err`** | `Ok` |

- 実測2（指し手行の字下げを変える）:

  | 行頭 | KIF 指し手3手 | KIF 走りの途中1行 | KIF `*` コメント |
  | --- | --- | --- | --- |
  | `' '` | `Ok` 3手 | `Ok` 3手 | **`Err`** |
  | `'\t'` | **`Ok` 0手** | `Ok` 3手 | **`Ok`・コメント0件** |
  | `'　'` / `'\u{a0}'` | **`Ok` 0手** | **`Err` `Invalid move: ２七→２六 at ply 2`** | **`Ok`・コメント0件** |

- 実測3（`変化：` の字下げ）:

  | pad | KIF（ブロックあり） | KIF（ブロック空） | KI2 |
  | --- | --- | --- | --- |
  | 無し | `Ok` fork 1 | `Err`「a 変化 block with no moves under it」 | `Ok` fork 1 |
  | `' '` | **`Err`** | `Err` | `Ok` fork 1 |
  | `'　'` / `'\t'` / NBSP | `Ok` fork 1 | **`Ok`。診断が黙って消える** | `Ok` fork 1 |

- KIF は指し手行を3桁の空白で字下げする形式（R-KIF-005 の例）なので、その列に揃えて注記や
  `まで…` を書いた棋譜がある。`is_padding` の doc が「a web page pads with `\u{a0}`」と書いた前提は
  KIF の指し手行にも同じだけ当てはまる
- コーパス609件に該当は0行（`main` も同じで退行ではない）。合成入力のみ
- 直し方: 「行頭の余白」を `is_padding` の1本にする。`not_move_line` は述語を当てる前に余白を消費し、
  `move_line` の `space0` と `skip_interruptions` の `[' ', '\t']` を置き換え、
  `skippable_line*` は `opens_a_branch_header(input.trim_start_matches(is_padding))` を聞く。
  回帰テストは KI2 側にある `a_line_is_the_line_it_is_however_far_in_it_starts` の KIF 版
- **結果**: 直した（`ee58b7a`）: `not_move_line` / `move_comment_line` / `comment_line` / `kif::move_line` / `skip_interruptions` / `skippable_line*` / `branch_header_line` を `is_padding` に寄せた。無し・半角・3桁・タブ・全角・NBSP の6通りで、散文・`#`・`*`・`&`・`まで…`・指し手行・`変化：` が同じ結果になることをテストで固定

### R8-02 [HIGH] `変化：` の番号が `usize` に収まらないと、KIF が使いもしない数でファイルごと `Err`

- 場所: `src/parser/kakinoki.rs:212` `.parse::<usize>().map_err(|_| unreadable())?`
- 指摘: rust（HIGH）/ spec（MEDIUM）/ robustness（MEDIUM）
- **R7-04 が持ち込んだ。** `opens_a_branch_header` は数字を1文字見るだけなので、
  `usize` に収まらない数はちょうど「数える集合」と「読める集合」の差分に落ちる。
  **R7-04 の doc とテストがその差分は無いと明言している**
- 実測:

  | 入力（本譜2手 + `変化：<N>手` + 分岐1手） | main | a402f71 | **HEAD** |
  | --- | --- | --- | --- |
  | `変化：18446744073709551615手`（`usize::MAX`） | `Ok` 分岐1本 | `Ok` 分岐1本 | `Ok` 分岐1本 |
  | `変化：18446744073709551616手` | **`Ok` 分岐1本** | **`Ok` 分岐1本** | **`Err`** |

- **KIF はこの数を1文字も使わない**（D3 規則1。`branch_header_line` のコメント自身がそう書いている）ので、
  弾いて得るものが無い。合成 KIF の全位置16通りで **8箇所が `Ok`→`Err`**
- **ユーザー判断（2026-09-02）: 手数は2000以下が確実に通ればよく、10万・20桁は気にしない。**
  ただし「気にしない」は「ファイルごと拒む」ではない
- 直し方: 文字列を組んで `parse` する代わりに1桁ずつ畳んで飽和させる。
  `saturating_mul(10).saturating_add(d)` にすれば「読めない数」が消え、集合の等式が本当に成り立つ。
  併せて `u32::from(c) - FULL_WIDTH_ZERO` が述語と別の式に分かれている問題も消える
  （`is_numeric()` などへ広げた瞬間に debug ビルドで underflow panic する）
- **結果**: 直した（`bf2da21`）: 1桁ずつ畳んで飽和させる。述語と減算が同じ式になり、`is_numeric()` に広げたときの underflow も消えた。**ユーザー判断 D19 として `research/95-decisions.md` に記録**

### R8-03 [HIGH] `変化：０手` が ply 0 を返し、`attach_branch` が分岐も終局も黙って捨てる

- 場所: `src/parser/kakinoki.rs:181`（0 を拒まない）と `src/parser/ki2.rs:388-393`
  （`start_ply.checked_sub(1)` が `None` → `return`）
- 指摘: rust
- **R7-04 が持ち込んだ。** 全角数字に広げたので `変化：０手` が初めてこの `return` に届くようになった
- 実測（最小形: 平手 + 4手 + `変化：０手` + 分岐2手 + `まで6手で中断`）:

  | | a402f71 | **HEAD** |
  | --- | --- | --- |
  | 結果 | **`Err`** | **`Ok`。分岐2手と `中断` が消える** |

  合成 KI2（12ノード / fork 1 / special 1）では **`Ok` 7ノード・fork 0・special 0**
- `変化：0手`（半角）は両方で `Ok` + 無言の欠落（GAP-018 の機構）。
  **今回の変更は綴りをもう1つその入口へ通したもの**
- 仕様: R-JKF-001（ply は1から）、R-REQ-004、R-JKF-004
- 直し方: `branch_header_ply` が `0` を `unreadable()` にする。KIF は数を使わないので影響なし、
  KI2 は意味の無い宣言を拒める。返り値を `NonZeroUsize` にすれば型で担保できる
- **結果**: 直した（`bf2da21`）: 拒否は「数を使う側」＝KI2 に置いた。KIF に置くと読みもしない数字で棋譜を拒む。併せて `move_run` の `first_ply + numbered` を飽和させた（飽和 ply がそのまま流れると Tauri コマンドの中で panic する）

### R8-04 [HIGH] `持駒：なし` の後ろに空白が1つあるとファイル全体が `Err`。`手合割：香落ち\t` は手番が全反転

- 場所: `src/parser/kakinoki.rs:445`（`なし` の腕だけ余白を食わない）/ `:449` / `:489`
  （`many0(one_of(" 　"))` はタブと NBSP を知らない）/ `:532`（`Failure`）
- 指摘: spec
- **`main` も同じ。退行ではない**
- 実測:

  | 入力 | 結果 |
  | --- | --- |
  | `後手の持駒：なし` / `後手の持駒：歩 ` / `歩　` | `Ok` |
  | `後手の持駒：なし ` / `なし　` / `なし\t` | **`Err KIF Error: 0: at line 1, in Tag:`** |
  | `手合割：香落ち` / `香落ち ` / `香落ち　` | `Ok` `PresetKY`、1手目 `White` |
  | `手合割：香落ち\t` / `香落ち\u{a0}` | **`Err failed to normalize: Invalid move: ３三→３四 at ply 1`** |

- 1つ目は `Failure` なので `opt(board)` の外まで抜け、**盤面図を持つ棋譜が丸ごと落ちる**。
  エラーは「in Tag:」としか言わず原因の空白は表示にも出ない（R-REQ-004 の3）。
  tsshogi の `readHand` は `split(/[ 　]/)` で空片を捨てるので**同じファイルを読む**
- 2つ目は preset が読み手の既定の平手に落ち、駒落ちは上手が先（R-HC-001 / R-RULE-006）なので**全反転**
- GAP-021 は「表にある名前に**注記**が付いた形」を未決としているが、**余白は注記ではない**
- 直し方: `なし` の腕を `terminated(..., many0(satisfy(is_padding)))` にし、
  `one_of(" 　")` の2箇所を `is_padding` に置き換える
- **結果**: 直した（`3bbae41`）: `なし` の腕と `one_of(" 　")` の2箇所を `is_padding` に。持駒7通り × 3種、手合割5通りで固定

### R8-05 [MEDIUM] 半角コロンを読まないので、KI2 の分岐が本譜に化け、駒落ちが痕跡なく消える

- 場所: `src/parser/kakinoki.rs:185` `tag("変化：")` / `:524` `tag("の持駒：")` / `:508` / `:575`
- 指摘: spec
- **`main` も同じ。退行ではない。** tsshogi は `[：:]` で両方を受ける
  （`~/obs-shogi/node_modules/tsshogi/dist/esm/kakinoki.mjs:131-181`）。
  R-KIF-014 の規則表も「tsshogi は半角 `:` も受ける」と明記している
- 実測:

  | 入力 | 結果 |
  | --- | --- |
  | `手合割:香落ち` + 平手でも合法な2手 | **`Ok`。`preset=PresetHirate`、`header` は空。手番が全反転** |
  | `.ki2` の `変化:3手` | **`Ok`、fork 0。分岐の1手目が本譜の3手目に連結** |
  | `後手の持駒:飛` | **ファイル全体が `Err`**（R8-04 と同じ `Failure` 経路） |
  | `.kif` の `変化:3手` | `Ok`（KIF の木は手数から作る。D3 規則1） |

- 1つ目は `not_move_line` に散文として飲まれるので `手合割` の文字列すら残らない
  （GAP-021 は `header` に残る形なので、こちらのほうが重い）
- 直し方: `変化：` `の持駒：` `：` を「全角 `：` または半角 `:`」に広げる。
  R7-04 で判定と消費が同じ関数を通るようになったので、`branch_header_ply` の1箇所で両方に効く。
  `information_line_keyvalue` を広げるときは**最初のコロンで割る**こと
  （`開始日時：2021/06/29 09:00:00` を壊さない）
- **結果**: 直した（`124121e`）: **3つのキーワードだけ**を `[：:]` に広げた。一般の key-value 行は全角のまま——半角を許すと `{"header":{},"moves":[{}]}` が「ヘッダ1件の棋譜」として読め、D1 / D8 のエラーが誰にも届かなくなる（実際に既存テストが落ちて分かった）

### R8-06 [MEDIUM] 消した `SPACES` を指す doc リンクが3件残り、`verify.sh` が `cargo doc` を走らせないので素通しする

- 場所: `src/parser/kakinoki.rs:258`（`[space0]` / `[SPACES]`）と `src/parser/ki2.rs:166`（`[ATTEMPTS]`）
- 指摘: architecture / comment / rust — 3本が独立に
- `cargo doc --no-deps --document-private-items` が3件の `unresolved link` を出す。
  `SPACES` は R7-05（`b2d9c8d`）が削除した定数で、**リポジトリ全体の残り1箇所がこの doc**
- この doc は R6-06 の再発防止の説明で、**次にこの述語を触る人が根拠を探しに行く場所**。
  表はもう無いので、読み手は段落ごと落とすか `space0` に戻す誘惑を読む
- `.claude/verify.sh` は `fmt` / `clippy` / `test` の3ステップ。`-D warnings` は clippy にしか
  掛かっておらず、**rustdoc の警告はこのリポジトリで誰も見ていない**
- 直し方: 3件を直し、`verify.sh` に `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps
  --document-private-items` を足す。既存の残り警告（`normalizer.rs:193` の private link、
  redundant explicit link target）も先に潰す
- **結果**: 直した（`ac83ea6`）: 6件の rustdoc 警告を全部潰し、`verify.sh` に `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --document-private-items` を足した

### R8-07 [MEDIUM] `SIDE_MARKS` の doc が「`▲` を含む行は KIF でも捨てない」と言うが、KIF は捨てる

- 場所: `src/parser/kakinoki.rs:76-79`
- 指摘: comment
- `not_move_line` が見ているのは**行頭の1文字だけ**。行の途中の `▲` には触れない。
  「含む」を見ているのは `parser/ki2.rs:349` の `line.contains(...)` だけ
- 実測（本譜1手の `.kif` の下に1行）: `※▲２六歩が本筋` / `（▲７六歩まで）` / `感想：▲有利でした`
  はいずれも **`Ok` 2ノード。行は痕跡なく消える**
- **GAP-029 が「ユーザー判断待ちの非対称」として記録した事実の逆を断言している。**
  この doc を信じると、GAP-029 の選択肢 (a) を「KIF は既にそうなっているから安全」と誤って評価する
- **結果**: 直した（`8b6a878`）: 「行頭で始まる場合だけ」に直し、GAP-029 への参照を足した

### R8-08 [MEDIUM] 速度テストの根拠コメントが、新しく差し込んだテストの見出しになった（4回目）

- 場所: `src/parser/ki2.rs:787-796`（コメント）と `:797` / `:823`（2つのテスト）
- 指摘: comment
- `a13f454`（R7-03）が既存の6行コメントの直後に空行なしで新テストを差し込んだ。
  前半（19.6 s / 「上限はゆるくしてある」）は下の
  `a_header_value_holding_a_whole_game_is_read_in_one_pass` の話で、そちらには説明が1行も無い
- **見出しになっているテストには時間の assert が無い。** 読んだ人はこれを速度の回帰テストと誤解し、
  `ATTEMPTS` を増減する変更を「速度テストが通ったから安全」と判断する
- **R3-17 / R4-04 / R6-13 と同じ故障の4回目。R6-13 を直した直後の再発で、原因も同じ**
  （既存コメントの直後への挿入）
- **結果**: 直した（`8b6a878`）: 説明を `a_header_value_holding_a_whole_game_is_read_in_one_pass` へ戻した

### R8-09 [MEDIUM] `parse_kif_file` / `parse_ki2_file` の公開 doc が「拡張子が文字コードを決める」と言うが、決めているのはバイト列

- 場所: `src/parser.rs:66` / `:205` / `:635-640`（テストの名前とコメント）
- 指摘: comment
- 同じファイルの `read_kifu` の doc が逆を言っている（「R-REQ-003: the extension names an
  encoding but does not guarantee one. … **the bytes get the last word**」）
- 実測: UTF-8 のバイト列を `utf8.kif` として保存 → **`Ok`**。Shift_JIS を `sjis.kifu` → **`Ok`**。
  doc の規則どおりならどちらも復号に失敗するはず
- **公開 API の契約が実装と逆**で、`parse_csa_file` の doc だけが正しい説明を持っている。
  3つを並べて読んだ consumer は「CSA だけ賢い」と結論し、obs-shogi 側で自前の再試行を書く
- `main` にも同じ文がある（上流から引き継いだ1行が D14 / R-REQ-003 の実装に追い越された）
- 併せて `# Errors` に `ParseError::FileExtension` と `ParseError::Decode` が書かれていない
- **結果**: 直した（`8b6a878`）: 拡張子は「最初に試す文字コード」と「そもそも読むかどうか」を決める、に書き直し。`# Errors` に `FileExtension` / `Decode` / `Io` を足し、テスト名も実態に合わせた

### R8-10 [MEDIUM] R7-01 が新設した経路（盤面図の下のコメント・並び順）にテストが無い

- 場所: `src/parser/kakinoki.rs:37` `InformationData::merged` と `:788-800`
  `comments_on_the_starting_position`
- 指摘: rust
- 変異を当てた実測:

  | 変異 | 結果 |
  | --- | --- |
  | `merged` が `rhs.comments` を捨てる（**盤面図の下のコメントを落とす**） | **157 passed** |
  | `comments_on_the_starting_position` が順序を逆にする | **157 passed** |

- 実際の動作は正しい（盤面図の上下に置いた `*` が原文の順で `moves[0].comments` に入る）。
  **正しさを支えているものが何も無い**だけ
- `a_comment_over_the_first_move_is_not_a_header` が押さえているのは「盤面図の無い KIF/KI2 で
  コメント1本」だけ。詰将棋・任意局面の棋譜はこの経路しか通らない（GAP-007）
- **結果**: 直した（`8b6a878`）: 盤面図の上下と `手数----` の下にコメントを置いて順序を固定。**reviewer が当てた2つの変異（`rhs.comments` を捨てる／順序を逆にする）で落ちることを確認した**

## reviewer が挙げたが所見にしなかったもの

- **`ATTEMPTS=8` が足りない形**（robustness が構成した、注記の中に読めないマーク7個）:
  GAP-020 の5 が既に記録している形そのもの
- **`is_padding` が U+2028 / U+2029 / U+0085 を余白にすること**: robustness が全位置で試して
  沈黙する欠落を見つけられなかった。`U+FEFF` / `U+200B` は `is_whitespace()` ではないので余白にならない
  （`main` と同じ挙動）
- **BOM 付き文字列を `parse_*_str` に渡すとヘッダキーが `\u{feff}手合割` になる**: GAP-006 のまま
- **`header` のキーが `*` / `&` で始まる JKF**: 書き出すとコメント行になり、読み戻すと
  `moves[0].comments` に入る。そのキーを作る経路が無い
- **`to_ki2` のヘッダ行順が `HashMap` 順**（GAP-022）なので、行番号で挿入位置を決める注入実験は
  KI2 に対して非決定的。**次に同じ実験をする人は先にヘッダを並べ替えること**
- 公開 API の差分なし（`main` / `a402f71` / `HEAD` で `pub` 項目の集合が md5 一致、38項目）
- `branch_header_ply` の UTF-8 境界に panic なし（`変化：1２3手` = 123 など）
- `comments_on_the_starting_position` の `else { return; }` は到達不能

## 数えた結果（architecture）

| 知識 | R6 | R7 | **R8** |
| --- | --- | --- | --- |
| 手番記号 | 3 | 2 | **2** |
| 注記の印 | 1 | 1 | **1** |
| 行末の集合 | 2 | 2 | **2** |
| 終局語 | 3 | 3 | **3** |
| 駒種の表記 | 4 | 4 | **4** |
| 「これは `変化：` の行か」 | 3 | 1 | **1（読み手）** |
| **行頭の余白とは何か** | 未計測 | 6 | **4**（残り3つが R8-01 の発現） |

## 見ていない範囲

- `.ki2` / `.csa` / `.jkf` の実コーパスが1件も無い
- obs-shogi はソース読みと `node_modules/tsshogi` の実装読みのみ。ビルドしていない
- CSA 経路はラウンド7の差分に無いため誰も読んでいない
- 深い入れ子の `forks`（GAP-019）、`normalize` の冪等性、盤面図が途中で切れた場合（GAP-007）

## 修正後の検証

- `bash .claude/verify.sh` — 通る（テスト162件。ラウンド7終了時は157件）。
  **`cargo doc` のステップが増えた**（R8-06）
- コーパス609件（`~/Desktop/temp`）の読み書き:
  - **ラウンド5終了時（`771cd0f`）とバイト一致**
  - `main` との差は意図した分だけ: 読み取り 0件 / `to_kif` 0件 /
    `to_ki2` 1件（`bug_mega.kif`）/ `to_csa` 33件（GAP-023）
- 2000手の KIF / KI2 が読めることを実測（どちらも 2001 ノード）
- R8-10 の新しいテストは**変異を当てて落ちることを確認した**（2種とも）

## research/ へ書き戻したもの

- **D19 を新設**: 手数は2000まで確実に読めること。桁あふれは飽和させ、
  「気にしない」を「ファイルごと拒む」にしない。ply 0 だけは数を使う側が拒む
  （**ユーザー判断、2026-09-02**）

## 次ラウンドの対象

ラウンド8の修正。特に:

- `is_padding` が読み手のほぼ全ての入口に入ったこと（R8-01 / R8-04）の波及。
  盤面図の桁、持駒の枚数、消費時間の欄
- `not_move_line` が「1文字目を除いた残り」ではなく「行そのもの」を返すようになったこと
- `colon` を3つのキーワードに広げたこと（R8-05）で、`変化` `手合割` `持駒` を
  含む散文が新たに行として読まれていないか
- `branch_header_ply` の飽和と ply 0 の拒否（R8-02 / R8-03）
- `verify.sh` に増えた `cargo doc` が CI で通るか
