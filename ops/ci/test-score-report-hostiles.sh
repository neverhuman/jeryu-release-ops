#!/usr/bin/env bash
# Exercise report admission only. Policy parsing is deliberately not run.
set -euo pipefail
repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd -P)"
script="$repo_root/ops/ci/score.sh"
umask 077
scratch="$(mktemp -d /tmp/jeryu-score-report.XXXXXX)"
cleanup() {
  local result=$? resolved
  resolved="$(realpath -e -- "$scratch" 2>/dev/null || true)"
  case "$resolved" in
    /tmp/jeryu-score-report.??????)
      if (( result == 0 )) && [[ "$resolved" == "$scratch" && ! -L "$resolved" ]]; then
        rm -rf -- "$resolved"
      else
        printf 'score report test failed; retained fixture: %s\n' "$scratch" >&2
      fi
      ;;
  esac
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

mkdir -p "$scratch/agent" "$scratch/ops/ci"
for path in owner-map.json test-map.json generated-zones.toml proof-lanes.toml \
  audit-policy.toml boundaries.toml JANKURAI_STANDARD.md; do
  printf 'synthetic report-admission input\n' >"$scratch/agent/$path"
done
cat >"$scratch/ops/ci/lib.sh" <<'LIB'
require_jankurai() { :; }
require_tool() { command -v "$1" >/dev/null 2>&1 || return 1; }
jankurai() {
  [[ $1 == audit ]] || return 97
  printf 'audit\n' >>calls
  (( SCORE_TEST_AUDITOR_STATUS == 0 )) || return "$SCORE_TEST_AUDITOR_STATUS"
  cat report-input.json >.jankurai/repo-score.json
  printf 'synthetic report\n' >.jankurai/repo-score.md
}
# Stop at the unchanged Python stage; never execute a Python interpreter.
python3() { printf 'policy-transport-not-executed\n' >>calls; return 79; }
LIB

valid='{"score":85,"caps_applied":[],"findings":[],"decision":{"hard_findings":0,"passed":true,"status":"advisory"}}'
cases=0
expect() {
  local expected=$1 report=$2 auditor_status=${3:-0} actual=0 expected_calls=audit
  printf '%s\n' "$report" >"$scratch/report-input.json"
  : >"$scratch/calls"
  (cd "$scratch" && SCORE_TEST_AUDITOR_STATUS=$auditor_status bash "$script") \
    >"$scratch/stdout" 2>"$scratch/stderr" || actual=$?
  [[ $actual == "$expected" ]] || {
    printf 'case %s: expected exit %s, got %s\n' "$cases" "$expected" "$actual" >&2
    return 1
  }
  (( expected != 79 )) || expected_calls+=$'\npolicy-transport-not-executed'
  [[ $(<"$scratch/calls") == "$expected_calls" ]] || {
    printf 'case %s: unexpected producer/policy order\n' "$cases" >&2
    return 1
  }
  # Admission and stopped policy transport never publish target score artifacts.
  [[ ! -e "$scratch/target/jankurai/repo-score.json" &&
     ! -e "$scratch/target/jankurai/repo-score.md" ]]
  cases=$((cases + 1))
}

expect 79 "$valid"
expect 79 "$(jq '.findings=[{severity:"medium",hardness:"soft"},{severity:"low"},{severity:"info"}]' <<<"$valid")"
expect 79 "$(jq '.hard_findings=0|.caps=[]|.decision={}' <<<"$valid")"
# These are report shape/range checks only; they do not prove any policy floor.
for score in 0 64 65 81 82 84 85 90 100; do
  expect 79 "$(jq --argjson score "$score" '.score=$score' <<<"$valid")"
done
for mutation in \
  '.score=-1' '.score=101' '.score=85.5' '.score=true' '.score="85"' \
  'del(.score)' '.caps_applied={}' '.caps_applied=null' 'del(.caps_applied)' \
  '.caps_applied=["cap"]' '.caps=["concealed cap"]' '.caps=null' '.caps={}' '.caps=false' \
  '.findings={}' '.findings=null' 'del(.findings)' '.findings=[null]' \
  '.findings=[{severity:"high"}]' '.findings=[{severity:"critical"}]' \
  '.findings=[{severity:"low",hardness:"hard"}]' '.findings=[{severity:"unknown"}]' \
  '.findings=[{}]' '.findings=[{severity:"low",hardness:null}]' \
  '.findings=[{severity:"low",hardness:false}]' '.findings=[{severity:"low",hardness:"unknown"}]' \
  '.hard_findings=1' '.hard_findings=-1' '.hard_findings="0"' '.hard_findings=false' '.hard_findings=[]' \
  '.decision.hard_findings=1' '.decision.hard_findings=-1' \
  '.decision.hard_findings="0"' '.decision.hard_findings=false' '.decision.hard_findings=[]' \
  '.decision=null' '.decision=[]' 'del(.decision)'; do
  expect 1 "$(jq "$mutation" <<<"$valid")"
done
expect 1 "$valid
$valid"
expect 1 ''
expect 1 '{'
expect 1 '[]'
expect 1 'null'
expect 23 "$valid" 23

printf '%s report admission/producer cases passed; policy parsing was not executed\n' "$cases"
