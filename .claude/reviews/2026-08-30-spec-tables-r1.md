# レビュー spec-tables ラウンド1

- 日付: 2026-08-30
- 範囲: `src/` 全体（状態遷移表 `research/tables/` の作成に伴う全面レビュー）
- 対象コミット: `83bf138`（ブランチ `fix/parser-silent-loss`、`src/` は未変更）
- 走らせた reviewer: architecture / robustness / perf / rust / test（5並列）

**注記**: reviewer が同時に `examples/` へ probe を置き、`src/normalizer.rs` に変異を当てていた。
所見の行番号は各 reviewer が HEAD で取り直したもの。**`src/` の作業ツリーが元に戻っているかは
全 reviewer 終了後に `git status` で確認すること。**

---

## 所見（深刻度順）

### BLOCK

| # | 所見 | 場所 | reviewer |
| --- | --- | --- | --- |
| R1-01 | **KI2 パーサが `相対→成` の順を読めない。** `opt(promote)` が `opt(relative)` の前にある。`▲３三銀右成` を読むと `成` が余って打ち切り、しかも `promote=None` → normalize が `Some(false)`（不成）に確定。**成る手が不成に化けて盤面が恒久的にずれる。** 往復で `▲３三銀右不成` が出る | `src/parser/ki2.rs:18-33` / `src/converter/ki2.rs:66-90` | robustness |
| R1-02 | **`to_ki2` が `special`（終局）を1行も書かない。** コーパス実測: 終局を持つ583件のうち **559件が KI2 保存で投了/千日手/切れ負け/詰みを失う**（95.7%） | `src/converter/ki2.rs:53-100` | robustness |
| R1-03 | **`to_ki2` が `forks` を丸ごと捨てる**（`to_kif` は書く）。`.ki2` で保存すると分岐が全部消えて成功が返る | `src/converter/ki2.rs:42-106` | architecture |
| R1-04 | **盤面付き KIF/KI2 の書き出しが `StateFormat.color` を落とす。** 後手番の任意局面を保存すると `後手番` 行が無く、読み直すと1手目で `Invalid move`。**obs-shogi はこれを `patch_gote_start` の文字列手術で埋めている**（コメントに「converter crate が後手番情報を出力しないための補正」と明記） | `src/converter/kakinoki.rs:63-101` | architecture |
| R1-05 | **`parse_jkf_str` が `moves: []` で panic。** `self.moves[1..]` が範囲外。tsshogi が空棋譜を書けば普通に出る形 | `src/normalizer.rs:302`, `:335` | robustness |
| R1-06 | **CSA の持駒・盤面変換に入力依存の panic が3種。** `P+00OU` → `unreachable!()`、歩19枚 → 減算オーバーフロー、`P+09FU`（筋0）→ オーバーフロー | `src/csa.rs:94,101,109,119` / `src/normalizer.rs:212,217` | robustness |
| R1-07 | **CSA で `PI` と `P+55FU` を併用すると盤面が消える。** `PI` の40枚が全部消えて足した歩だけ残る。`Ok` が返る | `src/csa.rs:77-104` | robustness |
| R1-08 | **`parse_csa_str` も残り入力を捨てる。** セパレータ `/` 以降の2局目が消える。読めない行以降も消える（GAP-005 と同型） | `src/parser.rs:33-40` | robustness |
| R1-09 | **`手合割：香落`（「ち」なし）が黙って平手になる**うえ header に紛れ込む。駒落ちの手番反転が発動せず、エラーは無関係な手を指す | `src/parser/kakinoki.rs:146-172, 219-228, 359` | robustness |
| R1-10 | **末尾に改行が無い KIF は最後の1手が消える。** 最後の手はたいてい `投了` なので勝敗が消える | `src/parser/kif.rs:103-112` | robustness |
| R1-11 | **駒落ち5種の panic 箇所が4つ目にあった。** `converter/kakinoki.rs:117` の `_ => unimplemented!()`。`.kif`/`.ki2` 保存で落ちる。**`research/tables/40-vocabulary.md` の「KIF 書き」列が誤って ✅ だった → 修正済み** | `src/converter/kakinoki.rs:105-118` | architecture |
| R1-12 | **`normalizer` の駒落ち色反転プリパスが本譜にしか効かない。** `correct_color=true` では冗長、`correct_color=false`（`normalize()` = obs-shogi が呼ぶ唯一の入口）では本譜だけ迂回し、分岐は `InvalidColor` → `retain_mut` で無言削除。**この関数が効く唯一の経路が、分岐を消す経路** | `src/normalizer.rs:277-293`, `:563-567` | architecture |
| R1-13 | **公開 trait の契約が守られていない。** `moves: []` / コメント専用ノードで `to_kif` / `to_ki2` / `to_csa` が全部 panic。doc は「`Err` は sink への書き込み失敗のときだけ」と書いてある。**write 経路には `catch_unwind` が無い** | `src/converter/{kif,ki2,csa}.rs`, `src/converter.rs:18` | architecture |

