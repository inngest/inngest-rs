#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

mapfile -t tests < <(find inngest/tests -maxdepth 1 -type f -name '*_e2e.rs' | sort)

if [[ "${#tests[@]}" -eq 0 ]]; then
  echo "No e2e test files found under inngest/tests" >&2
  exit 1
fi

for test_file in "${tests[@]}"; do
  test_name="$(basename "${test_file%.rs}")"
  echo "Running ${test_name}"
  cargo test -p inngest --test "$test_name" -- --test-threads=1
done
