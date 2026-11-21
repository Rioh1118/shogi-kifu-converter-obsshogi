# 公開関数の役割と、守るルール・ケース

このクレートの外から呼べるものを、役割ごとに並べる。各項目の「ルール」は満たすべきこと、「ケース」はテストで押さえるべき入力の種類。
規則の ID（`R-…`）は `research/` の要件。用語は [glossary.md](glossary.md) に従う。

consumer（obs-shogi、`~/src/github.com/Rioh1118/obs-shogi/src-tauri/src`）が本番のコードで呼んでいるのは、`parse_{kif,ki2,csa,jkf}_str`、`parse_{kif,ki2,jkf}_file`（`search/read/kifu_reader.rs`）、`try_to_{kif,ki2,csa}_owned`、`normalize`、`jkf` と `error` の型。
呼んでいないのは `parse_csa_file`、`normalize_with_*`、`populate_relative`、USI、非推奨の `to_*_owned`。

## 全体の流れ

```
テキスト ──読む──▶ JKF（読み取ったまま）──正規化──▶ JKF（盤で確かめた）──書く──▶ テキスト
   parse_*_str        各書式の読み手            normalize_*            ToKif / ToKi2 / ToCsa / ToUsi
```

公開の読む関数は、どれも**正規化まで済ませて返す**。例外は公開の trait 実装 `TryFrom<csa::GameRecord> for JsonKifuFormat`（`src/csa.rs:8`）で、正規化しない。

## 0. 書式を値で選ぶ（`Format`）

役割: 書式を1つの値で表し、拡張子からの選択・読み・書きを1か所にする。

- `Format::from_extension(ext)`: `kif`/`kifu` → KIF、`ki2`/`ki2u` → KI2、`csa` → CSA、`jkf` → JKF。大文字小文字を区別しない。それ以外は `None`
- `parser::parse_str(format, s)`: 書式ごとの `parse_*_str` と同じ
- `parser::parse_file(path)`: 拡張子で書式を選び、書式ごとの `parse_*_file` と同じ。拡張子が当たらなければ `Err(FileExtension)`
- `JsonKifuFormat::try_to_string(format)`: `try_to_{kif,ki2,csa}_owned` と同じ。JKF は字下げした JSON
- USI は書くだけなので `Format` に入れない

## 1. 読む（`parser`）

### `parse_kif_str(s) -> Result<JKF, ParseError>`

役割: KIF の文字列を、正規化済みの JKF にする。

ルール
- 先頭の BOM は記録の一部ではない。取り除いてから読む（R-KIF-001）
- 最後まで読めなかったら `Err(ParseError::Kif)`。途中で止まって `Ok` を返さない（判断 D1）
- 中身が1つも棋譜として読めなかったら `Err`。空の記録を `Ok` で返さない（ヘッダだけの KIF は `Ok`）
- 手番は手合割で決まる。駒落ちは1手目が後手（上手）（R-HC-001 / R-RULE-006）
- `relative`（右・左・上…）は埋めない（R-REQ-006）。KI2 へ書くとき書き手が盤から求める
- 反則手を含む記録も正常な入力。合法性で弾かない（R-RULE-002）。最初の終局より後は盤を追わない

ケース: BOM あり／なし、平手・駒落ち 15 種・`その他`＋盤面図、後手番から始まる盤面図、変化（分岐）、終局語 12 種（R-KIF-007）、消費時間、コメント、読めない行が途中にある、全行が読めない（CSA を `.kif` にした等）

### `parse_ki2_str(s) -> Result<JKF, ParseError>`

役割: KI2 の文字列を、正規化済みの JKF にする。

ルール（KIF と同じものは省く）
- 指し手に移動元が無い。移動元は盤と `relative` から決める（R-NOT-004）。決まらなければ `Err(Normalize)`
- `打` が無くても、盤上の駒で行けなければ打ちと読む（R-NOT-003）
- `不成` は書かれる。`成` も `不成` も無い手は、成れる手なら「不成」と同じ（R-NOT-005）
- `relative` を盤から求めて埋める（KIF と違う）
- 終局は `まで<N>手で…` の句で表す（R-KI2-006。仕様に定めが無く、判断 D5 で tsshogi に合わせた）

