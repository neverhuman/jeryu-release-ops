#!/usr/bin/env bash
# Fail-closed Jankurai evidence lane. Every tool runs strictly: a failing tool
# fails the lane, and nothing here synthesizes an artifact. All output goes under
# target/jankurai/. The ratchet baseline is the committed, provenance-bound audit
# of protected main (agent/baselines/); this lane never produces its own baseline.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${ROOT}"
source "${ROOT}/ops/ci/lib.sh"
require_tool jq
require_jankurai

BASE_REF="origin/main"
BASELINE_REPORT="agent/baselines/main.repo-score.json"
BASELINE_PROVENANCE="agent/baselines/main.repo-score.provenance.json"

if ! git rev-parse --verify "${BASE_REF}^{commit}" >/dev/null 2>&1; then
  printf 'missing proof-evidence base ref: %s\n' "${BASE_REF}" >&2
  exit 1
fi

validate_baseline() {
  local base_commit baseline_commit baseline_sha baseline_tree current_head path
  for path in "${BASELINE_REPORT}" "${BASELINE_PROVENANCE}"; do
    if [[ ! -f "${path}" || -L "${path}" || "$(stat -c '%h' -- "${path}")" != 1 ]]; then
      printf 'baseline input must be a one-link regular file: %s\n' "${path}" >&2
      return 1
    fi
  done
  base_commit="$(git rev-parse --verify "${BASE_REF}^{commit}")"
  current_head="$(git rev-parse --verify 'HEAD^{commit}')"
  baseline_commit="$(jq -er '.source.commit' "${BASELINE_PROVENANCE}")"
  if ! [[ "${baseline_commit}" =~ ^[0-9a-f]{40}$ ]] ||
     ! git cat-file -e "${baseline_commit}^{commit}" 2>/dev/null; then
    printf 'baseline source commit is unavailable: %s\n' "${baseline_commit}" >&2
    return 1
  fi
  if [[ "${current_head}" != "${base_commit}" ]]; then
    # A topic ratchets against the audit of the protected base it was cut from,
    # or of an ancestor of it once main has moved on.
    if ! git merge-base --is-ancestor "${baseline_commit}" "${base_commit}"; then
      printf 'baseline must be an ancestor of protected main: baseline=%s base=%s\n' \
        "${baseline_commit}" "${base_commit}" >&2
      return 1
    fi
  elif [[ "${baseline_commit}" == "${current_head}" ]] ||
       ! git merge-base --is-ancestor "${baseline_commit}" "${current_head}"; then
    printf 'protected-main baseline must name a strict ancestor: baseline=%s head=%s\n' \
      "${baseline_commit}" "${current_head}" >&2
    return 1
  fi
  baseline_tree="$(git rev-parse --verify "${baseline_commit}^{tree}")"
  baseline_sha="$(sha256sum "${BASELINE_REPORT}" | awk '{print $1}')"
  jq -e \
    --slurpfile baseline "${BASELINE_REPORT}" \
    --arg baseline_commit "${baseline_commit}" \
    --arg baseline_tree "${baseline_tree}" \
    --arg baseline_sha "${baseline_sha}" \
    --arg binary_sha "${JERYU_JANKURAI_SHA256}" \
    --arg version "${JERYU_JANKURAI_VERSION}" \
    '.schema == "jeryu.jankurai-baseline-provenance/v1" and
     .repository == "jeryu/jeryu-release-ops" and
     .source.branch == "main" and
     .source.commit == $baseline_commit and
     .source.tree == $baseline_tree and
     .source.dirty_worktree == false and
     .jankurai.version == $version and
     .jankurai.binary_sha256 == $binary_sha and
     .report.path == "agent/baselines/main.repo-score.json" and
     .report.sha256 == $baseline_sha and
     .report.mode == "full" and
     .report.score == $baseline[0].score and
     .report.hard_findings == $baseline[0].decision.hard_findings and
     .report.caps == ($baseline[0].caps_applied | length) and
     .report.report_fingerprint == $baseline[0].report_fingerprint and
     .report.input_fingerprint == $baseline[0].input_fingerprint and
     .report.policy_fingerprint == $baseline[0].policy_fingerprint and
     .report.score >= 85 and .report.hard_findings == 0 and .report.caps == 0' \
    "${BASELINE_PROVENANCE}" >/dev/null
  jq -e \
    --arg full "${baseline_commit}" \
    '.git.head == $full and .git.mode == "full" and .git.dirty_worktree == false and
     .score >= 85 and (.caps_applied | length) == 0 and .decision.hard_findings == 0' \
    "${BASELINE_REPORT}" >/dev/null
}

