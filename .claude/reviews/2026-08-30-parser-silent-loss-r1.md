# レビュー parser-silent-loss ラウンド1

- 日付: 2026-08-30
- 範囲: `git diff main...HEAD`（PR #2 の13コミット。**マージ済み**）
- 対象コミット: `8be07b1`（マージコミットは `d7f53b3`）
- 走らせた reviewer: spec / rust / robustness / architecture / perf / comment / test（7並列）

**このラウンドの所見は次のブランチで直す。** PR #2 は既にマージされているので、
`main` から新しいブランチを切って対応する。

---

## comment-reviewer

### BLOCK

| # | 所見 | 場所 |
| --- | --- | --- |
| R2-01 | **公開 API の `# Panics` が、同じブランチで消した panic を documenting している。** `b6d94d4` が契約を書き `0c63ab4` が panic を消したが doc が残った。obs-shogi が「空なら panic する」と読めば、存在しない panic を防ぐコードが consumer 側に残る。`populate_relative` は同じ変更を受けたのに `# Panics` を書いていない — 記述が割れている | `src/normalizer.rs:231-234`, `:260-263` |
| R2-02 | **CSA writer が指し手行の無いノードでも `T<sec>` を書く。** `move_` も `special` も無く `time` だけ持つノードで、指し手行を伴わない時間行が出る。読み手は**直前の指し手の消費時間**として解釈する（R-CSA-007）ので、前の手の時間が上書きされる。KIF 側は `has_line` で抑止しており、**2つの writer が逆の判断をしている**。しかもコメントは「残るのは comments だけ」と書いてあり、この経路を検分させない | `src/converter/csa.rs:196-199` |

### HIGH

| # | 所見 | 場所 |
| --- | --- | --- |
| R2-03 | **入れ子の分岐で `replay` が本譜を再生してしまう。** `replay(rest, start_ply, pos)` は常に**本譜**を辿るので、分岐の中の分岐を書くとき**別の局面**が返る。doc は失敗モードを「`None` になって `relative` に落ちる」の1つしか挙げておらず、「有効だが別の盤面」という3つ目の状態を書いていない。結果、入れ子の変化で曖昧性解消が欠けるか逆に付き、**D2 が消したはずの故障が入れ子の分岐だけに残る**。回帰テストに入れ子の分岐が無い | `src/converter/ki2.rs:56-66, 94-99, 104-110, 214-216` |
| R2-04 | **要件 ID の誤引用。** R-KI2-006 は tsshogi に一言も触れておらず、「現物では KIF の語彙」と書いてある — 実装が書く `まで2手で後手の勝ち` は KIF の語彙ではない。`まで<N>手で` の綴りは `research/` のどこにも無く、`95-decisions.md` にも記録が無い。**決定ログを持つリポジトリで、決定が ID の陰に隠れている** | `src/jkf.rs:245-246`, `src/converter/ki2.rs:281` |
| R2-05 | **R-KIF-009 は開始局面の省略可否しか述べていない。** `後手番` という語も手番指定も出てこない。`research/*.md` 全体で `後手番` は要件 ID を持たない表と決定ログにしか無い。**この差分で最も価値が高いはずの引用（外部仕様の根拠）が空振りする**。同じ ID が回帰テストのコメントにも複写されている | `src/converter/kakinoki.rs:122-125, 245-248` |
| R2-06 | **変更の経緯がコメントに8箇所混入。** CLAUDE.md が全面的に禁じている。「以前は4箇所にあった」「以前は panic した」は現在の判断に使えず、消えた過去を検証できないぶん腐りやすい。置き換え案は元レビューに表で提示済み | `src/handicap.rs:7-11, 174`, `src/jkf.rs:190-192`, `src/normalizer.rs:481-483`, `src/converter/kakinoki.rs:200-201, 247-248`, `src/converter/ki2.rs:186-187, 333-334` |

### MEDIUM

| # | 所見 | 場所 |
| --- | --- | --- |
| R2-07 | **要件 ID が「半分だけ」正しい引用が3箇所。** R-CSA-006 は「玉は駒台へはいかない」としか書いておらず**成駒の規定は無い**／R-KIF-007 の語彙表に `HIKIWAKE` は無く、述べているのは `JISHOGI` の話／R-NOT-001 の実際の構成は `[手番記号]<到達地点><駒種>[動作・相対位置|打][成|不成]` で、**スロットは1つ、`打` は択一** | `src/normalizer.rs:102-105`, `src/csa.rs:139`, `src/jkf.rs:194-196`, `src/parser/ki2.rs:198-200` |
| R2-08 | **「longest first」の理由が偽。** 手合割16名と終局語10語に**接頭辞の関係にある組は1つも無い**。`tag` は入力の先頭に錨を張るので `香落ち` は `右香落ち` の**接尾辞**であって接頭辞ではない。挙げた実例そのものが成り立たない。一方 `from_ki2_phrase` は**本物の**順序依存を持つのに `入玉勝ち` が理由に挙がっていない | `src/handicap.rs:139-141`, `src/parser/kakinoki.rs:155`, `src/parser/kif.rs:28-30`, `src/jkf.rs:271-272` |
| R2-09 | **`handicap.rs` の doc が成り立たない2つの主張をしている。** (1)「新しいエントリは各所でコンパイルエラーになる」— `lookup` は `find` なので配列の長さしか検査されず、writer 3箇所は網羅 `match` を持たない。(2)「CSA の `PI` が使う順」— R-HC-003 は**並び順に規定は無い**と明記しており、この並びは `research/` 側の導出 | `src/handicap.rs:10-11, 24-28` |
| R2-10 | **「the only mapping」「the only place」が事実と違う。** KIF パーサは第3の表 `KIF_SPECIAL_WORDS` を持つ。`from_kif_word` に語を足しても配列に足さなければ永久に読めない — **この doc が防ごうとしている食い違いそのもの**。`normalizer.rs:481` も KI2 パーサが `tag("左上") → LU` を持つ以上、字義どおりには成り立たない | `src/jkf.rs:189-192`, `src/normalizer.rs:481` |
| R2-11 | **`line` が同じ関数の中で2つの意味に使われている。** `write_line` の line は「手順」、その中の `at_line_start` は「テキストの行」。さらにパーサ側は同じ概念を `move_run` と呼ぶ。writer が `write_line`、reader が `move_run` で往復の対応が読めない。`replay` も doc でだけ「main line」に限ると言っており、**R2-03 の故障はこの名前で隠れている** | `src/converter/ki2.rs:122, 129-131, 108`, `src/converter/kif.rs:75, 82`, `src/parser/ki2.rs:111` |