ケース: `同`（R-NOT-002）、相対位置の全 13 語と `行`・`生` の表記（R-NOT-006）、どれでも行ける2枚の駒、打と移動のどちらでも行ける、成れるのに書いていない、終局句、変化

### `parse_csa_str(s) -> Result<JKF, ParseError>`

役割: CSA の文字列を、正規化済みの JKF にする。本体は外部の `csa` クレート。

ルール
- 手番は記録に書かれている（`+`/`-`）。食い違えば `Err(Normalize)`（色を直さない）
- 打ちは移動元 `00`（R-CSA-007）。JKF では `from` を書かない（R-JKF-003）
- 盤の座標は1〜9（R-CSA-005）。外れたら `Err(CsaConvert)`

**守れていないこと**（`csa` クレートの制約、判断ログ GAP-012）
- 一部の入力で panic する（存在しない日付、桁あふれの `T`）。consumer 側で捕まえている
- `%TIME_UP` `%ILLEGAL_MOVE` `%+ILLEGAL_ACTION` `%-ILLEGAL_ACTION` を読めず、**黙って捨てる**（今回の検査で再現。[logic-check.md](logic-check.md)）
- 読めない行から先を黙って捨てる。KIF・KI2 の「最後まで読めなければ `Err`」が CSA には無い

### `parse_jkf_str(s) -> Result<JKF, ParseError>`

役割: JKF（JSON）を読み、正規化する。

ルール: BOM を取る。手番は書かれたものを正とし、食い違えば `Err`。`relative` を埋める。

### `parse_{kif,ki2,csa,jkf}_file(path)`

役割: ファイルを読み、文字コードを決めて `parse_*_str` に渡す。

ルール
- KIF・KI2 は拡張子で最初に試す文字コードを選ぶ（`.kif`/`.ki2` は Shift_JIS、`.kifu`/`.ki2u` は UTF-8）。復号に失敗したらもう一方を試す（R-REQ-003）
- KIF・KI2 は拡張子が上の4つ以外なら `Err(FileExtension)`。大文字小文字は区別しない
- CSA は拡張子を見ない。UTF-8 → Shift_JIS の順に試す（R-CSA-001）
- JKF は UTF-8 のみ

### 読む関数の間の食い違い（整理の対象）

| 書式 | 手番が記録と食い違ったら | `relative` |
| --- | --- | --- |
| KIF | 直す | 埋めない |
| KI2 | 直す | 埋める |
| CSA | `Err` | 埋める |
| JKF | `Err` | 埋める |

同じ「正規化済みの JKF」でも、書式によって `relative` の有無が違う。返り値の型からは分からない。

## 2. 正規化する（`JsonKifuFormat` のメソッド）

### `normalize(&mut self)`

役割: 盤を1手ずつ進め、記録の各手を盤と食い違わない形にそろえる。手番は直さない。

ルール
- 盤から決まる `piece`・`same`・`capture` は盤の値で上書きする。打ちは移動元が無いので、この3つを触らない
- `promote` は記録のもの。盤からは「成れたのに成らなかった」ときに `false` を補うだけ（R-NOT-005）
- 移動元が「書かれていない」印（`(0,0)`）のときだけ、盤から移動元を求める
- `time.total` は盤を追えている手で計算し直す（盤を見失った後の手は触らない。`normalizer.rs:832-838`）
- 盤が手合割の初期配置と一致したら、`initial` を手合割の名前に畳む
- 最初の終局より前で盤が説明できない手があれば `Err(NormalizeError { ply, kind })`。終局より後、および分岐の中で説明できない手は、そのまま残す（R-RULE-002）

駒落ちで `moves[1]` の手番が先手なら、本譜の手番を全て反転する前処理がある。`correct_color` の値に関係なく走る（`normalize_with_options` の文書に書いた）。

### `normalize_with_color_correction(correct_color)` / `normalize_with_options(correct_color, infer_relative)`（非推奨）

役割: `normalize` の変種。`bool` 引数2つで、手番を直すかと、`relative` を埋めるかを選ぶ。

内部では、2つの選択を名前で持つ `Normalization`（`ColorSource::Recorded`／`Position`、`RelativeFill::Fill`／`Skip`）を渡す。読み手は `Normalization::KIF`／`KI2`／`RECORDED` で呼ぶ。公開の関数は真偽値のまま残し、中で変換する。