### HIGH

| # | 所見 | 場所 | reviewer |
| --- | --- | --- | --- |
| R1-14 | **KI2 パーサが空行・終局語・`変化：` で打ち切る。** `research/31-ki2.md` R-KI2-002 に載っている**柿木の公開仕様の実例そのもの**が11手→6手になる。KI2 は手数が書かれていないので**消えた量を突き合わせる手段が無い** | `src/parser/ki2.rs:64-81`, `src/parser.rs:137-148` | robustness |
| R1-15 | **`AL` が既指定の持駒を差し引かない。** 玉2枚 + `P+00KI` + `P-00AL` で金が合計5枚になる。詰将棋 CSA の定石形が壊れる | `src/csa.rs:105-123` | robustness |
| R1-16 | **文字コード判別が3形式でばらばら。** Shift_JIS の `.csa` が `ParseError::Io`（「壊れている」と区別できない）、UTF-8 の `.ki2` は再試行なし、`.KIF`（大文字）は `FileExtension` で全滅 | `src/parser.rs:21-26, 51-84, 115-130` | robustness |
| R1-17 | **行き所のない駒を含む棋譜が読めない**（R-REQ-008 違反）。二歩と自殺手は通るのに、9一歩打だけ `Err` で棋譜全体が落ちる | `src/normalizer.rs:576-578` | robustness |
| R1-18 | **`ParseError::Kif` / `Ki2` は構造上1度も返らない（dead code）。** 失敗しうる分岐が無い。結果 `parse_kif_file` の Shift_JIS→UTF-8 再試行アームも**到達しない** | `src/error.rs:45-75`, `src/parser.rs:75-81` | robustness |
| R1-19 | **`NormalizeError` が `String` に潰される。** `AmbiguousMoveFrom`（棋譜の問題）/ `MakeMoveFailed`（反則手）/ `Convert`（座標破損）を呼び出し側が区別できない。**位置情報（何手目・何行目・どの分岐）がどこにも無い**。`Square(61)` は shogi_core の内部添字で将棋の座標ですらない | `src/error.rs:72-74`, `src/parser.rs:96` | robustness |
| R1-20 | **CSA の消費時間 `u64` オーバーフローで `csa` クレート内の `unwrap()` が panic** | `csa-1.0.2/src/parser/game.rs:40` 経由 | robustness |
| R1-21 | **正規化 API が4本に分裂し、うち3本を obs-shogi は呼んでいない。** `normalize()` の3箇所のみ。`normalize_with_color_correction` と `normalize` には `///` が1行も無い | `src/normalizer.rs:271,311,318,326` | architecture |
| R1-22 | **手合割の対応表が4箇所（+consumer に5箇所目）。** 終局語も4箇所 | 上記 | architecture |

### MEDIUM