### 機械で強制できるもの

- `#![warn(clippy::missing_errors_doc)]` — `normalize_with_options` と `populate_relative` の `# Errors` 欠落が出る
- **変更の経緯の混入は hook で止められる。** 追加されたコメント行に対して
  `used to|no longer|now it is|drifted|came back|以前は|これまで|今回|に変更した|対応済み` を grep。
  **今回の8箇所すべてが掛かる**
- 要件 ID の**実在**確認 hook（内容の一致は人が見るしかない）
- `KIF_SPECIAL_WORDS` と `from_kif_word` の食い違いは、語の配列を `jkf.rs` に1本化すれば型で不可能になる

### comment-reviewer が見ていない範囲（申告）

- `research/tables/20-fork-merge.md` / `30-api-contract.md` は目次と grep のみ
- `research/90-gaps.md` は通読していない。GAP として既出の重複がありうる
- テストの実行はしていない。所見はすべて静的な読み
- **レビュー中に作業ツリーで `forks.iter().rev()` → `forks.iter()` の未コミット変更を観測**
  （他 reviewer の変異テスト）。範囲外として判定していない

---

## architecture-reviewer

所見は `8be07b1` を detach した clean worktree で再検証済み（レビュー中に他 reviewer が
作業ツリーを変異させていたため）。clean worktree では **53 passed / 0 failed**。

### BLOCK

| # | 所見 | 場所 |
| --- | --- | --- |
| R2-12 | **`ToUsi` の panic は消えておらず、依存クレートの中へ移っただけ。** `ToUsi` は外部 trait で、`to_usi_owned()` は default method。その中に `debug_assert_eq!(result, Ok(()))` がある。`Err` を返した瞬間に `shogi_core-0.1.5/src/to_usi.rs:18` で panic する。**`lib.rs` の `deny(clippy::expect_used)` の視界の外に出た分、悪化している。** release では `debug_assertions` が切れて**無言で空文字列**が返る（`.usi` を書けば中身が消える）。`examples/jkf2usi.rs:13` が実際に呼んでいる | `src/converter.rs:22` |
| R2-13 | **駒落ちで終局の主体が反転する。** `[Color::White, Color::Black][ply % 2]` が3箇所にあり、`handicap::side_to_move` を無視している。指し手の色は normalizer の反転プリパスが直すのに **`special` は偶奇で焼かれたまま補正されない**。実測: 香落ち + 投了 → `まで2手で後手の勝ち`（正しくは `先手の勝ち`）。反則勝ちは**反則を犯した側と勝った側が入れ替わって保存される** | `src/converter/ki2.rs:191`, `src/parser/ki2.rs:84`, `src/parser/kif.rs:128` |
| R2-14 | **新設した `後手番` 出力が obs-shogi の `patch_gote_start` と二重になり、`.ki2` の全指し手が消える。** 実測: `後手番` が2行出ると `parse_without_moves` は1行しか食わず、2行目が指し手列の先頭に残って `many0(single_move)` が0手で止まり、**残り入力は無言で捨てられて `Ok`**。`.kif` は無事、`.ki2` だけ全滅。**タグを上げた瞬間に後手番局面の `.ki2` 保存が全手数を失う** | `src/converter/kakinoki.rs:126-128` + obs-shogi `operations.rs:98-124` |

### HIGH

| # | 所見 | 場所 |
| --- | --- | --- |
| R2-15 | **`to_*_owned` の切り詰めがディスクに届く。** obs-shogi の `write_kifu_file_internal` は `normalize()` を呼ばずに `to_kif_owned()` の結果を `atomic_write` する。実測: `hands[0].FU = 21` で `"…先手の持駒：歩"` と**行の途中で切れた** `.kif` が保存され、成功が返る。**変更前は panic だったので可視性は退行**。`try_to_*_owned` を足したが**安全な方を opt-in にしたので既存の呼び出し側は自動的に危険な方に残る**。`to_csa_owned` は同じ入力で `P+00FU` を21回並べた**成功扱いの壊れた CSA** を返す（CSA 側は持駒枚数チェックが抜けている） | `src/converter/{kif,ki2,csa}.rs:30-34` |

