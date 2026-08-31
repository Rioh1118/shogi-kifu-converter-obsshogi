# レビュー four-issues ラウンド1

- 日付: 2026-08-31
- 範囲: `git diff main...HEAD`（8ファイル / +429 −82）。issue #6 / #7 / #8 / #9 を1つの PR で解消する変更
- 対象コミット: `eebfd1b`
- 走らせた reviewer: rust / spec / robustness / comment / perf / architecture

## 所見

### R1-01 [BLOCK] `starts_a_line` が行全体を見ており、数字を含む注記と読めない消費時間で正当な棋譜がファイルごと落ちる

- 場所: `src/parser/kakinoki.rs:78-88`
- 指摘: rust / spec / robustness / comment / architecture の**5本すべて**
- 実測（いずれも main は `Ok`、HEAD は `KIF Error: … in this line runs into the one below it`）:

| 入力 | main | HEAD |
| --- | --- | --- |
| `   1 ７六歩(77) ( 0:01)`（消費時間が今回分だけ） | Ok 1手 | Err |
| `   1 ７六歩(77)　( 0:01/00:00:01)`（全角空白） | Ok 1手 | Err |
| `   1 ７六歩(77) ( 0:01/00:00:01) 評価値+120` | Ok 1手 | Err |
| `手合割：平手（10分切れ負け）` | Ok（header へ） | Err |
| `先手番 残り3分` | Ok | Err |

- なぜ重いか: `move_time` が受けるのは `(m:s/h:m:s)` 系の半角形だけ。読めなかった時間欄がそのまま「下の行」と判定される。**tsshogi はここを明示的に寛容にしている**（`kakinoki.mjs` の `timeRegExp` は検索・失敗しても続行）ので、tsshogi が読める棋譜をこのクレートだけが拒否する。obs-shogi ではそのファイルが索引から丸ごと落ちる。コーパス609件では踏まないが、踏んだときの被害が「1手落とす」ではなく「全滅」
- 直し方: 判定を行頭に錨づける。数字は「`<手数>` + 空白 + 何か」という**指し手行の形**まで見る（`( 0:01)` は数字を含むが形ではない）。改行が空白以外に化けた形を拾うため、先頭の1文字だけ読み飛ばして再判定する

### R1-02 [HIGH] KI2 の `branch_header` の `Failure` が呼び手に握り潰され、`ends_here` が効いていない

- 場所: `src/parser/ki2.rs:265`（`if let Ok((rest, start_ply)) = branch_header(input)`）
- 指摘: rust / spec / robustness
- 実測: `手合割：平手\n▲７六歩 △３四歩\n変化：2手 まで1手で中断\n` → **`Ok` 2手、分岐も終局も痕跡なく消える**
- なぜ問題か: `ends_here` の doc は「呼び手は `opt`/`alt` の下にあるので `Failure` にした」と書くが、`if let Ok` は `Failure` も捨てる。KI2 では `broken_line` が**一度も表に出ていない**
- 直し方: `match` にして `Err(e @ nom::Err::Failure(_)) => return Err(e)`

### R1-03 [HIGH] `変化：` ブロックの検査が、誤った行を指し・実際の原因を隠し・空行が無いと素通しする

- 場所: `src/parser/kif.rs:312`（`opt(skippable_line)`）、`:404-415`
- 指摘: architecture / spec / rust
- 実測:

| 入力 | 結果 |
| --- | --- |
| `…本譜…\n\n変化：2手\n`（空行あり） | Err |
| `…本譜…\n変化：2手\n`（**空行なし**） | **Ok**（分岐が黙って消える） |
| `bug_big.kif`（831行目が `変化：42手`、832行目が語彙外の `パス`） | `at line 832, in a 変化 block with no moves under it` ← **原因は「手が無い」ではない**。main は `cannot read this` で正確だった |
| `…\n\n変化：3手\n`（EOF） | `at line 8`（存在しない行）、キャレット行が空 |

- 直し方: (a) `moves_with_index` 末尾の `opt(skippable_line)` が `変化：` を食わないようにする（消費する場所を1箇所に決める）、(b) `Skipped::BranchHeader` にヘッダ行のスライスを持たせてそこを指す、(c) 専用エラーは「ヘッダの後に読めるものが1つも無い」ときだけにし、読めない行があるなら D1 の残り入力検査に正確な位置と文言を任せる

### R1-04 [HIGH] `promote` の推定が、棋譜が述べた `false` を上書きし、棋譜に無い `成` を書き足す

- 場所: `src/normalizer.rs:451-452`
- 指摘: rust / spec
- 実測:

| 入力 | main | HEAD |
| --- | --- | --- |
| JKF `{"piece":"TO","promote":false}` | `None` | **`Some(true)`**（述べた `false` が消える） |
| KIF `   3 ７五と(76)`（76 にあるのは歩） | `Ok`、`成` なし | **`to_kif` が `７五歩成(76)`** — 原文に無い `成` を書く |

