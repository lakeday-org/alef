# shellcheck shell=bash

setup() {
  bats_load_library xberg-bats
  xberg_setup
  SCRIPT="$BATS_TEST_DIRNAME/../toolchain-census.sh"
  DIRECTORY_SCRIPT="$BATS_TEST_DIRNAME/../toolchain-census-dir.sh"
  mkdir -p "$XBERG_WORK/census"
}

@test "directory follows Cargo metadata rather than stale environment" {
  xberg_stub cargo 'printf "%s\n" '\''{"target_directory":"/external output"}'\'''
  run env CARGO_TARGET_DIR=/stale sh "$DIRECTORY_SCRIPT"
  xberg_assert_status 0
  xberg_assert_output '/external output/toolchain-census'
}

@test "directory falls back to environment when metadata fails" {
  xberg_stub cargo 'exit 1'
  run env CARGO_TARGET_DIR='/fallback output' sh "$DIRECTORY_SCRIPT"
  xberg_assert_status 0
  xberg_assert_output '/fallback output/toolchain-census'
}

@test "directory normalizes Windows JSON paths" {
  xberg_stub cargo 'printf "%s\n" '\''{"target_directory":"D:\\a\\target"}'\'''
  run sh "$DIRECTORY_SCRIPT"
  xberg_assert_status 0
  xberg_assert_output 'D:/a/target/toolchain-census'
}

@test "census sums rows and accepts fully executed required fixtures" {
  printf 'ruby\t2\t2\t0\t0\n' >"$XBERG_WORK/census/first.tsv"
  printf 'ruby\t3\t3\t0\n' >"$XBERG_WORK/census/second.tsv"
  run sh "$SCRIPT" --dir "$XBERG_WORK/census" --require ruby
  xberg_assert_status 0
  xberg_assert_output_contains '5 of  5 fixtures executed'
}

@test "census rejects missing required measurements" {
  run sh "$SCRIPT" --dir "$XBERG_WORK/census" --require ruby
  xberg_assert_status 1
  xberg_assert_output_contains 'FAILED: nothing ran'
}

@test "census rejects absent and unusable required fixtures" {
  printf 'ruby\t3\t1\t1\t1\n' >"$XBERG_WORK/census/run.tsv"
  run sh "$SCRIPT" --dir "$XBERG_WORK/census" --require ruby
  xberg_assert_status 1
  xberg_assert_output_contains '1 absent and 1 unusable'
}

@test "census rejects malformed inconsistent and nonnumeric rows" {
  for row in 'ruby 1 1' 'ruby 2 1 0 0' 'ruby nope 0 0'; do
    printf '%s\n' "$row" >"$XBERG_WORK/census/run.tsv"
    run sh "$SCRIPT" --dir "$XBERG_WORK/census"
    xberg_assert_status 1
  done
}

@test "census rejects unknown flags and missing values" {
  for flag in --unknown --dir --require; do
    run sh "$SCRIPT" "$flag"
    xberg_assert_status 2
  done
}
