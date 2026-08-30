# Plan: shogi-kifu-converter 10x+ Parse Performance

**Source**: 計測駆動レビュー (`/ecc:code-review` 結果, 本ファイル末尾参照)
**Selected Milestone**: パース速度 10× 改善 (現状 3.72 s → 目標 ≤ 0.37 s on `test/bug_mega.kif`)
**Complexity**: Medium

---

## Summary

`shogi-kifu-converter` の KIF パースが大型ファイル (`test/bug_mega.kif`: 19,681 行 / ~14,527 手 / 1,151 変化) で 3.72 秒かかる。計測の結果、**99% が `src/normalizer.rs` の `display_single_move_kansuji` 呼び出しに費やされている**ことが判明。nom パーサ本体は 33 ms (1%) で問題ない。

本計画は、(1) KIF パスで不要な `display_single_move_kansuji` 呼び出しを止める、(2) `Move::try_from` 二重呼び出しと `Vec` の早期確保を解消、(3) 変化(fork)の正規化を rayon で並列化、の 3 段構えで **10×〜30×** の高速化を目指す。

---

## Baseline (確定計測)

`./target/release/examples/kif2jkf test/bug_mega.kif`

| Phase | Time | 占有率 |
|---|---|---|
| nom parse | 33 ms | 1% |
| normalize (`display_single_move_kansuji` × ~28 k 手) | 3,639 ms | 99% |
| **合計** | **3.72 s** | 100% |

| 参考ファイル | 合計 |
|---|---|
| `test/bug_big.kif` (3,953 行) | 0.15 s |
| `data/tests/kif/Shogidokoro.kif` (412 行) | 0.04 s |

→ **改善のすべては `src/normalizer.rs` で起きる。** nom 文法は触らない。

---

## Patterns to Mirror

| Category | Source | Pattern |
|---|---|---|
| 公開 API 拡張 | `src/normalizer.rs:267` `normalize_with_color_correction(&mut self, correct_color: bool)` | bool フラグを足して挙動を切り替える前例あり。同形でフラグを追加 |
| エラー伝播 | `src/error.rs:1-75` (`thiserror`) | `ParseError` / `NormalizeError` の variant を再利用 |
| 内部ループ | `src/normalizer.rs:524-557` `normalize_moves` | `for mf in moves`、`pos.clone()` で fork を再帰 |
| パーサ→正規化フロー | `src/parser.rs:90-101` `parse_kif_str` | `kif::parse(s).finish()` → `normalize_*` |
| テスト | `src/parser.rs:183-261` | ディレクトリスキャン + serde で `JsonKifuFormat` を直 deserialize → assert_eq |
| 並列化の前例 | なし | rayon を新規追加(`Cargo.toml` の `[dependencies]` に追記) |
| ベンチ | なし | criterion を `dev-dependencies` に追加し `benches/parse.rs` を新規作成 |
| KIF golden での `relative` 出現 | `data/tests/kif/Shogidokoro.json` / `oui202106290101.json` には 0 件 | KIF パスは元々 `relative=None` が標準出力 → `infer_relative=false` でも diff が出にくい |
| Converter 側の `relative` 利用 | `src/converter/ki2.rs:66` | KI2 出力時には `relative` を読む。KIF→KI2 変換時のためのフォールバックが必要 |

---

## Files to Change

