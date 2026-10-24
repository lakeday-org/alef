#!/usr/bin/env bash
set -euo pipefail
source "${CATEGORY_SCRIPT:?generated category path is required}"
sample_cli() { printf '%s' "$JSON_PAYLOAD"; }
assert_equals() {
  printf 'assertion\n' >>"$ASSERTION_LOG"
  [[ "$1" == "$2" ]] || {
    printf 'exact mismatch\n' >&2
    return 1
  }
}
assert_is_empty() { [[ -z "$1" ]]; }
assert_not_empty() { [[ -n "$1" && "$1" != null ]]; }
assert_contains() { [[ "$1" == *"$2"* ]]; }
if test_capture; then
  exit 0
else
  exit 1
fi
