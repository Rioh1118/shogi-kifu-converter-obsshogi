# JKF（JSON 棋譜フォーマット）

出典: https://github.com/na2hiro/json-kifu-format README「形式の定義」（Version 1.0）、スキーマ `data/specification/json-kifu-format.schema.json`

- **R-JKF-001** `moves[0]` は初期局面のコメント用の枠で、`moves[n]` が n 手目。`initial` が無ければ平手。〔README moves・initial〕確信度 高
- **R-JKF-002** `MoveFormat` のフィールドは全て省略できる。コメントだけの要素もありうる。〔README MoveFormat〕確信度 中
- **R-JKF-003** `move.from` が無ければ打ち。〔README「移動元 打った場合はなし」〕確信度 高
- **R-JKF-004** `forks` の各分岐の初手は、その要素の手の**代替**で、次の手ではない。〔README「分岐の初手はこのforksを持つ棋譜の代替の手とする(次の手ではなく)」〕確信度 高

参考（ID 未割り当て）
- `move.piece` は移動前の駒種、`promote` は true＝成・false＝不成・無し＝どちらでもない
- `special` の列挙は CSA V2.2 の 14 語。このクレートは `FUSENSHO`・`FUSENPAI` を独自に足している（D1）。JKF の標準の読み手は読めない
- 座標 `x`・`y` は1〜9（スキーマは説明文だけで、範囲を検査しない）
- スキーマの `relative` の正規表現は `^` `$` が無く、README より緩い