### MEDIUM

| # | 所見 | 場所 |
| --- | --- | --- |
| R2-16 | **`handicap.rs` が `normalizer::HIRATE_BOARD` に依存し、`normalizer` が `handicap` を呼び返す。** doc は「その集合が知識の全体」と宣言しているのに、その「平手」は `normalizer` にある。**中心的なデータ表が処理モジュールに依存している**。`HIRATE_BOARD` を `handicap.rs` に移せば一方向になる | `src/handicap.rs:17,158-165`, `src/normalizer.rs:306-317` |
| R2-17 | **「表を1本化する」を `MoveSpecial` にだけ適用し、`Relative` は3箇所のまま。** しかも KI2 書き出しは **綴る → 綴りを enum に戻す → enum をまた綴る** を通っている。`converter/ki2.rs`（網羅 match）/ `parser/ki2.rs`（nom の `alt`、網羅チェック無し）/ `normalizer.rs`（`_ => None`）。**バリアントを足しても落ちるのは1箇所だけ**。`src/notation.rs` を作って持ち主を移せば `converter → normalizer` の辺も消える | `src/converter/ki2.rs:148-170`, `src/parser/ki2.rs:20-33`, `src/normalizer.rs:498-514` |
| R2-18 | **`MoveSpecial` の表を `jkf.rs` に置いた判断。** `ki2_phrase` は KI2 書き手のポリシーであってスキーマではない。同じブランチが同じ問題に2つの機構（新規 module / 型のメソッド）を使っている。**`ki2_phrase` だけ `_ =>` があり、1本化の狙いがこの1本で無効化されている** | `src/jkf.rs:197,251,265,293` |
| R2-19 | **追加した lint が、このブランチで実際に直したバグの種類を止めない。** `a5e583c` が直したのは `indexing_slicing`（35件）と `arithmetic_side_effects`（63件）の領域で、**その2つは有効になっていない**。`[profile.release] overflow-checks = true` も無い。`deny(clippy::panic)` は lib にしか掛からず、`examples/`・`benches/` と**依存クレートの中は対象外**（R2-12 がまさにそれ） | `src/lib.rs:10-19` |
| R2-20 | **`Handicap.removed` が裸の `(u8, u8)` なので、`from.rs` に到達不能な error 分岐ができた。** `const` 表で 1..=9 が保証されているのに型が表現していない。`Square` にすれば `ok_or` 分岐が型で消える | `src/handicap.rs:28`, `src/shogi_core/from.rs:113-117` |

### `pk2k` の重複（前回の指摘への回答）

**次のブランチでよい。ただし `Relative` の表より優先度は下。** `pk2k` と `into.rs` は
**どちらも `_ =>` の無い14アーム網羅 match** なので、バリアントが増えれば両方が
コンパイルエラーになる。**黙って腐る種類の重複ではない。**
対して `Relative` の3箇所は片方が `_ => None`、片方が nom の `alt` で、
片方だけ直しても誰も気づかない。同じ「重複」でも危険度が違う。

### フォークとしての差分（所見なし相当）

`Cargo.toml` は1行も変わっていない。`src/` 直下に増えたのは `handicap.rs` 1本で `private`。
公開面の変化は `try_to_*_owned` 3本（default method、非破壊）と、
4つの trait の**振る舞いの契約**変更だけ。`MoveSpecial` の5メソッドも
`starting_position` も `infer_relative_from_position` も `pub(crate)`。**意図どおりに絞れている。**

ただし `examples/` に追跡外の作業ファイルが5本残っている（R1-26 の再発）。

### 次のブランチの優先順位（architecture-reviewer の提案）

| # | 内容 | 理由 |
| --- | --- | --- |
| **0** | **obs-shogi 側の同時変更** — `patch_gote_start` 削除（5箇所）と `to_*_owned` → `try_to_*_owned` | **これをやらずにタグを上げると後手番局面の `.ki2` 保存が全手数を失う**（R2-14）。書く量は最小だが順序として最初 |
| **1** | **GAP-005 / R1-14 — 残り入力の厳格化** | このブランチは KI2 に**新しい出力を2つ**足した。綴りが1文字でも食い違うと `Err` ではなく無言の途中打ち切りになる。新しい `parser/ki2.rs:moves()` も**構造的に残り入力を許す**。**R1-27 のテスト（無言削除を正解として固定しているもの）を同じコミットで消すこと** |
| **2** | R2-13（駒落ちの終局主体の反転） | 1 と同じ箇所を触るので同一ブランチが安い。被害が最も直接的（勝敗が逆に保存される） |
| **3** | R1-19 — エラー型の構造化 | **1 を単独でやると「読めませんでした」が大量に出るのに何手目か分からず、厳格化そのものが実用に耐えないと判断されて巻き戻る。** 1 と 3 は同じ仕事の前半と後半 |
| **4** | R2-17 — `Relative` の1本化 + `notation.rs` 新設 | 1・2 でどうせ両ファイルを編集するのでついでにやると差分が小さい |
| **5** | R1-16 — 文字コード判別の統一 | 独立。`.KIF`（大文字）が全滅する件は見える割に修正が小さい |
| **6** | lint 追加（`indexing_slicing` / `arithmetic_side_effects` / `overflow-checks`）と `pk2k` | 1〜5 の後。今入れると差分がノイズで埋まる |

### 機械で強制できるもの（追加分）

