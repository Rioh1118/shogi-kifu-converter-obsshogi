#!/usr/bin/env bash
# このリポジトリの検証。**検証コマンドの唯一の出典。**
#
# CLAUDE.md も verify-gate.sh もここを呼ぶだけにしてある。コマンドを別の場所へ
# 写すと、片方だけ直されて必ず腐る。
#
# 手で走らせるときも `bash .claude/verify.sh`。所要はおよそ30秒。

set -uo pipefail

cd "$(dirname "$0")/.." || exit 1

failed=0

step() {
  local label=$1
  shift
  printf '=== %s\n' "$label"
  if ! "$@"; then
    printf '!!! %s が落ちた\n' "$label"
    failed=1
  fi
}

# clippy は `--all-targets` を付ける。付けないと `benches/` と `examples/` が
# コンパイルされず、そこを壊したコミットが素通しする。
step "cargo fmt" cargo fmt --all --check
step "cargo clippy" cargo clippy --all-targets --all-features -- -D warnings
step "cargo test" cargo test --all-features

# rustdoc も警告を落とす。`-D warnings` は clippy にしか掛かっておらず、
# 消した項目を指し続ける doc リンクが3回コミットゲートを素通りした
# （`[SPACES]` は R7-05 で消えた定数を R8 まで指していた）。
# `--document-private-items` が要る。壊れるのはほぼ private 項目へのリンク。
step "cargo doc" env RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --document-private-items --all-features

exit "$failed"