| # | 所見 | 場所 | reviewer |
| --- | --- | --- | --- |
| R1-23 | 空ファイル・CR 単独改行の KIF が「平手0手の正常な棋譜」として `Ok` | `src/parser/kakinoki.rs:336-370` | robustness |
| R1-24 | CSA の消費時間が256時間を超えると `as u8` で黙って切り捨て（1000時間 → 232時間） | `src/csa.rs:250-261` | robustness |
| R1-25 | `normalizer.rs` が `jkf` 型のヘルパ置き場になり、`parser`/`converter` から余計な依存辺。`pk2k` は `into.rs` と14アーム完全重複 | `src/normalizer.rs:157-227, 242-259` | architecture |
| R1-26 | **調査用 probe が `examples/` に残り、`cargo fmt --all --check` が現在落ちている** | `examples/zz_*.rs` | architecture |

### rust-reviewer の追加分（上と重複しないもの）

| # | 深刻度 | 所見 | 場所 |
| --- | --- | --- | --- |
| R1-27 | **BLOCK** | **`merge_forks` の HEAD の「防御」は panic を無言の分岐削除に置き換えただけ。しかも `#[test] parse_entire_moves_malformed_fork_index` が `must NOT be merged in` と書いて無言削除を正解として固定している。** 後から直すと「テストが落ちる＝退行」に見える | `src/parser/kif.rs:170-181`, テスト `:589-613` |
| R1-28 | **BLOCK** | **`ToUsi for JsonKifuFormat` の `expect` が入力依存で panic。** 反則手を含む棋譜（R-RULE-002 で正常な入力）で落ちる | `src/converter.rs:18` |
| R1-29 | **BLOCK** | **書き出し側が入力の `u8` をそのまま添字・減算に使う。** `to.x=0` で減算オーバーフロー、持駒 `FU=21` で添字外。**`FU=20` は panic せず `歩十十` を出力し、自分のパーサ（十八まで）で読み直せない** | `src/converter/kakinoki.rs:9,13-19`, `src/converter/kif.rs:124` |
| R1-30 | **BLOCK** | **パーサの `u8` 算術が溢れる。** 持駒 `歩十八` ×20、消費時間の累積。**release では `overflow-checks` が切れているので panic せずラップし、枚数と時間が黙って別の値になる** | `src/parser/kakinoki.rs:130-136`, `src/normalizer.rs:230-232` |
| R1-31 | MEDIUM | **公開 enum に `#[non_exhaustive]` が無い。** ただし付けるべきは**エラー型だけ**。`Preset`/`MoveSpecial`/`Relative` は JKF スキーマが確定した閉じた集合なので**逆に付けず**、`_` を禁じて網羅性をコンパイラに見せる側 | `src/error.rs:7,24,47` |
| R1-32 | MEDIUM | **網羅性を型に見てほしい箇所で `_` を使っている5箇所。** とくに `normalizer.rs:382` の `_ => *initial` があるせいで、GAP-001 を直しても**コンパイラが「ここも直せ」と言ってくれない** | `src/normalizer.rs:212,224,382,435`, `src/converter/kif.rs:118` |
| R1-33 | MEDIUM | `ToKif`/`ToKi2`/`ToCsa` の doc が **panic について嘘をついている**（「`Err` は sink 失敗のときだけ」）。`missing_docs` 警告は**ちょうど2件**（`normalize` と `normalize_with_color_correction`）で、今すぐ入れられる | `src/converter/{kif,ki2,csa}.rs:9-10` |

### 分岐の再帰深さ — 実測（`90-gaps.md` の「未調査」を埋めた）

| ビルド / スタック | 通る | 落ちる |
| --- | --- | --- |
| release / 2 MiB（tokio worker 既定） | 16,000 | **20,000 → `stack overflow` で `abort`** |
| release / 4 MiB | 20,000 | — |

1レベルあたり release 約100バイト。**コーパスの実際の最大深さは 8**（`bug_mega.kif`。1346分岐は全部浅い兄弟）。
実棋譜で踏む余地は無いので **BLOCK ではない**。ただし**スタック溢れは `catch_unwind` で受けられず `abort` する**。
上限定数（例 256）を置いて `Err` にするのが安い。

### lint を入れたときの実測件数（`--lib`）