- **`clippy.toml` の `disallowed-methods` に `shogi_core::ToUsi::to_usi_owned` を登録。**
  R2-12 はこれで機械的に落ちる。**外部クレートの default method は既存の lint の視界外**なので、
  これ以外に検出手段が無い
- `ki2_phrase` の `_ =>` を14アームに展開すれば、`MoveSpecial` 追加時のエラーが3箇所すべてで出る
- `handicap.rs:28` を `Square` にすれば `from.rs:115` の到達不能分岐が型で消える
- **`[Color::White, Color::Black][n % 2]` をリポジトリから消す。** 現状3箇所。
  grep できる形（`fn side_to_move_at_ply`）に集約する
- **往復テストを駒落ち16種 × 3形式に広げる。** `every_handicap_round_trips` は指し手0手なので
  R2-13 を1件も捕まえない。各手合割に「2手＋終局語」を足すだけで落ちる

---

## rust-reviewer

### BLOCK

| # | 所見 | 場所 |
| --- | --- | --- |
| R2-21 | **R2-03（入れ子分岐の `replay`）の実測。コーパス609件中17件で手が消える。消えた手は<br>すべて fork 深さ2以上**（深さ0/1の欠落は0件）。`bug_mega.kif` 16847→**12468** ノード、<br>`bug_big.kif` 714→**405**。分岐が本譜の局面で綴られ、接尾辞が付かない／間違って付く →<br>読み戻しで `AmbiguousMoveFrom` → `retain_mut` が分岐ごと捨てて `Ok`。<br>**直し方**: `replay` を消し、`write_line` が着手前に持っている `position` を `stack` に一緒に積む。<br>分岐ごとの再生 O(P) も O(1) になる | `src/converter/ki2.rs:96-98, 108-118, 214-217` |
| R2-22 | **KI2 の `まで…` 行の語句が未知だと、終局も*その後の 変化 ブロック全部*も黙って消える。**<br>`opt(end_of_game_line)` なので `Err` は `None` になり、**`まで` 行が未消費のまま残る**。<br>続く `while let Ok(..) = branch_header` は即座に抜け、残り入力が捨てられる。<br>実測: `まで4手で持将棋成立` + 変化2本 → ノード9が**5**、`Ok` が返る。<br>`不戦勝` / `不戦敗`（`from_kif_word` が意図的に返さない語）でも同じ。<br>**`まで` が通った時点で行は終局行だと確定しているので、必ず消費すること** | `src/parser/ki2.rs:72-97, 172-182` |

### HIGH

| # | 所見 | 場所 |
| --- | --- | --- |
| R2-23 | **`calculate_from` の `左`/`右` が「移動先との比較」になっており、移動元が移動先と同じ筋の<br>候補を落とす。** R-NOT-004 段階2 は候補どうしの比較。馬が 4三・5三 で 4四 へ動く局面で<br>`shogi_official_kifu` は `▲４四馬右` と正しく書くが、読み側は `右` を `relative_file < 4` で<br>絞るので 4三（`relative_file == 4`）が落ち、候補が空になって `AmbiguousMoveFrom([])`。<br>**これが往復で残っていた1件の正体。** `calculate_from` 自体は差分の外だが、<br>この差分が「局面から正しい接尾辞を書く」ようになって初めて実データで踏むようになった。<br>**直し方**: `froms` を `relative_file` で並べ、`L` は最大・`R` は最小を採る | `src/normalizer.rs:348, 350` |
| R2-24 | `to_*_owned` の切り詰めがディスクに届く（R2-15 と同じ。`#[deprecated]` を付けて<br>consumer の `-D warnings` に移行漏れを止めさせる案） | `src/converter/{kif,ki2,csa}.rs:30-34` |

### MEDIUM

| # | 所見 | 場所 |
| --- | --- | --- |
| R2-25 | `ki2_phrase` の `_` アーム（R2-18 と同じ）。**KIF 側は `const ALL: [MoveSpecial; 14]` で<br>数を固定しているのに KI2 側だけ守られていない**。`SpecialMatta`/`SpecialError` が `中断` に<br>なるのは `csa_word()` が両方を保持しているのと非対称で、CSA→KI2 で情報が落ちる | `src/jkf.rs:251-267` |
| R2-26 | **`まで<N>手で` の N を読んで捨てている。KI2 で打ち切りを検出できる唯一の情報。**<br>KI2 は指し手行に手数を持たないので、`many0(single_move)` が途中で止まったことを<br>N 以外に検出する手段が無い。しかも `side_to_move` が `v.len()` 由来なので、<br>手が落ちていると**反則勝ちの先後が反転して記録される** | `src/parser/ki2.rs:79-84` |
| R2-27 | **`From<csa::Position> for Initial` を `TryFrom` にしたのは公開面の破壊的変更。**<br>`mod csa` が private でも impl は下流から見える。obs-shogi は使っていないので実害は無いが、<br>**`Cargo.toml` が `0.3.0` のままなので版に現れない**。`try_to_*_owned` の追加と<br>`後手番` の出力追加と併せて `0.4.0` にすること | `src/csa.rs:66`, `Cargo.toml` |

### 差分で新しく書いた添字・減算・キャストの検分（所見なし）

`handicap.rs:board` の `file as usize - 1`、`[Color::White, Color::Black][ply % 2]` 3箇所、
`out[1..]`、`forks.len() - 1`、`ply - 1`、`num -= 10`、`csa.rs` の `b[file][rank]` を全部追い、
**いずれも到達しうる入力で範囲外にならないことを確認した**。
`unwrap`/`expect`/`panic!` の追加は `#[cfg(test)]` の中だけ。