| File | Action | Why |
|---|---|---|
| `Cargo.toml` | UPDATE | `rayon = "1"` (deps), `criterion = "0.5"` (dev-deps), `[[bench]] name = "parse" harness = false` |
| `src/normalizer.rs` | UPDATE | `infer_relative` フラグ追加、`display_single_move_kansuji` 条件化、`Move::try_from` 二重排除、`calculate_from` の Vec 遅延、fork 並列化 |
| `src/parser.rs` | UPDATE | KIF パスは `infer_relative=false`、KI2 / CSA / JKF は従来挙動。UTF-8 リトライ条件を `Kif` パーサエラーに絞る |
| `src/parser/kif.rs` | UPDATE (小) | `merge_forks` 境界ガード(`get_mut` / `checked_sub`) |
| `src/jkf.rs` | UPDATE (小) | `JsonKifuFormat::populate_relative(&mut self)` の公開メソッド宣言 |
| `src/lib.rs` | 変更なし想定 | モジュール構成はそのまま |
| `benches/parse.rs` | CREATE | criterion で `bug_mega.kif` / `bug_big.kif` / `Shogidokoro.kif` 計測 |
| `docs/PERF.md` | CREATE (任意) | フェーズごとの計測ログを記録 |

---

## Phases & Tasks

### Phase 1 — ベンチ整備とベースライン固定 (Small)

#### Task 1.1: criterion 導入
- **Action**: `Cargo.toml` に `[dev-dependencies] criterion = "0.5"` と `[[bench]] name = "parse" harness = false`。
- **Action**: `benches/parse.rs` を作成。`parse_kif_file("test/bug_mega.kif")`, `test/bug_big.kif`, `data/tests/kif/Shogidokoro.kif` の 3 ケースを `c.bench_function` で。重いケースはサンプル 10 程度。
- **Validate**: `cargo bench --bench parse -- --quick` がエラーなく完了し中央値が表示される。

#### Task 1.2: ベースライン記録(任意)
- **Action**: `docs/PERF.md` に表で初期値を記録。各 Phase 後に追記。

---

### Phase 2 — `display_single_move_kansuji` のスキップ (~9×) (Medium) — **本命**

#### Task 2.1: `normalize_with_options` の追加
- **Action**: `src/normalizer.rs:267` の `normalize_with_color_correction` を、次の関数に置き換える:
  ```rust
  pub fn normalize_with_options(
      &mut self,
      correct_color: bool,
      infer_relative: bool,
  ) -> Result<(), NormalizeError>
  ```
  既存の `normalize_with_color_correction(correct_color)` は `normalize_with_options(correct_color, true)` を呼ぶラッパで残し、`normalize(&mut self)` も `normalize_with_options(false, true)` のラッパとして互換維持。
- **Action**: `normalize_moves`, `normalize_move` のシグネチャに `infer_relative: bool` を貫通。
- **Mirror**: `correct_color` の伝播 (`normalizer.rs:267-528`) と同形。
- **Validate**: `cargo build --release` 成功。

#### Task 2.2: `display_single_move_kansuji` の条件化
- **Action**: `src/normalizer.rs:501` の `if mmf.relative.is_none() { ... }` を `if infer_relative && mmf.relative.is_none() { ... }` に変更。
- **Validate**: `cargo test --release` 緑(KIF ゴールデン JSON は `relative` 未収録なので差分なしのはず)。

#### Task 2.3: パーサ層での使い分け
- **Action**:
  - `src/parser.rs:90` `parse_kif_str` → `normalize_with_options(true, false)` (KIF: `from` 明示なので推論不要)
  - `src/parser.rs:134` `parse_ki2_str` → `normalize_with_options(true, true)` (KI2: 必要なので継続)
  - `parse_csa_str`, `parse_jkf_str` → 既存どおり `normalize()` で互換維持
- **Mirror**: 既存 `parse_kif_str` 構造を尊重。
- **Validate**: 全テスト緑、ベンチで bug_mega が ~400 ms 程度に。

#### Task 2.4: lazy populate API
- **Action**: `JsonKifuFormat::populate_relative(&mut self) -> Result<(), NormalizeError>` を実装。中身は「位置を再シミュレートして `mmf.relative` が None の手だけ `display_single_move_kansuji` を呼ぶ」。
- **Why**: KIF→KI2 変換など `relative` が要る下流が opt-in でコストを払える。
- **Validate**: `parse_kif_str(s) → populate_relative()` の結果が、旧挙動と等価であるゴールデン比較を 1 件追加。

