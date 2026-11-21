# 要件（consumer との境界）

外部資料ではなく、このクレートが consumer（obs-shogi）に対して守ること。根拠は `CLAUDE.md` と consumer のコード。

- **R-REQ-002** consumer は読み（`parse_*_str`）と書き（`try_to_{kif,ki2,csa}_owned`）の両方を使う。保存は consumer が作った JKF を、このクレートがテキストへ書くことで行う。パーサだけ確かめて済ませない。〔CLAUDE.md、`~/obs-shogi/src-tauri/src/workspace/record.rs`〕確信度 高
- **R-REQ-003** 拡張子は文字コードを示すが保証しない。バイト列を復号して誤りが出たら、もう一方の文字コード（UTF-8 / Shift_JIS）を試す。〔資料なし。KIF §2 の拡張子の規則を運用で補ったもの〕確信度 中
- **R-REQ-004** consumer は Tauri コマンド。入力から届く経路で panic しない。読めない行で黙って手を落とさない（途中で止まったら `Err`）。〔CLAUDE.md〕確信度 中
- **R-REQ-006** KIF を読むときは `relative` を埋めない（移動元が書いてあるので不要な計算）。`relative` そのものが要るときは `populate_relative()` を呼ぶ。KI2 へ書くときは書き手が盤から求めるので不要。〔CLAUDE.md〕確信度 高
- **R-REQ-007** consumer は多数の KIF を読んで索引を作るので、読む速さが使い心地に直接出る。ただし速さのために仕様への適合を落とさない。〔`.claude/agents/perf-reviewer.md` の引用から推定〕確信度 中
- **R-REQ-008** 反則手（行き所の無い駒、二歩、自殺手など）を含む棋譜も、棋譜全体を落とさずに読む。R-RULE-002 を読み手の要件にしたもの。〔`.claude/reviews/2026-08-30-spec-tables-r1.md` R1-17 から推定〕確信度 中
