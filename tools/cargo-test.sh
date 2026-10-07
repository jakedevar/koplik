#!/usr/bin/env bash
# Accept cargo test arguments, prepare dependencies, then isolate ALL execution
# (including rustdoc tests and test-spawned processes, not just Cargo's runner).
set -euo pipefail
project_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-"$project_dir/target"}
cargo_command=(cargo)
if [[ -x $HOME/.rsi/bin/cargo-slot ]]; then
  cargo_command=("$HOME/.rsi/bin/cargo-slot" cargo)
fi
# Cargo cannot combine --doc with --no-run. Compile the library first in that
# case; rustdoc compiles and executes its examples in the isolated second phase.
build_args=()
doc_only=0
for arg in "$@"; do
  if [[ $arg == --doc && $doc_only == 0 ]]; then
    build_args+=(--lib)
  else
    build_args+=("$arg")
  fi
  # Arguments after -- belong to the test binary, even if they look like --doc.
  [[ $arg != -- ]] || doc_only=1
done
"${cargo_command[@]}" test --no-run "${build_args[@]}"
exec "$project_dir/tools/offline-test.sh" "${cargo_command[@]}" test --offline "$@"