| lint | 件数 | 止まる所見 |
| --- | --- | --- |
| `#![warn(missing_docs)]` | **2** | R1-33。**今すぐ入れられる** |
| `#![deny(clippy::unimplemented, clippy::todo)]` | 3 | GAP-001 の3箇所が全部 |
| `#![deny(clippy::unreachable)]` | 4 | R1-06, R1-13 |
| `#![deny(clippy::unwrap_used, clippy::expect_used)]` | 7 | R1-28 |
| `#![warn(clippy::indexing_slicing)]` | 35 | R1-05, R1-29 |
| `#![warn(clippy::arithmetic_side_effects)]` | 63 | R1-30。モジュール単位で段階導入 |

`[profile.release] overflow-checks = true` を入れると R1-30 の「release では黙ってラップ」が検出可能になる。

### perf-reviewer（すべて実測。出力は609件で完全一致を確認済み）

| # | 深刻度 | 所見 | 効果 |
| --- | --- | --- | --- |
| R1-34 | **HIGH** | **`VerboseError` が成功パスで手ごとに28回ヒープ確保する。** `alt` の失敗分岐は KIF では成功パスの一部（`move_special` の8タグ、`piece_kind` の14分岐…）。パーサを `E: ParseError<&str>` でジェネリック化し、**失敗したときだけ `VerboseError` で再パース**すればエラーメッセージの質は落ちない | コーパス609件 **192ms → 42ms（4.6×）**。`bug_mega` は 468,753 → 24,570 allocs。**出力は609件すべて一致（不一致0）** |
| R1-35 | **HIGH** | **`relative` 推定が KI2/CSA/JKF 経路では丸ごと残っている。** R-REQ-006 の遅延化は KIF にしか適用されていない | `.ki2` 168手で normalize **32.6ms → 0.37ms**。normalize 時間の **86〜88倍**が `relative` 推定。120手×1000件で **23秒** |
| R1-36 | **HIGH** | **`relative` 推定が1手ごとに 15,390 手の全列挙をする。** `display_single_move_kansuji` → `disambiguate` → `all_valid_moves`。**81マス走査で候補を数え、Normal で ≤1 / Drop で空なら `None` 確定**という早期打ち切りが、コーパス **86,906手すべてで現行と一致（不一致0）** | 1手 **357µs → 期待11µs（約32×）**。81マス走査は 0.5µs で **653×安い** |
| R1-37 | MEDIUM | `many0(move_comment_line)` が**指し手行ごとに使わない `Vec` を確保**（nom7 の `many0` は無条件に `with_capacity(4)`）。`ki2.rs:36` は既に `opt(many1(..))` で正しい | `bug_mega` **7.4ms → 6.1ms（18%）**、24,570 → 7,767 allocs |
| R1-38 | MEDIUM | `parser/kakinoki.rs:230` が所有済みの `String` を `.iter()` + `to_owned()` で**ヘッダ行ごとに2本複製** | `into_iter()` にすれば0 |

**重要な注意（R1-36）**: 早期打ち切りに `LiteLegalityChecker::normal_to_candidates` を使っては**いけない**。
そちらはピン考慮の `is_legal_partial_lite` で、`disambiguate` の prelegality と食い違い、
**86,906手中34手（0.04%）で綴りが変わる**。`shogi_legality_lite::prelegality::is_valid` を使うこと。
R-REQ-007「速さのために正しさを削らない」に直結する。

### D2 差し替え案（`to_ki2` の自給化）に付く前提条件 — R1-39

`populate_relative` の実測コスト:

| 対象 | 手数 | 所要 |
| --- | --- | --- |
| 130手の対局 | 133 | **28〜42 ms** |
| `bug_mega.kif` | 16846＋1346分岐 | **12.0 秒** |

`to_ki2` を自給化しても、`display_single_move_write_kansuji` は同じ `disambiguate` を通るので
**1手 190µs のコストはそのまま乗る。** 130手の保存で約25ms（許容）だが、
分岐の多い研究ファイルでは Tauri コマンドが数秒ブロックする。