- 仕様: D12 が決めたのは「棋譜が述べた成を消さない」であって、その逆ではない。`piece` の食い違いは D12 により「盤で上書き」であって、`promote` の言明に昇格させる筋は無い
- 直し方: 述べられていればそれが正。`mmf.promote.unwrap_or_else(|| from_piece_kind.promote() == Some(stated))`。あわせて KIF パーサが `promote: Some(promote.is_some())` を入れる（R-KIF-006 により KIF では `成` の不在が不成の言明そのもの）。これで駒名からの推定は CSA と外部 JKF だけに効く

### R1-05 [MEDIUM] `to_ki2` / `to_kif` が、手合割を名乗っている棋譜に2本目の `手合割：平手` を書く

- 場所: `src/converter/kakinoki.rs:152-160`
- 指摘: architecture / robustness
- 実測: `手合割：詰将棋\n先手：A\n▲７六歩 △３四歩\n` を読んで書き戻すと `手合割：詰将棋` と `手合割：平手` が並ぶ。**KI2 では main に無かった行**（`omit_hirate` を落としたため）
- 直し方: `header` に `手合割` があるときは preset 行を書かない（**ユーザー決定: 書き出し側で握る**）

### R1-06 [MEDIUM] `parse_csa_file` が Shift_JIS の CSA を `csa-1.0.2` の入力依存 `unwrap` に流し込むようになった

- 場所: `src/parser.rs:30-34`
- 指摘: robustness
- 実測: `$START_TIME:2004/02/30` を含む Shift_JIS の `.csa` — main は `Err(Io)`、HEAD は **panic**（`csa-1.0.2/src/parser/time.rs:57`）
- 位置づけ: obs-shogi は CSA だけ `catch_unwind` で包んでいる（`kifu_reader.rs:85`）ので**アプリは落ちない**。ただし踏める入力の集合が広がった
- 直し方: `parse_csa_file` / `parse_csa_str` の `///` に `# Panics` を書く（CLAUDE.md「公開 API には panic 条件を書く」）。`90-gaps.md` に記録し、`csa` クレートの扱いは GAP-012 に集約する

### R1-07 [MEDIUM] `normalize_with_options` の新しい契約が駒打ちに当てはまらない

- 場所: `src/normalizer.rs:174-188`、`:257-259`
- 指摘: comment / architecture
- 根拠: `piece` / `same` / `capture` の再計算は `if let Some(pf) = &mmf.from` の中だけ。駒打ち（R-JKF-003 により `from` なし）では1つも走らない。実測で `capture: Some(HI)` を入れた `１五角打` はそのまま残る
- 直し方: doc に駒打ちの例外を明記する。`same` は「`to` を決めるための入力」でもあるので、その向きも書く

### R1-08 [MEDIUM] 新しいテストのアサーションが緩く、壊れた実装でも通る

- 場所: `src/normalizer.rs:987-991`、`src/parser.rs:389-398`
- 指摘: rust / comment
- 根拠: `contains("６八") || contains("Invalid move")` の第2項は**あらゆる正規化失敗で真**。`変化` 系の2ケースは `is_err()` しか見ておらず、**KI2 では意図した `broken_line` ではなく残り入力検査が偶然 `Err` にしていた**（R1-02）ことを見分けられない
- 直し方: `||` を落とす。`at ply 1` / `at line N` と壊れた行の内容まで見る

### R1-09 [MEDIUM] コメントに変更の経緯が混ざっている（`used to` 系 9箇所）

- 場所: `src/converter/ki2.rs:802`、`src/parser.rs:366-368`、`src/converter/kakinoki.rs:148-152`、`src/normalizer.rs:941-943` ほか
- 指摘: comment
- 線引き: 主語が**過去の実装や過去の出来事**なら経緯（禁止）。主語が**採らなかった規則**なら理由（可、ただし現在形に直す）
- 直し方: 前者は現在の入出力の記述に置き換え、後者は時制を現在形にする

### R1-10 [MEDIUM] 要件 ID の引き方の誤り

- 場所: `src/parser/kif.rs:224-229`（R-KIF-005 / R-KIF-008 は「行がそこで終わる」と述べていない）、`src/normalizer.rs:970-974`（`J14` は**未決の論点 ID**。決着は D12）、`src/parser/kakinoki.rs:273-278`（Mynavi の主張に出典が無く、`above all` が何も指していない）
- 指摘: comment
- 直し方: 規定の範囲に切り詰める / `D12` に差し替える / 出典を書けないなら実物への依存を落とす

### R1-11 [MEDIUM] `write_initial` の doc「Always writes one」に、書かずに `Err` を返す経路が抜けている