### 機械で強制できるもの（追加分）

- **`#![deny(clippy::wildcard_enum_match_arm)]`** — `ki2_phrase` の `_` を止める。
  `Hand::slot` のような正当な `_` には `#[allow]` を明示することになり、
  **「網羅させたい `_`」と「潰してよい `_`」が区別できるようになる**
- **往復テストに深さ2の分岐を1つ足すだけで R2-21 は落ちる**
- `clippy::missing_panics_doc` — 嘘の `# Panics` の再発を止める
- **`Cargo.toml` の version と git タグの一致を hook で確認する**

---

## robustness-reviewer

**全所見を実行して確認済み。この差分が作った退行が2件ある。**

### BLOCK

| # | 所見 | 場所 |
| --- | --- | --- |
| R2-31 | **【退行】指し手列の途中に終局ノードがあると、KI2 がその後の指し手を `まで…` 行に流し込む。**<br>書き側は終局語の後に改行を書かず、続く手に半角スペースを付ける。読み側は `not_line_ending` で<br>**その行の残り全部**を飲む。実測: `1 ７六歩 / 2 中断 / 3 ３四歩 / 4 ２六歩 / 5 投了` の KIF が<br>KI2 往復で **5手 → 1手**。`main` では `to_ki2` が終局を書かなかったので3手残っていた。<br>**中断して再開した棋譜は実在する**（`中断` が途中に入る KIF） | `src/converter/ki2.rs:184-213`, `src/parser/ki2.rs:72-80` |
| R2-32 | **【退行】KIF 書き出しでコメント専用ノードが手数を1つ消費し、`変化：N手` の番号がずれる。**<br>`for (i, mf) in (index..).zip(moves)` の `i` は**行を出さないノードでも進む**のに、<br>`forks_stack.push((i, fork))` と `変化：{i}手` がその `i` を使う。実測: 手数が飛び（`1` の次が `3`）、<br>読み戻すと**分岐が消えて `Ok`**。`main` ではこの入力は `unreachable!()` で panic していた。<br>**panic を沈黙する分岐削除に置き換えている**（R1-27 と同じ形）。<br>**直し方**: 手数カウンタと配列添字を分ける | `src/converter/kif.rs:78-85, 140-150` |
| R2-33 | **`to_*_owned` の切り詰めが、保存も再読込も `Ok` で通る。** 持駒19枚以上の KIF はパースが `Ok`<br>（`checked_add` を通る）だが `write_kansuji` が `Err`。書き出しは `先手の持駒：歩` で切れ、<br>**指し手行も `手数----` 見出しも無い**。それを読むと **`Ok`（0手）**。<br>**「読める棋譜 → 保存 → 指し手が全部消えたファイル → 再読み込みも `Ok`」が<br>どこにもエラーを出さずに成立する** | `src/converter/{kif,ki2,csa}.rs:30-34` |
| R2-34 | R2-03/R2-21 の独立再現。**欠落7件、すべて深さ2以上**（深さ1は100%往復）。<br>`Keimuscat.kif` は深さ別 `{1:2, 2:2, 3:1, 4:1, 5:2}` → `{1:2, 2:2}` で**深さ3以上が全滅**。<br>「GAP-010 を消した代わりに、同じデータ喪失を入れ子分岐に付け替えただけ」 | `src/converter/ki2.rs:94-99` |

### HIGH

| # | 所見 | 場所 |
| --- | --- | --- |
| R2-35 | **`attach_branch` が置き場を見つけられない `変化：` を黙って捨てる。GAP-005 と同型の穴を<br>KI2 側に新設した。** 3箇所の `else { return; }` は戻り値も副作用も無く、呼び出し側も見ていない。<br>実測: `変化：` を**昇順**に並べた KI2（他ソフトの並べ方）で `変化：5手` が消える。<br>本譜2手に `変化：9手` を足すと丸ごと消えて `Ok`。<br>**GAP-005 を直すときに2箇所直す必要が生まれた** | `src/parser/ki2.rs:136-161, 176-179` |
| R2-36 | R2-22 の独立再現。`まで2手で先手の不戦敗` / `まで2手で中座` / `まで2手で引き分け` の<br>いずれでも**終局と後続の分岐が消えて `Ok`** | `src/parser/ki2.rs:72-93` |

### MEDIUM

| # | 所見 | 場所 |
| --- | --- | --- |
| R2-37 | **終局語テストが `assert!(got.is_some())` で、`assert_eq!` になっていない。**<br>CSA→KIF→CSA で `HIKIWAKE`→**`JISHOGI`**、`MATTA`→**`CHUDAN`**、`ERROR`→**`CHUDAN`**。<br>GAP-003 の7種のうち5種は直ったが3種は別の終局語に化けて `Ok`。<br>**R1-43 が指摘したのと同じ形の穴が、新しく書いたテストに再発している** | `src/parser/kif.rs:262-297` |
| R2-38 | **`%TIME_UP` / `%ILLEGAL_MOVE` / `%±ILLEGAL_ACTION` は CSA 入口で終局が消える。**<br>`src/csa.rs:196-215` にアームは揃っているが、依存の `csa::parse_csa` がその行を返さない<br>（**dead code**）。実測: `%TORYO`/`%JISHOGI`/`%MATTA` は3ノード、`%TIME_UP` は2ノード。<br>**キーワードによって落ちたり落ちなかったりする** | `src/csa.rs:196-215`, `src/parser.rs:33-40` |
| R2-39 | `▲４四馬右` の件（R2-23 と同じ）。**`research/20-notation.md` R-NOT-004 段階2 の記述だと<br>4三馬は `左` でも `右` でもなくなり、段階3でも決まらず「エラー」になってしまう。**<br>正典側に「一方が移動先と同筋のときの基準」を追記してから実装を合わせる | `src/normalizer.rs:347-371`, `research/20-notation.md` |