validate_baseline

rm -rf target/jankurai
mkdir -p target/jankurai/security target/jankurai/proofbind target/jankurai/proofmark target/jankurai/rust

jankurai security run . --out target/jankurai/security/evidence.json \
  --script tools/security-lane.sh --strict --profile ci
jq -e --arg head "$(git rev-parse HEAD)" \
  '.git_head == $head and .exit_code == 0 and
   ([.commands[] | select(.status == "ran" and .exit_code == 0)] | length) >= 1' \
  target/jankurai/security/evidence.json >/dev/null

# Scope the proof to what this checkout changed against protected main; when it
# changed nothing, the tool-adoption config is the declared surface.
if [[ "$(git rev-parse HEAD)" != "$(git rev-parse "${BASE_REF}^{commit}")" ]] ||
   [[ -n "$(git status --porcelain --untracked-files=no)" ]]; then
  changed_args=(--changed-from "${BASE_REF}")
else
  changed_args=(--changed agent/tool-adoption.toml)
fi

jankurai proof "${changed_args[@]}" \
  --out target/jankurai/proof-plan.json --md target/jankurai/proof-plan.md .
jankurai proofbind map . "${changed_args[@]}" --mode advisory \
  --out target/jankurai/proofbind/surface-witness.json \
  --obligations-out target/jankurai/proofbind/obligations.json \
  --md target/jankurai/proofbind/proofbind.md
jankurai proofbind verify . "${changed_args[@]}" --mode advisory \
  --out target/jankurai/proofbind/surface-witness.json \
  --obligations-out target/jankurai/proofbind/obligations.json \
  --md target/jankurai/proofbind/proofbind.md
jankurai proofmark rust . --obligations target/jankurai/proofbind/obligations.json \
  "${changed_args[@]}" --mode advisory \
  --out target/jankurai/proofmark/proofmark-receipt.json \
  --proof-receipt target/jankurai/proofmark/proof-receipt.json \
  --md target/jankurai/proofmark/proofmark.md
jankurai copy-code . --json target/jankurai/copy-code.json --md target/jankurai/copy-code.md
jankurai rust map . --out-dir target/jankurai/rust
jankurai rust witness build . --out target/jankurai/rust/witness-graph.json

# Ratchet last, so the audit sees the evidence the tools above produced. The
# protected-main audit is the baseline; copy the validated bytes.
cp "${BASELINE_REPORT}" target/jankurai/accepted-baseline.json
jankurai audit . --mode ratchet --baseline target/jankurai/accepted-baseline.json --json target/jankurai/repo-score.json --md target/jankurai/repo-score.md --policy agent/audit-policy.toml --repair-queue-jsonl target/jankurai/repair-queue.jsonl --full --no-score-history
baseline_fingerprint="$(jq -r '.report_fingerprint' "${BASELINE_REPORT}")"
jq -e --arg fp "${baseline_fingerprint}" \
  '.score >= 85 and (.caps_applied | length) == 0 and
   .decision.passed == true and .decision.hard_findings == 0 and
   .decision.ratchet.passed == true and
   .decision.ratchet.baseline_report_fingerprint == $fp and
   (.decision.ratchet.new_caps | length) == 0 and
   (.decision.ratchet.new_hard_findings | length) == 0 and
   .decision.ratchet.policy_changed == false' \
  target/jankurai/repo-score.json >/dev/null

printf 'proof evidence ok: head=%s score=%s baseline=%s\n' \
  "$(git rev-parse HEAD)" "$(jq -r '.score' target/jankurai/repo-score.json)" \
  "$(jq -r '.source.commit' "${BASELINE_PROVENANCE}")"