- 場所: `src/converter/kakinoki.rs:146-161`
- 指摘: comment
- 根拠: `Initial { preset: PresetOther, data: None }` では `write_initial_preset` が `ConvertError::UnknownPreset` を返す（GAP-001 の範囲）
- 直し方: 例外を明記し、`write_initial_preset` 内の「`その他` はここへ来ない」も「盤を持つ `その他` はここへ来ない」に直す

### R1-12 [MEDIUM] KI2 が `変化：N手` の `N` を使う理由が書かれていない（KIF は捨てる）

- 場所: `src/parser/ki2.rs:123`
- 指摘: comment
- 根拠: D3 —「tsshogi は KIF では `変化：N手` を読まない。KI2 のときだけ使う」。非対称は正典どおりだが、KIF 側にしか根拠が書かれていない
- 直し方: KI2 側の doc に D3 を引く1行を足す

## 重複・矛盾した所見

- **R1-01 は5本が同じ場所を別の入力で指摘した。** 最優先
- **R1-03 について spec は「guard ごと落とす」、architecture は「消費場所を1箇所にして残す」と割れた。** 判断: **残すが「ヘッダの後に読めるものが1つも無い」ときだけ**に狭める。D3 は「KIF では宣言を読まない」だが、ヘッダの直後でファイルが切れているのは D1 が言う「読み残し」であって、無害な空ブロックとは別
- **R1-04 の直し方について rust は normalize 側、spec はパーサ側を挙げた。** 両方入れる（normalize で「述べられた値が正」、KIF パーサで「`成` の不在も言明」）。片方だけでは `７五と(76)` か `promote:false` のどちらかが残る
- **`成れない駒が成る` 棋譜（robustness の HIGH）は所見にしない。** ユーザー決定により**現状維持（ファイル全体を `Err`）**。R-RULE-002 との衝突は承知のうえの選択なので、`95-decisions.md` に決定として残す

## 見ていない範囲

- **`.ki2` と `.csa` の実コーパスが1件も無い**（コーパス609件はすべて `.kif`）。KI2 / CSA の判断はすべて合成入力
- CSA の往復（`parse_csa_str` → `normalize` → `to_csa`）を実ファイルで確かめていない
- 書き出し側のベンチ（`bench_write`）は回していない
- obs-shogi は grep とソース読みのみ。このブランチを指してビルドしていない
- 深い入れ子のスタック消費（GAP-019）への影響

## lint / hook / 型で強制できるもの

- **`nom::Err::Failure` を `if let Ok` で握り潰す形**（R1-02）。`branch_header` を `Result<Option<..>, nom::Err<..>>` に変えれば型で落とせる
- **要件 ID の実在検査**: `src/` のコメントから `R-xxx-nnn` / `D<n>` / `GAP-nnn` / `J<n>` を抜き、`research/` の見出しと照合する。`J` で始まる ID（未決の論点）は警告
- **変更の経緯の混入**: `git diff --cached` の追加行に対する grep（`used to` / `no longer` / `今回` / `〜に変更した`）
- **`starts_a_line` / `ends_here` の表引きテスト**（`)+` は通る / `評価値+50` は通る / `   5 ７六歩(77)` は落ちる）
- **コーパス回帰の機械化**: 「main が `Ok` にした入力を HEAD が弾かない」をタグ前に走らせる

## research/ へ書き戻すもの

- **GAP-020 に追記**: 改行の化けでも残る KI2 の2ケース（`*` コメント行が指し手を飲む / `まで` 行が飲まれる）と、ヘッダ行どうしが繋がると `先手：` が消える件（判定が届かない行種）
- **GAP-012 に追記**: #8 で Shift_JIS の CSA も `csa-1.0.2` の panic 経路に入るようになった。CSA には残り入力の検査が無く、壊れた行から先が黙って落ちる（`XYZZY` を挟んだ5手の CSA が `Ok` 2手）
- **新しい決定**: 成れない駒の `成` はファイル全体を `Err` にする（R-RULE-002 との衝突を承知で選択）。`手合割` が header にあるときは preset 行を書かない
- **文字コードの限界**: `N+凜々` を Shift_JIS で書いた CSA は UTF-8 としても妥当に復号され、対局者名が化けたまま `Ok`（main も同じ。doc の書き方が「解決した」と読める点だけ直す）

## 次ラウンドの対象

今回直す: R1-01 → R1-02 → R1-03 → R1-04 → R1-05 → R1-06 → R1-07 → R1-08 → R1-09 → R1-10 → R1-11 → R1-12（順に1件1コミット）

見送る: `成れない駒の成`（ユーザー決定により現状維持）、CSA の打ち切り検出（`csa` クレートのフォークが要る。GAP-012 に集約）、`move_time` の綴りの寛容化（R1-01 を直せば main と同じ挙動に戻るため、この PR の範囲外）