### 機械で強制できるもの（追加分）

- **往復テストを `data/tests/` 全件 × 3形式で回すと、BLOCK 1・2・4 は*この1本で全部落ちる*。**
  `data/tests/ki2/` は3件しかないので、再現入力（入れ子分岐・途中の `中断`・コメント専用ノード）を
  フィクスチャに足す
- `#[must_use]` + `#[deprecated]` を `to_*_owned` に。**危ない既定を呼んでいる箇所を
  コンパイラに列挙させるのが唯一の確実な検出手段**
- **`attach_branch` と `merge_forks` の戻り値を `Result` にして `#[must_use]`。**
  握りつぶす `return;` は型が `()` である限り lint では捕まらない
- 終局語テストを `assert_eq!` にし、**往復しない3種は `const LOSSY` の別リストに分ける**
- `.gitignore` に `/examples/_*.rs`（R1-26 の再発防止）

---

## test-reviewer

**変異36件を実際に当てて確認。13件が生き残った。** `src/` `data/` は元通り（`git status` 空）。

指示した5つの変異の結果:

| 変異 | 落ちたテスト |
| --- | --- |
| `ki2_phrase` の勝者/敗者入替 | ✅ `outcome_survives_a_ki2_round_trip` |
| `stack.pop()` → `remove(0)` | ✅ `branches_survive_a_ki2_round_trip` |
| `forks.iter().rev()` → `iter()` | ✅ 同上 |
| **`handicap.rs` の `removed` から1件削除** | ❌ **なし（53 passed）** |
| **`needs_disambiguation` を常に true** | ❌ **なし**（0.6s → 13.3s になるだけ） |
| `後手番` の条件反転 | ✅ `side_to_move_survives_a_round_trip` |

### BLOCK

| # | 所見 | 場所 |
| --- | --- | --- |
| R2-41 | **`to_usi_reports_an_unreplayable_record` は何も検証していない。** 入力は `parse_jkf_str` を通り<br>`Position::try_from` も成功し、`to_usi` は `Ok` と `"startpos moves 9i1a"` を返す。<br>**`map_err` を `expect` に戻す変異が素通し。** `to_usi` の panic 除去には回帰テストが存在しない | `src/converter.rs:79-90` |
| R2-42 | **手合割表に外部の錨が無い。期待値を検証対象の表から導出している。**<br>三枚落ちの `KY_LEFT` 削除 → **53 passed**。左五枚落ちの `KE_LEFT`→`KE_RIGHT`（**R-HC-003 が<br>「取り違えないこと」と名指しで警告している箇所**）→ **53 passed**。`kif_name` の改変 → **53 passed**。<br>今この表を守っているのは既存 golden（平手/香/角/飛/2/4/6）だけで、**GAP-001 で足した5 preset には<br>golden も `#[test]` も無い。panic の代わりに黙って別の局面が返る** | `src/handicap.rs:60-137, 176-217` |
| R2-43 | **`to_kif` / `to_csa` の終局語書き出しに1本もテストが無い。**<br>`kif_outcome_words_are_written_back` は名前に反して **`to_kif` を一度も呼ばない**。<br>`converter/kif.rs:111` を `let word = "中断";` に（＝GAP-003 そのもの）→ **53 passed**。<br>`converter/csa.rs:193` を `"CHUDAN"` 固定 → **53 passed**。`入玉勝ち`→`投了` → **53 passed**。<br>**直したはずのものを元に戻しても誰も気付かない。**<br>`data/tests/` に**書き出しの golden が1つも無い**のが共通原因 | `src/converter/kif.rs:111`, `src/converter/csa.rs:193`, `src/parser/kif.rs:262-297` |

### HIGH

| # | 所見 | 場所 |
| --- | --- | --- |
| R2-44 | **`後手番` は片方向しか固定していない。無条件出力にしても通る。** 先手番の任意局面を保存すると<br>`後手番` が付き、次に開くと手番が逆になる。**修正しようとしたバグと同じ失敗が逆向きに開いたまま** | `src/converter/kakinoki.rs:126-128, 250-266` |
| R2-45 | **KI2 の入れ子分岐が未検証。`attach_branch` の `path` 機構を丸ごと無効にしても通る。**<br>`path.retain(...)` → `path.clear()` で **53 passed**。実際の挙動は変わっており、<br>入れ子だった分岐が本譜の3手目にぶら下がる（`変化：` の本数は同じなので `count()` も通る） | `src/parser/ki2.rs:130-166` |
| R2-46 | **CSA の `PI` ブロックの手番行が未検証。** `write_color(Black)`（常に `+`）→ **53 passed**。<br>駒落ち16種すべてが先手番として書かれ、下手が指すべき初手を上手が指す局面になる | `src/converter/csa.rs:160` |