#### Task 2.5: リベンチ
- **Validate**: `cargo bench --bench parse` で bug_mega ≈ 350–450 ms (~9×)。

---

### Phase 3 — マイクロ最適化 (1.1–1.3×) (Small)

#### Task 3.1: `Move::try_from` の二重呼び出し解消
- **Action**: `normalize_move` の戻り値を `Result<Move, NormalizeError>` に変更し、`normalize_moves` 側 (`normalizer.rs:545`) でその `Move` を `make_move(mv)` に流す。
- **Validate**: 全テスト緑。

#### Task 3.2: `calculate_from` の Vec 確保を遅延
- **Action**: `src/normalizer.rs:371-372`:
  ```rust
  // before
  let mut froms = bb.into_iter().collect::<Vec<_>>();
  match bb.count() { 0 => ..., 1 => ..., 2.. => { /* uses froms */ } }
  // after
  match bb.count() {
      0 => Ok(None),
      1 => Ok(bb.into_iter().next().map(...)),
      2.. => {
          let mut froms: Vec<_> = bb.into_iter().collect();
          /* existing logic */
      }
  }
  ```
- **Why**: KI2 パスで 2-5×。KIF パスは入らない。
- **Validate**: `ki2_to_jkf` テスト緑。

#### Task 3.3: `parse_kif_file` の UTF-8 リトライ条件を絞る
- **Action**: `src/parser.rs:73-80` のマッチを以下に変更:
  ```rust
  Err(err @ ParseError::Kif(_)) if encoding == SHIFT_JIS => { /* retry */ }
  Err(err) => Err(err),
  ```
  `ParseError::Normalize` ではリトライしない(再パースしても同じエラーになる蓋然性が高く、3.7 秒 × 2 になるのを防ぐ)。
- **Validate**: 既存 KIF テスト緑。

#### Task 3.4: `merge_forks` の境界ガード
- **Action**: `src/parser/kif.rs:172` `last[j - *i].forks` を `last.get_mut(j.checked_sub(*i)?)` 系に変更。
- **Validate**: 既存テスト緑 + 不正な `変化:` インデックスの単体テスト 1 件追加。

---

### Phase 4 — fork 並列化 (rayon) (~2-3× 追加) (Medium)

#### Task 4.1: rayon 依存追加
- **Action**: `Cargo.toml` の `[dependencies]` に `rayon = "1"`。

#### Task 4.2: fork 正規化を `par_iter_mut` 化
- **Action**: `src/normalizer.rs:530-536`:
  ```rust
  if let Some(forks) = &mut mf.forks {
      let pos_snapshot = pos.clone();
      let totals_snapshot = totals;
      if forks.len() >= 8 {
          use rayon::prelude::*;
          let oks: Vec<bool> = forks
              .par_iter_mut()
              .map(|v| normalize_moves(v, pos_snapshot.clone(), totals_snapshot, correct_color, infer_relative).is_ok())
              .collect();
          let mut iter = oks.into_iter();
          forks.retain_mut(|_| iter.next().unwrap_or(false));
      } else {
          forks.retain_mut(|v| {
              normalize_moves(v, pos_snapshot.clone(), totals_snapshot, correct_color, infer_relative).is_ok()
          });
      }
  }
  ```
- **Why**: fork 同士は独立。`PartialPosition: Clone`, `TimeFormat: Copy` で安全に共有可能。閾値 8 はオーバーヘッド回避。

#### Task 4.3: スレッドセーフ性の事前確認
- **Action**: `shogi_official_kifu` / `shogi_legality_lite` のソース内に `static mut` / `thread_local!` / 共有 `RefCell` が無いか軽く確認。問題があれば該当処理をメインスレッドへ集約。
- **Validate**: `cargo test --release -- --test-threads=4`。

#### Task 4.4: リベンチ
- **Validate**: `cargo bench --bench parse` で bug_mega ≈ 150–250 ms (~15–25×)。

---

## Validation

