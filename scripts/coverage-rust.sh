#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: scripts/coverage-rust.sh [--html] [--include-ignored]

Generate Rust coverage for solver-core-rs using LLVM tools from rustup.

Options:
  --html             Write an HTML report to /tmp/solverlab-coverage-html.
  --include-ignored  Include ignored slow smoke tests.
  -h, --help         Show this help.
USAGE
}

html=0
include_ignored=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --html)
      html=1
      ;;
    --include-ignored)
      include_ignored=1
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
  shift
done

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/.." && pwd)"
crate_dir="$repo_root/solver-core-rs"
profile_dir="/tmp/solverlab-cov"
target_dir="/tmp/solverlab-target-cov"
html_dir="/tmp/solverlab-coverage-html"

rm -rf "$profile_dir" "$target_dir"
mkdir -p "$profile_dir"

test_args=(test --lib --tests)
if [[ "$include_ignored" -eq 1 ]]; then
  test_args+=(-- --include-ignored)
fi

(
  cd "$crate_dir"
  CARGO_INCREMENTAL=0 \
    RUSTFLAGS="-Cinstrument-coverage" \
    LLVM_PROFILE_FILE="$profile_dir/solver-core-%p-%m.profraw" \
    CARGO_TARGET_DIR="$target_dir" \
    cargo "${test_args[@]}"
)

llvm_bin="$(rustc --print sysroot)/lib/rustlib/$(rustc -vV | awk '/host:/ {print $2}')/bin"
"$llvm_bin/llvm-profdata" merge -sparse \
  "$profile_dir"/*.profraw \
  -o "$profile_dir/merged.profdata"

solver_bin="$(find "$target_dir/debug/deps" -maxdepth 1 -type f -perm +111 -name 'solver_core-*' | head -n 1)"
integration_bin="$(find "$target_dir/debug/deps" -maxdepth 1 -type f -perm +111 -name 'integration_tests-*' | head -n 1)"

if [[ -z "$solver_bin" || -z "$integration_bin" ]]; then
  echo "Could not find coverage test binaries under $target_dir/debug/deps" >&2
  exit 1
fi

common_args=(
  "$solver_bin"
  --object="$integration_bin"
  --instr-profile="$profile_dir/merged.profdata"
  --ignore-filename-regex='/(\.cargo|rustc|tmp/solverlab-target-cov|tests/)'
)

if [[ "$html" -eq 1 ]]; then
  rm -rf "$html_dir"
  "$llvm_bin/llvm-cov" show "${common_args[@]}" \
    --format=html \
    --output-dir="$html_dir"
  echo "HTML coverage report written to $html_dir/index.html"
else
  "$llvm_bin/llvm-cov" report "${common_args[@]}"
fi