### MEDIUM

| # | 所見 | 場所 |
| --- | --- | --- |
| R2-47 | **`write_kansuji` の `>18` ガードに回帰テストが無い。テストが選んだ値 21 が判別しない。**<br>21 はガードが無くても `KANSUJI.get(10)` が `None` になる。**効くのは 19 と 20 だけ**。<br>実測（ガード無し）: `FU=19` → `先手の持駒：歩十九` → **reparse `Ok` で持駒0枚**。<br>doc 自身が「自分のパーサで読み直せない」と書いた境界が押さえられていない | `src/converter/kakinoki.rs:29-31, 233-244` |
| R2-48 | **算術オーバーフローの修正3箇所すべてに回帰テストが無い。**<br>`.min(u8::MAX)` を外して `as u8` → **53 passed**。`checked_add` → `wrapping_add` → **53 passed**。<br>`a5e583c` が直したものはそのまま元に戻せる | `src/normalizer.rs:141`, `src/parser/kakinoki.rs:47, 146` |
| R2-49 | `outcome_survives_a_ki2_round_trip` は**14種中9種**しか回していない。<br>`MATTA`/`ERROR`→`Chudan`、`HIKIWAKE`→`Jishogi` の**非対称が意図か事故かをテストが言っていない** | `src/converter/ki2.rs:283-335` |
| R2-50 | **`不成` を先に試す理由が偽**（`tag` は前方一致なので `成` を先にしても等価）。<br>正しいのは `normalizer.rs` の `strip_suffix` の方だけ。**根拠の写し間違い** | `src/parser/ki2.rs:36-37` |

### 追加14本のうち、名前と実態がずれているもの

| テスト | ずれ |
| --- | --- |
| `outcome_survives_a_ki2_round_trip` | 14種中9種 |
| `branches_survive_a_ki2_round_trip` | **入れ子分岐を検証していない** |
| `side_to_move_survives_a_round_trip` | **先手番側が無い** |
| `every_handicap_round_trips` | **表の中身は検証していない**（期待値が表から導出） |
| `kif_outcome_words_are_written_back` | **`to_kif` を呼ばない。同一性も見ない** |
| `unspellable_records_are_errors` | `write_kansuji` の `>18` は選んだ値が判別しない |
| `degenerate_records_do_not_panic` | 出力を一切見ない（`let _ =`）。panic のみ |
| `to_usi_reports_an_unreplayable_record` | **何も検証していない** |

**この差分で最も強いテストは `kif_outcome_words`**（`反則勝ち` の符号方向を含む12語の読み）。

### golden の貼り直し（該当なし）

`data/tests/ki2/4.json` の `{"special":"TORYO"}` は仕様から導出できる。
`まで102手で先手の勝ち`、最終手は102手目の Black なので終局は103手目＝White の手番、
`勝ち` 判定で `SpecialToryo`。`moves` の長さも `1+102+1 = 104` で整合。

ただし根本問題として **`data/tests/` に writer の golden が1つも無い**。
`parser::tests::*` は読むだけ、`shogi_core::tests::jkf_to_jkf` は `Position` 経由で
`special` も `initial.data` の細部も落ちる。**書き出し側が golden で守られていない**のが
BLOCK 3件の共通原因。

### 機械で強制できるもの（追加分）

- **`#[test]` 内に `assert` が1つも無いことを検出する grep hook**。`to_usi_reports_an_unreplayable_record` が該当
- **`assert!(x.is_some())` / `assert!(x.is_ok())` だけのアサーションも同種の grep で拾える**。
  `is_err()` は正当なので `is_some()` / `is_ok()` に限る
- `const ALL: [MoveSpecial; 14]` は variant 追加でコンパイルエラーにならない。
  **`match` で全 variant を潰して `&[..]` を返す関数にすれば型エラーになる**

---

## spec-reviewer

### BLOCK

| # | 所見 | 場所 |
| --- | --- | --- |
| R2-51 | 入れ子分岐（R2-03/21/34 と同じ）の**コーパス全体での定量**:<br>**分岐 1735 → 1372（-363、21%）／指し手 87489 → 82315（-5174、5.9%）**<br>データを失うファイル 16/609、読み直せない 1。`Sab3.kif` は4段の入れ子が `ply15` で切れ、<br>その下の `ply25`/`ply45` も道連れ | `src/converter/ki2.rs:92-100, 108-118` |
| R2-52 | **駒落ちの終局手番。tsshogi と実際に突き合わせた結果、こちらが誤り。**<br>`手合割：香落ち / 1 ３四歩(33) / 2 投了` →<br>こちら「**まで1手で先手の勝ち**」／tsshogi「まで1手で後手の勝ち」。<br>下手が投了しているのに「先手の勝ち」と書かれる。`parser/kif.rs:128` も同じ偶奇で<br>`反則勝ち` の向きを決めるので **JKF の値まで反転する** | `src/converter/ki2.rs:191`, `src/parser/ki2.rs:84`, `src/parser/kif.rs:128` |
| R2-53 | **`ki2_phrase` が `+ILLEGAL_ACTION` と `-ILLEGAL_ACTION` を区別せず、同じ文字列を出す。**<br>KI2 の綴りは勝者を明示できるのでバリアントから一意に決まるのに、`side_to_move` から作っている。<br>実測: `+ILLEGAL_ACTION` を保存して開き直すと **`-ILLEGAL_ACTION` になる**。CSA に書き戻すと<br>`%-ILLEGAL_ACTION`。**反則した側が入れ替わる** | `src/jkf.rs:261-263` |
| R2-54 | **書き手と読み手で 左/右 の規則が違う。** 読み手は「移動元の筋 vs **到達点の筋**」を比べるが、<br>`shogi_official_kifu` の `disambiguation::run_file` は**2枚の候補どうし**を比べる（桂・角・馬・飛・龍）。<br>馬・龍は到達点と同じ筋から動けるので、`from.file == to.file` で読み手の条件が誰も満たさなくなる。<br>**読み手側だけの誤りなので、tsshogi や ShogiHome が書いた既存の `.ki2` も同じ理由で読めない**<br>（tsshogi は `▲４四馬右` を102ノードで読める＝実測） | `src/normalizer.rs:348, 350` |

