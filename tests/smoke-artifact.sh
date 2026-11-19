#!/usr/bin/env bash
# Smoke test of the PUBLISHED artifact: `cargo package` the crate, check the .crate holds only
# Cargo.toml, README, LICENSE, CHANGELOG and src/, unpack it into an empty temp dir, use it as a path
# dependency of a fresh crate and run the README "Testing" example (offline, against MockOnlineSim).
# Usage: tests/smoke-artifact.sh [extra `cargo package` args, e.g. --allow-dirty]
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
name=onlinesim-rs-api
version=$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -n 1)

files=$(cargo package --list "$@")
stray=$(printf '%s\n' "$files" | grep -Ev '^(Cargo\.toml|Cargo\.toml\.orig|Cargo\.lock|README\.md|LICENSE|CHANGELOG\.md|\.cargo_vcs_info\.json|src/.+\.rs)$' || true)
if [ -n "$stray" ]; then
  echo "unexpected files in the package:"; echo "$stray"; exit 1
fi
for f in Cargo.toml README.md LICENSE CHANGELOG.md src/lib.rs src/mock/mod.rs; do
  printf '%s\n' "$files" | grep -qx "$f" || { echo "missing from the package: $f"; exit 1; }
done
echo "package list ok: $(printf '%s\n' "$files" | wc -l | tr -d ' ') files"

cargo package "$@"
crate="$root/target/package/$name-$version.crate"

tmp=$(mktemp -d "${TMPDIR:-/tmp}/onlinesim-smoke-XXXXXX")
trap 'rm -rf "$tmp"' EXIT
mkdir "$tmp/unpacked" "$tmp/app" "$tmp/app/src"
tar -xzf "$crate" -C "$tmp/unpacked"

cat > "$tmp/app/Cargo.toml" <<TOML
[package]
name = "smoke-app"
version = "0.0.0"
edition = "2024"
rust-version = "1.85"
publish = false

[dependencies]
$name = { path = "$tmp/unpacked/$name-$version", features = ["mock-transport"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }

[workspace]
TOML

# The first ```rust block after "## Testing" in the packaged README.
awk '/^## Testing/ { t = 1 } t && /^```rust$/ { c = 1; next } c && /^```$/ { exit } c { print }' \
  "$tmp/unpacked/$name-$version/README.md" > "$tmp/app/src/main.rs"
[ -s "$tmp/app/src/main.rs" ] || { echo 'README.md has no "## Testing" rust example'; exit 1; }

CARGO_TARGET_DIR="$root/target/smoke-artifact" cargo run --quiet --manifest-path "$tmp/app/Cargo.toml"
echo "README \"Testing\" example from $name-$version.crate: ok ($(rustc --version))"