```bash
# 静的検査
cargo clippy --all-targets -- -D warnings
cargo fmt --check

# 単体・結合テスト
cargo test --release

# ベンチ (criterion)
cargo bench --bench parse -- --save-baseline phase0   # 計測開始前に保存
# … 各 Phase 完了ごとに --save-baseline phaseN
cargo bench --bench parse -- --baseline phase0        # 差分でリグレッション検出

# 受け入れ閾値
# bug_mega.kif: ≤ 370 ms (≥10x)
# bug_big.kif:  ≤ 15 ms  (≥10x)
```

---

## Risks

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| KIF パスで `relative` を計算しなくなることで下流の KIF→KI2 変換が壊れる | Medium | High | `populate_relative()` を `pub` で提供。KI2 コンバータ側で必要なら呼ぶ |
| rayon 内部から呼ぶ `shogi_official_kifu` / `shogi_legality_lite` がスレッドセーフでない | Low | High | コードを grep で `static mut` / `thread_local!` / `Cell` 監査。問題あれば Phase 4 をスキップ |
| ベンチが安定しない (CI のノイズ) | Medium | Medium | criterion の reps 増、`.cargo/config.toml` で `target-cpu=native` |
| 並列化のオーバーヘッドで変化が少ないファイルが遅くなる | Low | Low | Task 4.2 の閾値 (`len() >= 8`) 分岐 |
| 既存ゴールデン JSON が `relative` を含むケースで diff が出る | Low | Medium | 全 KIF ゴールデンを `grep -c '"relative"'` で事前確認(Shogidokoro / oui202106290101 は 0 件確認済み) |
| `populate_relative` の再シミュレーションコストが現状と同じ | High | Low | これは想定どおり。README に「fast path で恩恵を得る用途」を明記 |

---

## Expected Progression

| Phase 完了時点 | bug_mega.kif | 倍率 |
|---|---|---|
| ベースライン | 3,720 ms | 1.0× |
| Phase 2 (kansuji skip) | ~400 ms | ~9× |
| Phase 3 (micro) | ~350 ms | ~10.6× |
| Phase 4 (rayon forks, 4 core) | ~120-180 ms | **~20-30×** |

**10× は Phase 3 までで到達**、Phase 4 で 20× 超を狙う構成。

---

## Acceptance

- [ ] `cargo test --release` 緑のまま
- [ ] `cargo clippy --all-targets -- -D warnings` 緑
- [ ] `cargo bench --bench parse` で bug_mega ≤ 370 ms (≥10×)
- [ ] `parse_kif_str` / `parse_kif_file` の公開シグネチャ不変
- [ ] `relative` の出力差分が既存ゴールデンに無い (`assert_eq` で担保)
- [ ] スレッド数 1 でも実行可能 (`RAYON_NUM_THREADS=1`)

---

## Open Decisions (実装前に確認)

1. **`infer_relative` のデフォルト** — 既存挙動互換派 (`true`) / 高速派 (`false` で KIF のみ opt-out)。本計画では既存 API は `true` 据え置き、KIF パーサ層で `false` を渡す方針。
2. **rayon 追加可否** — 依存追加して良い。バイナリサイズ増加は数百 KB。
3. **Phase 5 (主系列並列化)** — 本回はスキップ(Phase 2 後は主系列のコストが誤差レベルになるため)。

---

## Review Background (for context)

計測ログ:

```
[bench] test/bug_mega.kif
  nom parse: 33.861ms
  normalize: 3.639s

[bench] test/bug_big.kif
  nom parse: 1.557ms
  normalize: 133.762ms

[bench] data/tests/kif/Shogidokoro.kif
  nom parse: 497.833µs
  normalize: 26.950ms
```

主犯: `src/normalizer.rs:502` `display_single_move_kansuji(pos, mv)` を毎手呼んでいる。KIF 形式では `from` が明示されているため `relative` 推論は不要で、これがそのまま無駄な処理になっている。
