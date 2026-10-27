# shellcheck shell=bash

setup() {
  bats_load_library xberg-bats
  xberg_setup
  SCRIPTS="$BATS_TEST_DIRNAME/../publish"
}

@test "normalizer defaults empty and whitespace to crates and CLI" {
  for input in '' '   ' $'\t\n'; do
    run env RAW_TARGETS="$input" bash "$SCRIPTS/normalize-release-targets.sh"
    xberg_assert_status 0
    xberg_assert_output 'crates,cli'
  done
}

@test "normalizer defaults unset input and trims explicit targets" {
  run env -u RAW_TARGETS bash "$SCRIPTS/normalize-release-targets.sh"
  xberg_assert_status 0
  xberg_assert_output 'crates,cli'
  run env RAW_TARGETS=' crates,scoop ' bash "$SCRIPTS/normalize-release-targets.sh"
  xberg_assert_status 0
  xberg_assert_output 'crates,scoop'
}

@test "Scoop gate accepts all and exact trimmed elements" {
  for input in all ' scoop ' 'crates, scoop ,cli'; do
    run env RELEASE_TARGETS="$input" bash "$SCRIPTS/compute-scoop-gate.sh"
    xberg_assert_status 0
    xberg_assert_output true
  done
}

@test "Scoop gate rejects partial names and missing targets" {
  for input in none crates scooping ALL; do
    run env RELEASE_TARGETS="$input" bash "$SCRIPTS/compute-scoop-gate.sh"
    xberg_assert_status 0
    xberg_assert_output false
  done
  run env -u RELEASE_TARGETS bash "$SCRIPTS/compute-scoop-gate.sh"
  [ "$status" -ne 0 ]
}

prepare_bucket() {
  export BUCKET_DIR="$XBERG_WORK" MANIFEST_PATH='bucket/alef.json' VERSION=0.1.0
  xberg_stub git 'printf "%s\n" "$*" >> "$XBERG_TRACE"' \
    'if [[ "$1" == diff ]]; then exit "${DIFF_STATUS:-0}"; fi' \
    'if [[ "$1" == push ]]; then exit "${PUSH_STATUS:-0}"; fi'
}

@test "unchanged staged manifest skips commit and push" {
  prepare_bucket
  run bash "$SCRIPTS/commit-scoop-manifest.sh"
  xberg_assert_status 0
  xberg_assert_output_contains skipped
  run tail -2 "$XBERG_TRACE"
  xberg_assert_output $'add -- bucket/alef.json\ndiff --cached --quiet -- bucket/alef.json'
}

@test "new manifest is staged before diff then committed and pushed" {
  prepare_bucket
  export DIFF_STATUS=1
  run bash "$SCRIPTS/commit-scoop-manifest.sh"
  xberg_assert_status 0
  xberg_assert_output committed
  run tail -4 "$XBERG_TRACE"
  xberg_assert_output $'add -- bucket/alef.json\ndiff --cached --quiet -- bucket/alef.json\ncommit -m alef 0.1.0\npush origin HEAD'
}

@test "push failure propagates without reporting committed" {
  prepare_bucket
  export DIFF_STATUS=1 PUSH_STATUS=7
  run bash "$SCRIPTS/commit-scoop-manifest.sh"
  xberg_assert_status 7
  [[ "$output" != *committed* ]]
}

prepare_formula() {
  export TAG=v0.2.0 VERSION=0.2.0 TAP_DIR="$XBERG_WORK/tap"
  mkdir -p "$TAP_DIR/Formula"
  printf '%s\n' 'class Alef < Formula' '  url "old-url"' '  sha256 "old-sha"' '  version "0.1.0"' '  bottle do' '    sha256 arm64: "bottle-sha"' '  end' 'end' >"$TAP_DIR/Formula/alef.rb"
  xberg_stub curl 'while [[ $# -gt 0 ]]; do if [[ "$1" == -o ]]; then printf archive > "$2"; exit 0; fi; shift; done' 'exit 2'
}

@test "Homebrew updates source metadata and preserves bottles" {
  prepare_formula
  run bash "$SCRIPTS/update-homebrew-formula.sh"
  xberg_assert_status 0
  run cat "$TAP_DIR/Formula/alef.rb"
  xberg_assert_output_contains 'url "https://github.com/xberg-io/alef/archive/v0.2.0.tar.gz"'
  xberg_assert_output_contains 'version "0.2.0"'
  xberg_assert_output_contains 'sha256 arm64: "bottle-sha"'
  [[ "$output" != *old-sha* ]]
}

@test "Homebrew missing formula aborts before download" {
  prepare_formula
  rm "$TAP_DIR/Formula/alef.rb"
  xberg_stub curl 'touch "$XBERG_TRACE"; exit 99'
  run bash "$SCRIPTS/update-homebrew-formula.sh"
  xberg_assert_status 1
  xberg_assert_trace_empty
}

@test "Homebrew download failure leaves formula unchanged" {
  prepare_formula
  cp "$TAP_DIR/Formula/alef.rb" "$XBERG_WORK/before"
  xberg_stub curl 'exit 22'
  run bash "$SCRIPTS/update-homebrew-formula.sh"
  xberg_assert_status 22
  run cmp "$XBERG_WORK/before" "$TAP_DIR/Formula/alef.rb"
  xberg_assert_status 0
}