**→ R1-36 の早期打ち切りを先に入れる。** 曖昧でない手（97.1%）は
外部関数を呼ばずに自前で綴りを組み立て、曖昧な手だけ `display_single_move_write_kansuji` に回す。
そうすれば自給化しても保存は速いままになる。**R1-36 は D2 の前提であって、独立した最適化ではない。**

### test-reviewer（変異を実際に当てて確認。壊した変更は全て復元済み）

**GAP-001〜011 の11件すべてに回帰テストが無い。** 変異で確認した根拠付き。

| # | 深刻度 | 所見 |
| --- | --- | --- |
| R1-40 | **HIGH** | **`relative` 推定の13アームが `normalizer.rs:533` と `:604` に一字一句同じで2つある。** `research/90-gaps.md` GAP-010 は `:531` しか指していない。**`:533` だけ直しても obs-shogi の経路（KIF → `populate_relative` → `to_ki2`）は `:604` を通るので直らない。** 実測で確認済み |
| R1-41 | **HIGH** | **GAP-010 を直しても 39 passed のまま。** golden JSON で `promote` を持つ97手のうち `relative` を持つものが **0手**。曖昧な成る手のフィクスチャが1件も無い。**修正を revert しても誰も気づかない** |
| R1-42 | **HIGH** | **`to_ki2` の相対表記・成/不成の変異が全部生き残る。** `relative` を一切書かないようにしても 39 passed。テストの3手は全て `relative: None` / `promote: None` で、13分岐に1度も入らない |
| R1-43 | **HIGH** | KIF 終局語は読み側で「投了/中断/詰み」しか固定されず、**書き側は1つも固定されていない**。`SpecialToryo => "投了"` を `"中断"` に変えても 39 passed |
| R1-44 | **HIGH** | **公開 API に一度も呼ばれない入口・出口が並ぶ。** `.kifu`→`SHIFT_JIS`、`.ki2u`→`SHIFT_JIS`、`KYR` の `PI` 文字列変更、CSA の手番反転 — **全部 SURVIVED**。`data/tests/` の `.kifu` 8件 / `.ki2u` 2件は**どこからも参照されていない**（「UTF-8 もテストされている」という誤った安心） |
| R1-45 | MEDIUM | **兄弟分岐2本以上のフィクスチャが1つも無い。** `v.push` → `v.insert(0, …)`（順序反転）が SURVIVED。GAP-009 を直しても守るテストが無い |
| R1-46 | MEDIUM | **`parse_*_str` に不正入力を渡して `Err` を期待するテストが0本。** `is_err()` の12箇所は全部 nom 下位パーサに空文字列を渡すだけ。GAP-005/007/008 は「`Err` になるべきものが `Ok`」なので、**直しても退行を検出できない** |
| R1-47 | MEDIUM | 手合割17種のうち **8種がどのフィクスチャにも無い**（`KY_R` `3` `5` `5_L` `7_L` `7_R` `8` `10`）。GAP-001 が緑なのは安全だからではなくデータが無いから |
| R1-48 | MEDIUM | **`benches/parse.rs` が `.gitignore` された `test/` に依存**し、clean checkout で panic する。`8f2a279 perf: KIF パース 100× 高速化` の主張が**誰にも再現できない** |

### 追加すべきテスト（最小入力と期待値まで）

**T1（最優先、GAP-010）** — 7一と3一の角が両方5三に届く盤面を作り、4通りを固定する。

| 指し手行 | 期待 `relative` | 期待 KI2 | 現在の実測 |
| --- | --- | --- | --- |
| `５三角成(71)` | `Some(L)` | `▲５三角左成` | `None` / `▲５三角成` / 再パース `Err` |
| `５三角成(31)` | `Some(R)` | `▲５三角右成` | 同上 |
| `５三角(71)` | `Some(L)` | `▲５三角左不成` | 同上 |
| `５三角(31)` | `Some(R)` | `▲５三角右不成` | 同上 |

**`不成` の綴りも末尾が `成` なので同じく壊れる。`成` だけを剥がす修正はこのテストの後半2行で落ちる。**