### 検証済み（所見なし）— 実測で裏を取ったもの

- **`src/handicap.rs` の手合割16種を R-HC-003 と1枡ずつ突き合わせた。** 落とす駒・左右の向き<br>（`KY_LEFT=(1,1)` / `KE_RIGHT=(8,1)` / `GI_RIGHT=(7,1)` / `KI_RIGHT=(6,1)`）・`PI` 文字列すべて一致。<br>**`5`/`5_L`、`7_R`/`7_L` の取り違えも無い。** `side_to_move` も R-HC-001 どおり
- **`needs_disambiguation` と `disambiguate` が集合として等価。** コーパス **86,906手で不一致0**。<br>D2 の前提（R1-36）は成立している
- **KI2 の `変化：` ブロックの並び順と接続規則が tsshogi と一致。** 書き側の LIFO は tsshogi の DFS と<br>同じ順序を出し、読み側の `path` 切り詰めは `record.goto(num-1)` と同じ意味。<br>兄弟2本×各々入れ子、同手数と異手数の混在を手で追って確認
- `single_move` の読み順（相対→成/不成）と `不成` の先行判定
- KIF 終局語の読み書きの対称性（`KIF_SPECIAL_WORDS` ⊇ `kif_word` の出す語）

---

## perf-reviewer

`main`(83bf138) と `HEAD`(8be07b1) を別 worktree に展開して release ビルドで比較。

### **KIF パースは遅くなっていない。むしろ速くなった**

| bench | main | HEAD |
| --- | --- | --- |
| `parse_kif/bug_mega` | 40.3 ms | **28.2 ms** |
| `parse_kif/bug_big` | 2.22 ms | **1.24 ms** |
| 609件一括 | 210 ms | **165 ms** |

### BLOCK / HIGH

| # | 所見 | 場所 |
| --- | --- | --- |
| R2-55 | 入れ子分岐の**書き出し側から見た定量**: `write_line` に計数器を入れると<br>`bug_mega.kif` は 16846手中 **12901手（77%）が局面を失っている**。<br>正しく辿れば曖昧性解消が必要な手は **233手**だが、現状 **37手**しか書けていない<br>（**196手が `左`/`右`/`上`/`引` を失って出力されている**）。<br>**プロトタイプ修正を実装して実測: `differing_moves=0` で往復が完全一致**<br>（bug_big 713→713、bug_mega 16846→16846）。差分は3箇所20行以下 | `src/converter/ki2.rs:92-99` |
| R2-56 | `replay` は O(分岐数 × 分岐開始手数)。bug_mega で **48,320回の `make_move`、1.45 ms**<br>（`to_ki2` 全体の16%）。**`populate_relative_moves` は同じ木の走査を再生なしで正しくやっている**<br>のに、`ki2.rs` だけが再生でやり直している。上の修正で 1,346回の `clone` に置き換わる | `src/converter/ki2.rs:96-99, 108-119` |
| R2-57 | **局面が正しくなると `display_single_move_kansuji` が支配的になる**（150〜300µs/回）。<br>bug_mega の `to_ki2` は **8.86 ms → 44.4 ms**（プロトタイプ実測）。<br>**`needs_disambiguation` は既に候補集合を走査しているので、2個以上見つけた時点で候補マスを集め、<br>R-NOT-004 の段階1→2→3 を直接判定すれば描画も文字列の逆読みも要らない。**<br>見込み 44.4 ms → 約7 ms | `src/normalizer.rs:494`, `:536-568` |
| R2-58 | **`benches/parse.rs` がクリーンチェックアウトで動かない**（`test/` は `.gitignore`）。<br>この差分は `to_ki2` を「無視できるコスト」から「1件45ms」に変えたのに、測る手段がリポジトリに無い | `benches/parse.rs:19-21` |

### 検証済み（所見なし）— 依頼した項目

| 項目 | 実測 |
| --- | --- |
| `needs_disambiguation` の81マス走査 | **0.36〜0.41 µs/手**。R1-36 の採用で 4.88s → 6.09ms（**800×**）になった側の数字 |
| `names_longest_first()` の Vec+sort | 1827パースに対して**1836回**＝`手合割：` 行1本につき1回。測定不能 |
| `handicap::lookup` の線形探索 | パース中の呼び出し**0回**。書き出しで1ファイル数回、16要素 |
| `ki2_phrase` の `String` | 終局ノード1個につき1回 |
| `normalize_initial` の16手合割ループ | 盤面付き棋譜のみ、1棋譜1回。パース時間に現れず |
