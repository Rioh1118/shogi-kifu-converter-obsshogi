# research/ — 仕様の正典

2026-11-21 に作り直した。前の版はこのディレクトリが `.gitignore` されていたため失われ、git の履歴にも残っていない。
コードが引く要件 ID（`R-…`）は前の版の番号をそのまま使い、意味をコードの引用と一次資料から推定して埋め直した。

## 読み方

- 各規定は `R-<領域>-<番号>`。末尾の `〔…〕` が出典の節。出典の全文の抜き書きは [99-sources.md](99-sources.md)
- **確信度**: 高＝一次資料の文言どおり／中＝資料はあるが解釈を含む／低＝一次資料が無く、コードの引用から推定した
- **一次資料が無い規定**は「資料なし」と書く。コードの挙動を写しただけで、正しさは確かめていない
- `D<番号>`（判断）は [95-decisions.md](95-decisions.md)、`GAP-<番号>`（実装とのずれ）は [90-gaps.md](90-gaps.md)。どちらも失われた記録をコードのコメントから復元したもので、中身は不完全

## ファイル

- [00-requirements.md](00-requirements.md) — このクレートが守る要件（consumer との境界）。R-REQ
- [10-rules.md](10-rules.md) — 将棋のルールのうち棋譜に効く範囲。R-RULE
- [20-notation.md](20-notation.md) — 伝統形式の表記と曖昧さの解き方。R-NOT
- [30-kif.md](30-kif.md) — KIF。R-KIF
- [31-ki2.md](31-ki2.md) — KI2。R-KI2
- [32-csa.md](32-csa.md) — CSA。R-CSA
- [33-jkf.md](33-jkf.md) — JKF。R-JKF
- [34-usi-sfen.md](34-usi-sfen.md) — USI / SFEN。R-USI
- [40-handicap.md](40-handicap.md) — 手合割 16 種。R-HC
- [90-gaps.md](90-gaps.md) — 実装とのずれ
- [95-decisions.md](95-decisions.md) — 仕様に定めが無い所で決めたこと
- [99-sources.md](99-sources.md) — 一次資料の抜き書き、未定義と食い違いの一覧

## 番号の欠け

コードが引いていない番号（R-KIF-003・009・013、R-CSA-002・003、R-KI2-004、R-RULE-003〜005、R-REQ-001・005）は、意味を推定する材料が無い。空き番号として残し、新しい規定には使わない。
