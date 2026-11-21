# 用語集（ユビキタス言語）

1つの概念に1つの語。コード（識別子）、コメント（英語）、文書（日本語）でそれぞれ使う語を決め、使わない語を並べる。
「使わない」の括弧内の数は、置き換える前の `grep -rio <語> src | wc -l`（2026-11-21、大文字小文字を区別しない。他の語の一部としての出現も含むので目安）。

**コメントの言語**: 今のコメントは 3,704 行のほぼ全てが英語で、日本語だけの行（英字を含まない行）は 104 行で、多くは `jkf.rs` の `/// 香落ち` のような型の1語の説明。英語に統一する。日本語は将棋の表記そのもの（`打`、`同`、`手合割`）を引くときだけ使う。
**採否の決め方**: JKF のフィールド名がある概念は、その名前を使う（コードが JKF の型を公開しているので、そこから逃げられない）。

## 手番と対局者

| 概念 | コード | コメント | 文書 | 使わない（今の出現数） |
| --- | --- | --- | --- | --- |
| 先に指す側の色 | `Color::Black` | Black | 先手 | sente(5) |
| 後に指す側の色 | `Color::White` | White | 後手 | gote(4) |
| 駒落ちで駒を落とす側 | なし | upper hand | 上手 | なし |
| 駒落ちで駒を落とされない側 | なし | lower hand | 下手 | なし |
| 次に指す側 | `side_to_move` | side to move | 手番 | なし |

**上手・下手と先手・後手は別の軸。** 上手は後手（`White`）、下手は先手（`Black`）で、上手が先に指す（R-HC-001）。
（直した）以前のコードは2つの軸を混ぜていた。`jkf.rs` の `MoveSpecial` のコメントが、同じ enum の中で先手を「下手」（`SpecialIllegalActionBlack`）とも「上手」（`SpecialFusensho`）とも書いている。`jkf.rs` の `kif_word` のコメントにも「Black (the upper hand)」がある。どちらかが誤り。

## 手と手数

| 概念 | コード | コメント | 文書 | 使わない |
| --- | --- | --- | --- | --- |
| `moves` の1要素 | `MoveFormat` | node | 枠 | entry, slot（混在） |
| 駒を動かす・打つ1手 | `MoveMoveFormat`、`move_` | move | 指し手 | なし |
| `moves` の添字。0 は初期局面の枠 | `ply` | ply | 手数 | move number(6) |
| 盤上の駒を動かす手 | `Move::Normal` | board move | 移動 | なし |
| 持ち駒を打つ手 | `from: None` | drop | 打ち | なし |
| 移動元 | `from` | origin | 移動元 | なし |
| 移動元が書かれていない印 `(0,0)` | `ORIGIN_UNSTATED` | unstated origin | なし | なし |
| 移動先 | `to` | `to` | 移動先 | destination |
| 直前の手と同じ移動先 | `same` | `same` | 同 | なし |

コメントの文では origin、コードでは `from` と書く。「a move with a `from`」のような英文は読みにくいので、文の語と識別子を分けた（D27）。2つは同じものを指す。

## 終局と分岐

| 概念 | コード | コメント | 文書 | 使わない |
| --- | --- | --- | --- | --- |
| 投了・中断などの終局 | `special`、`MoveSpecial` | special move | 終局 | outcome |
| 本譜に対する別の手順 | `forks` | fork | 変化 | branch, variation |
| `forks[k]` の先頭 | `forks[k][0]` | the alternative to this move | 変化の1手目 | 「次の手」（R-JKF-004） |

`branch` 243 回と `fork` 119 回が同じものを指している。JKF のフィールド名 `forks` に合わせて fork に寄せる。KIF の見出し語 `変化` はそのまま。

## 局面と手合割

| 概念 | コード | コメント | 文書 | 使わない |
| --- | --- | --- | --- | --- |
| 対局開始時の局面 | `initial` | initial position | 開始局面 | starting position |
| 名前つきの開始局面 | `Preset` | preset | 手合割 | handicap（平手を含めて呼ぶとき） |
| 平手以外の手合割 | `Handicap` | handicap | 駒落ち | なし |
| 名前の無い開始局面 | `Preset::PresetOther` | other | その他 | なし |

`handicap.rs` は平手を含む 16 種を `Handicap` 型・`HANDICAPS` 表として持つ。平手は駒落ちではないので、型名と中身がずれている。

## 書式と表記

| 概念 | コード | コメント | 文書 | 使わない |
| --- | --- | --- | --- | --- |
| 1局分の記録 | `JsonKifuFormat`（`JKF`） | record | 棋譜 | kifu(201), game record(3) |
| 柿木形式（KIF と KI2 の総称） | `kakinoki` | Kakinoki formats | 柿木形式 | なし |
| KI2・連盟方式の表記 | `notation` | traditional notation | 伝統形式 | なし |
| 相対位置と動作（右・左・上・引・寄・直・打） | `relative`、`Relative` | relative | 相対位置 | なし |
| 移動先と駒の後ろに書く語の全体（相対位置・成・不成・打） | `Suffix` | suffix | 装飾 | なし |
| ヘッダ（対局情報） | `header` | header | 対局情報 | information |
| 書く先 | `sink` | sink | なし | なし |
| 書く関数 | `to_*`、`write_*` | writer | 書き手 | converter（モジュール名としてのみ） |
| 読む関数 | `parse_*` | reader | 読み手 | parser（モジュール名としてのみ） |
| 文字列にできる | なし | spellable | 書ける | spell(172) |

`information` は `parser/kakinoki.rs` の型名 `Information` に残っている。`spell` は動詞として使われているが、日本語の文書では「書ける」と言う。

`suffix` と `relative` は別の概念として残す。`relative` は JKF のフィールド（相対位置1つ）、`suffix` は表記の上で駒名の後ろに付く語で、`Suffix` 型は「何も書かない」「`Relative` を1つ書く」「書けない」の3通りを持つ（`normalizer.rs` の `Suffix` の doc）。

## 関数名の形

真偽値を返す関数（テスト以外に 22 個）は2つの形にそろえた。

- 状態を問う: `is_*`（`is_padding`、`is_unrecognised`、`is_promotion_spellable`）
- 動作を問う: 三人称の動詞（`opens_kif_line`、`ends_word`、`could_promote`）。標準ライブラリの `starts_with` と同じ形

前は `a_move_body_opens` のような文の形が8個、`promotion_was_on_the_table` のような名詞で始まる形が2個あった。

略語は使わない（例: `pk2k` は `kind_from_piece_kind` に相当する）。
