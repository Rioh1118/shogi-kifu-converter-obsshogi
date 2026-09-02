# shogi-kifu-converter (obs-shogi fork)

棋譜フォーマット（KIF / KI2 / CSA / JKF / USI）の相互変換を行う Rust ライブラリ。
[sugyan/shogi-kifu-converter](https://github.com/sugyan/shogi-kifu-converter) のフォーク。
**正規化とパーサのエラーが実用に耐えなかったため**フォークして直しながら使っている。

## 検証（変更後に必ず実行）

```bash
bash .claude/verify.sh    # cargo fmt --check + clippy -D warnings + test （約30秒）
```

**コマンドはここに写さない。** `.claude/verify.sh` が唯一の出典。

`git commit` は `.claude/hooks/verify-gate.sh` が横取りし、変更ファイルの種類に応じて
上を自動で走らせる。落ちればコミット自体が止まる。**止まったら直す。飛ばさない。**
`.md` や `data/` だけの変更は素通しする。

作業を「完了」と報告する前に必ず通すこと。通していないなら「未検証」と明示すること。

## 仕様の正典は `research/`

**挙動に迷ったら実装ではなく `research/` を読む。** ここが外部基準。

| ファイル | 内容 |
| --- | --- |
| `research/00-requirements.md` | 機能要件と obs-shogi との境界 |
| `research/10-rules.md` | 将棋のルールのうち棋譜に効く範囲 |
| `research/20-notation.md` | 伝統形式の表記と曖昧性解消規則 |
| `research/30-kif.md` 〜 `34-usi-sfen.md` | 各フォーマット仕様 |
| `research/40-handicap.md` | 手合割 16 種の初期配置（正典） |
| `research/90-gaps.md` | **再現を確かめた**実装とのギャップ |

`research/` は `.gitignore` されている。コミットしない。

規定には `R-<領域>-<番号>` の ID が振ってある。コメントやテスト名からこの ID を参照してよい。

**「元の実装がこう書いてあるから正しいはず」で判断しない。** それでフォークする羽目になった。
`research/` に書いていないことは「まだ調べていない」であって「どうでもいい」ではない。
一次情報に当たらずに `research/` へ記述を足さない。

## 実装

**コードを変更する前に `/implement` を読む。** 刻み方・レビューの厚さの決め方・
途中で見つけた既存の問題の扱い・タグを打つまでの順序をそこに置いてある。

- `/review-round` — 観点ごとの reviewer を並列で走らせ `.claude/reviews/` に報告書を書く
- `/review-fix` — 報告書の所見を1件1コミットで直し、結果を報告書に書き戻す

指摘がゼロのラウンドが1回出るまでこのループを終わらせない。

## consumer は obs-shogi ひとつ

`~/obs-shogi` が `src-tauri/Cargo.toml` で **git タグを指して**このクレートを使っている。

```toml
shogi_kifu_converter_obsshogi = { git = "...", tag = "v0.3.1", package = "shogi-kifu-converter" }
```

- **read 側**: `search/kifu_reader.rs` が `parse_kif_str` / `parse_ki2_str` などを呼ぶ
- **write 側**: `kifu.rs` と `file_system/operations.rs` が `converter::{ToCsa, ToKi2, ToKif}` を呼ぶ

**「write は tsshogi 側だからコンバータは影響しない」は誤り。** tsshogi が作るのは
書き出す JKF であって、テキストへのシリアライズはこのクレート。
パーサだけ確認して済ませないこと。詳細は `research/00-requirements.md` R-REQ-002。

**consumer は Tauri コマンド。panic はアプリのクラッシュとして出る。**
`unimplemented!()` / `unwrap()` / `unreachable!()` を入力依存の経路に置かない。

## 既知の落とし穴

- **`moves[0]` は指し手ではない。** 初期局面のコメントを持つ枠。指し手は添字 1 から
- **駒落ちでは1手目が後手（上手）。** 手数の偶奇だけで手番を決めると全反転する（R-RULE-006）
- **`Preset::PresetOther` は盤面を持たない。** 手合割の表（`src/handicap.rs`）が
  残り 15 種を展開する。`その他` だけは `ConvertError` になる。**panic はしない**
  （`research/90-gaps.md` GAP-001 は解消済み）
- **KIF の「打」「不成」は伝統形式と規則が違う。** KIF は打に必ず「打」を付け、
  「不成」を書かない。KI2 は逆（R-KIF-006 / R-NOT-003 / R-NOT-005）。
  KIF ↔ KI2 の変換でここを素通しにすると壊れる
- **`relative` は KIF パースでは埋めない。** KI2 へ書き出すなら `populate_relative()` を呼ぶ（R-REQ-006）
- **`forks[k][0]` はその手の「代替」であって「次の手」ではない**（R-JKF-004）
- 反則手が記録された棋譜は正常な入力。**合法性でパースを弾かない**（R-RULE-002）

## テストの現状（誇張しないこと）

**件数をここに書かない。** 書くと必ず腐る。現在値は `cargo test` の末尾で確認すること。

`src/` の `#[test]` は動いており、`data/tests/` との突き合わせもある。
ただし**カバーしているのは平手中心の正常系**で、駒落ち・壊れた入力・往復変換は薄い。
`cargo test` の green は「今テストしている範囲が壊れていない」以上を意味しない。
「テストが通ったので安全」と書いてはいけない。新規ロジックには実際にテストを足すこと。

## コメント

読み手は**その変更を書いた人ではない**。

- 書くのは「なぜ」。何をしているかはコードで表す
- **仕様に根拠がある判断には `research/` の要件 ID を書く**（`// R-KIF-006: KIF は不成を書かない`）。
  これが一番価値のあるコメント。外部仕様は絶対にコードから読み取れない
- **変更の経緯を書かない。**「今回の修正で」「〜に変更した」「PR #N で対応」は全て禁止。
  経緯は git log と PR に残る。コードには現在どうあるべきかだけを書く
- コードを変えたらコメントも変える。腐ったコメントは無いより悪い
- `TODO` は issue 番号を伴わせる（`// TODO(#123): ...`）
- 公開 API（`pub`）には `///` を付ける。panic 条件とエラーの意味を書く

## フォークであることの作法

- **upstream との差分は小さく保つ。** リポジトリ直下に新しいファイルを増やさない。
  作業用のものは `.claude/` か `research/`（どちらも upstream には無い）へ置く
- `Cargo.toml` の `authors` / `repository` は upstream のまま。**帰属を消さない**
- upstream の設計に不満があっても、直す範囲は「obs-shogi が壊れている箇所」に留める。
  作り直したくなったら issue にして、ユーザーに判断させる

## 進め方

- コミットは `<type>: <description>`（type: feat/fix/refactor/docs/test/chore/perf/ci）
- `main` に直接コミットしない。ブランチを切ること
- **obs-shogi はタグを指している。** `main` に入れただけでは consumer に届かない。
  リリースするならタグを打ち、obs-shogi 側の `Cargo.toml` を更新するところまでが1セット
- 同じ失敗を2回するまでルールを足さない。1回目は**ルールではなくテスト**を書く
