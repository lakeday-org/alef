#!/usr/bin/env bash
#MISE description="Package and verify all three release crates without publishing"
#USAGE arg "[version]" help="Release version to verify (default: the root Cargo.toml version)"
# shellcheck disable=SC2154 # usage_* are set by mise from the #USAGE lines
set -euo pipefail

manifest_version=$(awk -F '"' '/^version = / { print $2; exit }' Cargo.toml)
version="${usage_version:-$manifest_version}"
case "$version" in
"" | *[!0-9A-Za-z.+-]*)
  echo "Invalid release version" >&2
  exit 1
  ;;
esac
[ -f Cargo.toml ] && [ -f crates/alef-component-abi/Cargo.toml ] && [ -f crates/alef-component-runtime/Cargo.toml ]
[ "$version" = "$manifest_version" ]
target=$(mktemp -d "${TMPDIR:-/tmp}/alef-publish-verify.XXXXXX")
[ -n "$target" ] && [ -d "$target" ]
case "$target" in
/*) ;;
*)
  echo "Package target must be absolute" >&2
  exit 1
  ;;
esac
echo "Verifying release packages in $target"
cargo package --workspace --locked --target-dir "$target" -j1
set -- "$target"/package/*.crate
[ "$#" -eq 3 ] || {
  echo "Expected 3 crate archives, found $#" >&2
  exit 1
}
for crate in alef-component-abi alef-component-runtime alef; do
  [ -s "$target/package/$crate-$version.crate" ] || {
    echo "Missing or empty archive for $crate" >&2
    exit 1
  }
done
echo "Verified 3 nonempty release crate archives for $version"
