#!/usr/bin/env bash
# Exercise the production build-cache guard without running the later compilation lanes.
set -euo pipefail
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd -P)"
mkdir -p "$repo_root/target"
fixture="$(mktemp -d "$repo_root/target/cache-guard-test.XXXXXX")"
trap 'rm -rf "$fixture"' EXIT
mkdir -p "$fixture/local/target/build-cache" "$fixture/foreign/target/build-cache"
{
  printf '%s\n' 'set -euo pipefail' 'repo_root="$1"' 'cd "$repo_root"'
  sed -n '/^# BEGIN BUILD CACHE GUARD$/,/^# END BUILD CACHE GUARD$/p' "$repo_root/ops/ci/pr-ci.sh"
  printf '%s\n' '[[ "$CARGO_TARGET_DIR" == "$repo_root/target/build-cache" && "$CARGO_INCREMENTAL" == 0 ]]'
} > "$fixture/guard.sh"
run_guard() {
  env JERYU_CI_BUILD_CACHE_DIR="$1" bash "$fixture/guard.sh" "$fixture/local" >"$fixture/output" 2>&1
}
refuse() {
  if run_guard "$1"; then
    echo "cache guard accepted $1" >&2
    exit 1
  fi
  grep -q 'refusing' "$fixture/output"
}
run_guard "$fixture/local/target/build-cache"
refuse "$fixture/foreign/target/build-cache"
# Ambient Git overrides must not move the anchor to another repository.
git init -q "$fixture/local"
git init -q "$fixture/foreign"
export GIT_DIR="$fixture/foreign/.git" GIT_WORK_TREE="$fixture/foreign"
run_guard "$fixture/local/target/build-cache"
refuse "$fixture/foreign/target/build-cache"
# A checkout that tracks files under target/ is refused, with Git overrides pointing elsewhere.
printf 'x\n' > "$fixture/local/target/tracked"
env -u GIT_DIR -u GIT_WORK_TREE git -C "$fixture/local" add -f target/tracked
refuse "$fixture/local/target/build-cache"
grep -q 'target/ contains tracked files' "$fixture/output"
printf 'build cache guard tests passed\n'