**T2（GAP-002/003）** — 終局語12語 × 読み、`MoveSpecial` 14種 × 書き。
配列に並べて `assert_eq!(14, CASES.len())` を置き、バリアント追加時に落とす。

**T3（GAP-009）** — 同じ手数の `変化：2手` を2ブロック。`forks[0]` が先に書かれた方であること。
`v.push` → `v.insert(0, …)` の変異で必ず落ちることが受け入れ条件。

---

## 重要な設計提案（J10 の再検討）

**architecture-reviewer が D2 より良い案を出した。**

> `to_ki2` が `relative` を読むのをやめる。`shogi_official_kifu::display_single_move_write_kansuji`
> は ▲/△・同・筋段・駒種・曖昧性解消・成/不成 を全部書き、`converter/ki2.rs:53-89` が
> 手で組み立てている1手分と過不足なく一致する。局面を再生しながらこれに書かせればよい。

- **破壊的変更ゼロ。** `ToKi2::to_ki2` のシグネチャは変わらず、obs-shogi の修正が不要
- **GAP-010 が根から消える。** `normalize_move` の `pop()` パターンを直す必要すらなくなる
- `relative` は局面から導出できる値であって JKF に焼かれた事実ではない。
  **導出値をデータに書いてから書き手がそれを信じる構造が問題の発生源**という指摘

→ **D2 を差し替えるか、ユーザーに確認する。**

---

## lint / hook / 型で強制できるもの

両 reviewer が独立に同じ結論に達した。

1. **`src/lib.rs` に `#![deny(clippy::unimplemented, clippy::todo, clippy::unreachable, clippy::panic, clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::arithmetic_side_effects)]`。**
   本ラウンドの panic 系 BLOCK（R1-05/06/11/13）は**全部これで機械的に落ちる**。
   `.claude/verify.sh` が `clippy -D warnings` を回しているので追加コストゼロ
2. **`#![deny(missing_docs)]`。** obs-shogi が唯一呼ぶ `normalize()` が無文書という状態を止める
3. **対応表を単一の `const` に畳み、各利用側を網羅 `match` にする。**
   `_ =>` を書かなければバリアント追加が**1箇所のコンパイルエラー**になる。
   `converter/kif.rs:117` の TODO が放置されているのは、そこが `_ =>` で潰されているから
4. **`clippy.toml` の `disallowed-methods` に `nom::character::complete::line_ending` を登録。**
   CR 単独と末尾改行なしの再発を止める（R1-10 / R1-23）
5. **`File::read_to_string` も `disallowed-methods` へ。** CSA の UTF-8 決め打ち（R1-16）が検出される
6. **往復テスト `parse_X(to_X(jkf)) == jkf` を3形式 × `data/tests/` 全件で。**
   R1-01 と R1-02 は**この1本で両方落ちる**
7. `.gitignore` に `/examples/_probe*.rs`（R1-26）

## research/ へ書き戻したもの

- `research/tables/40-vocabulary.md` 表1 に `converter/kakinoki.rs` 列を追加し、
  誤って ✅ だった「KIF 書き」を 💥 に修正（R1-11）
- `research/tables/50-tsshogi-comparison.md` を新規作成（tsshogi との突き合わせ）
- `research/95-decisions.md` に D1〜D4 と、D3 の調査結果

## 見ていない範囲（統合）

- KI2 の**実ファイルコーパスが無い**。KI2 の所見は合成入力と仕様書の実例に基づく
- `normalize` の冪等性（`30-api-contract.md` 不変条件4）
- obs-shogi の TS 側。`moves: []` やコメント専用ノードが実際に TS から来るかは未確認。
  R1-13 は**到達可能性ではなく契約違反として**挙げている
- `data/tests/` のゴールデン JSON の妥当性
- 深い分岐でのスタック消費: 公開入口からの到達は再現できなかった
  （`serde_json` の再帰上限128、KIF テキストからは T5 で捨てられる）。コーパス最大 nesting は 8