### `populate_relative(&mut self)`

役割: `relative` が無い手に、盤から求めた相対位置を埋める。KIF から読んだ JKF を JKF として出すときに呼ぶ。

ルール: 最初の終局より後は盤を追わない。KI2 へ書くだけなら不要（書き手が盤から求める、判断 D2）。

## 3. 書く（`converter`）

### `ToKif::to_kif(sink)` / `try_to_kif_owned()`

役割: JKF を KIF の文字列にする。

ルール
- 打ちには必ず `打` を付ける。`不成` は書かない（R-KIF-006）
- 指し手行は `手数 指し手(移動元) (消費時間/累計)`（R-KIF-005 / R-KIF-008）
- 手合割は名前で書き、名前の無い局面は盤面図で書く（R-KIF-014）。名前も盤も無ければ `Err(UnknownPreset)`
- ヘッダの値は1行。改行を取り除く（R-KIF-004）
- 終局語は KIF の語彙だけ（R-KIF-007）。`HIKIWAKE` は `持将棋`、`MATTA`・`ERROR` は `中断` として書く（判断 D7。読み戻すと種類が変わる）
- 分岐は `変化：N手`。N は手数（コメントだけの枠は数えない）

### `ToKi2::to_ki2(sink)` / `try_to_ki2_owned()`

役割: JKF を KI2 の文字列にする。

ルール
- 相対位置は `relative` を読まず、盤から求める（判断 D2）
- 2枚以上の駒が行けて、相対位置でも区別できない手は `Err(UnspellableMove)`（R-NOT-004 段階3）
- `打` は、盤上の駒でも行けるときだけ付ける（R-NOT-003）
- 成れる手で成らなかったら `不成` を書く（R-NOT-005）
- 終局は `まで<N>手で…`（判断 D5）

### `ToCsa::to_csa(sink)` / `try_to_csa_owned()`

役割: JKF を CSA V2.2 の文字列にする。

ルール
- ヘッダは CSA に置き場のあるキーだけ書く（R-CSA-004）。先手・後手（無ければ 下手・上手）を `N+`/`N-` に、棋戦・場所・戦型を `$` 行に。他のキーは書かない
- 打ちの移動元は `00`、手の後の `T<秒>` はその手の消費時間（R-CSA-007）
- `FUSENSHO`・`FUSENPAI` は CSA に語が無く `%CHUDAN` として書く

### `to_{kif,ki2,csa}_owned()`（非推奨）

失敗すると、そこまで書いた途中の文字列を返す。consumer は `try_to_*_owned` に移行済み。

### `ToUsi::to_usi(sink)` / `try_to_usi_owned()`

役割: `startpos` か `sfen …` に続けて `moves …` を書く。

ルール: 盤で再生できない記録は `Err`（`to_usi` は `fmt::Error`、`try_to_usi_owned` は理由つき）。`to_usi_owned` は空文字を返す（`shogi_core` の既定は panic するため上書きしている）。

## 4. 型の相互変換（`shogi_core` との間）

- `TryFrom<&JsonKifuFormat> for shogi_core::Position` — 盤で手順を再生する。`その他` で盤が無ければ `Err`
- `TryFrom<&shogi_core::Position> for JsonKifuFormat` — 手順から JKF を作る。正規化はしない（`same`・`capture`・`relative` は空）
- `TryFrom<csa::GameRecord> for JsonKifuFormat` — CSA クレートの記録から JKF を作る。正規化しない
- 駒・手番・座標・持ち駒の `From`／`TryFrom`

## 5. エラー（`error`）

| 型 | 出る場面 |
| --- | --- |
| `ParseError` | 読む関数 |
| `NormalizeError { ply, kind }` | 正規化。`ply` は本譜の添字 |
| `ConvertError` | 書く関数、盤への変換 |

整理の対象: 同じ「手合割に盤が無い」に `ConvertError::InitialBoardNoDataWithPresetOTHER`（盤への変換）と `ConvertError::UnknownPreset`（書くとき）の2つの名前がある。メッセージの大文字小文字も揃っていない（`Failed to normalize` と `failed to normalize`）。
