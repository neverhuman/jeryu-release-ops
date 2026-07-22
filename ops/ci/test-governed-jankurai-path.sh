#!/usr/bin/env bash
# Fail-closed selection tests for the release broker's governed Jankurai path.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${here}/../.." && pwd)"
source_lib="${here}/lib.sh"
source_verifier="${here}/ensure-jankurai.sh"
production_broker="/opt/jain-ci/authority/release-bin/jankurai"
tmp="$(mktemp -d /tmp/test-governed-jankurai-path.XXXXXX)"
cleanup() {
  rm -rf -- "${tmp}"
}
trap cleanup EXIT

fail() {
  printf 'test-governed-jankurai-path: %s\n' "$*" >&2
  exit 1
}

expect_failure() {
  local description="$1" pattern="$2"
  shift 2
  if "$@" >"${tmp}/failure.log" 2>&1; then
    fail "${description}: command unexpectedly succeeded"
  fi
  grep -Fq "${pattern}" "${tmp}/failure.log" || {
    sed -n '1,80p' "${tmp}/failure.log" >&2
    fail "${description}: expected failure text was absent"
  }
}

for source in \
  "${source_lib}" \
  "${source_verifier}"; do
  grep -Fq "${production_broker}" "${source}" ||
    fail "release broker path contract is absent from ${source#"${repo_root}/"}"
  grep -Fq 'local mode=receipt-bound' "${source}" ||
    fail "release broker mode is absent from ${source#"${repo_root}/"}"
done
for entrypoint in \
  "${source_lib}" \
  "${source_verifier}" \
  "${here}/pr-ci.sh" \
  "${repo_root}/scripts/ci-doctor.sh"; do
  if grep -Eq '/home/ubuntu/\.jeryu/bin/jankurai|JERYU_JANKURAI_BIN:-' \
    "${entrypoint}"; then
    fail "${entrypoint#"${repo_root}/"} still selects an ambient-home or caller-provided auditor"
  fi
done

mkdir -p "${tmp}/broker/bin" "${tmp}/attacker/bin" \
  "${tmp}/home/.jeryu/bin"
governed_source="/usr/local/libexec/jain/jankurai"
if [[ ! -x "${governed_source}" ]]; then
  governed_source="$(command -v jankurai 2>/dev/null || true)"
fi
[[ "${governed_source}" == /* && -f "${governed_source}" &&
   ! -L "${governed_source}" && -x "${governed_source}" ]] ||
  fail "governed Jankurai test source is unavailable"
[[ "$("${governed_source}" --version)" == 'jankurai 1.6.11' ]] ||
  fail "governed Jankurai test source has the wrong version"
[[ "$(sha256sum "${governed_source}" | awk '{print $1}')" == \
   '96d99e6e7d8dc9cf23df1081edd1f975231456592f81d9405385219a2c7298aa' ]] ||
  fail "governed Jankurai test source has the wrong digest"

broker_bin="${tmp}/broker/bin/jankurai"
attacker_bin="${tmp}/attacker/bin/jankurai"
ambient_bin="${tmp}/home/.jeryu/bin/jankurai"
cp -- "${governed_source}" "${broker_bin}"
cp -- "${governed_source}" "${attacker_bin}"
cp -- "${governed_source}" "${ambient_bin}"
chmod 0555 "${broker_bin}" "${attacker_bin}" "${ambient_bin}"

# Exercise the exact production logic without requiring write access beneath
# /opt: only this automatically removed test copy substitutes the fixed broker
# path, while every other byte remains the reviewed source.
test_lib="${tmp}/lib.sh"
sed "s#${production_broker}#${broker_bin}#g" "${source_lib}" >"${test_lib}"
test_verifier="${tmp}/ensure-jankurai.sh"
sed "s#${production_broker}#${broker_bin}#g" "${source_verifier}" >"${test_verifier}"

run_release_broker() {
  local path="$1"
  local release_command
  shift
  # The child shell expands its positional inputs.
  # shellcheck disable=SC2016
  release_command='source "$1"; require_jankurai; [[ "$JERYU_GOVERNED_JANKURAI_BIN" == "$2" ]]'
  env -i HOME="${tmp}/home" PATH="${path}:/usr/bin:/bin" \
    JAIN_RELEASE_CI=1 "$@" bash -c \
    "${release_command}" \
    bash "${test_lib}" "${broker_bin}"
}

run_release_broker "${tmp}/broker/bin"
env -i HOME="${tmp}/home" PATH="${tmp}/broker/bin:/usr/bin:/bin" \
  JAIN_RELEASE_CI=1 bash "${test_verifier}" >/dev/null

# Caller substitutions never select the auditor: the broker-controlled PATH and
# fixed authority path remain decisive.
run_release_broker "${tmp}/broker/bin" \
  JERYU_GOVERNED_JANKURAI_BIN="${attacker_bin}" \
  JERYU_JANKURAI_BIN="${attacker_bin}"
expect_failure "caller receipt substitution" \
  "release broker Jankurai rejects caller receipt authority" \
  run_release_broker "${tmp}/broker/bin" \
  JERYU_JANKURAI_RECEIPT="${tmp}/caller-receipt.json" \
  JERYU_JANKURAI_RECEIPT_SHA256="$(printf 'a%.0s' {1..64})" \
  JERYU_JANKURAI_ALLOW_TEST_RECEIPT=1

expect_failure "ambient home auditor" "release broker Jankurai path mismatch" \
  run_release_broker "${tmp}/home/.jeryu/bin"
expect_failure "caller PATH substitution" "release broker Jankurai path mismatch" \
  run_release_broker "${tmp}/attacker/bin"
expect_failure "missing broker auditor" "release broker Jankurai path mismatch" \
  run_release_broker "/usr/bin:/bin"

cp -- "${broker_bin}" "${tmp}/governed-backup"
chmod 0755 "${broker_bin}"
printf '#!/usr/bin/env bash\nprintf "jankurai 1.6.11\\n"\n' >"${broker_bin}"
chmod 0555 "${broker_bin}"
expect_failure "wrong broker binary" "governed jankurai identity mismatch" \
  run_release_broker "${tmp}/broker/bin"
mv -- "${tmp}/governed-backup" "${broker_bin}"
chmod 0555 "${broker_bin}"

chmod 0755 "${broker_bin}"
expect_failure "writable broker binary" "release broker Jankurai custody mismatch" \
  run_release_broker "${tmp}/broker/bin"
chmod 0555 "${broker_bin}"

ln "${broker_bin}" "${tmp}/broker/bin/jankurai-linked"
expect_failure "linked broker binary" "release broker Jankurai custody mismatch" \
  run_release_broker "${tmp}/broker/bin"
rm -f -- "${tmp}/broker/bin/jankurai-linked"

printf 'governed Jankurai path tests passed: broker ambient env path receipt missing identity custody\n'
