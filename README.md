# jeryu-release-ops

[![Jankurai score: 88/100](https://img.shields.io/badge/Jankurai-88%2F100-brightgreen)](ops/ci/score.sh)

Release, signing, governance, observability, and compliance tooling.

This repository was seeded from Jeryu source commit `cbecf7caa0e932c76a341b2521e66e911233860d` by
`ops/split/materialize.py`. It is part of the seven-repo Jeryu split family and keeps source
paths stable where practical so ownership remains auditable.

Repository-specific agent instructions and ownership boundaries start in
[`AGENTS.md`](AGENTS.md).

## Getting Started

Install the Rust toolchain declared by `rust-toolchain.toml` and
[`just`](https://github.com/casey/just), then run the deterministic local gate:

```bash
just fast
```

Before proposing release evidence, run the complete source-readiness lane:

```bash
just release-readiness
```

That lane is non-promoting: deployment and production mutation remain owned by
`jeryu-deploy`.

## Owned Cargo Packages

- `crates/jeryu-signing`
- `crates/jeryu-signrail`
- `crates/jeryu-wsversion`
- `crates/jeryu-repogate`
- `crates/jeryu-mapcheck`
- `crates/jeryu-evidence`
- `crates/jeryu-obs`
- `crates/jeryu-bench`
- `crates/jeryu-ops`
- `crates/jeryu-phase11-core`
- `crates/jeryu-phase11-audit`
- `crates/jeryu-compliance-export`
- `crates/jeryu-lifecycle`
- `crates/jeryu-tenant`
- `crates/jeryu-kernel`
- `crates/jeryu-replay-verifier`
- `crates/jeryu-git-guard`
- `bins/jeryu-phase11-bin`

## Source Coverage

- `crates/jeryu-signing/**`
- `crates/jeryu-signrail/**`
- `crates/jeryu-wsversion/**`
- `crates/jeryu-repogate/**`
- `crates/jeryu-mapcheck/**`
- `crates/jeryu-evidence/**`
- `crates/jeryu-obs/**`
- `crates/jeryu-bench/**`
- `crates/jeryu-ops/**`
- `crates/jeryu-phase11-core/**`
- `crates/jeryu-phase11-audit/**`
- `crates/jeryu-compliance-export/**`
- `crates/jeryu-lifecycle/**`
- `crates/jeryu-tenant/**`
- `crates/jeryu-kernel/**`
- `crates/jeryu-replay-verifier/**`
- `crates/jeryu-git-guard/**`
- `bins/jeryu-phase11-bin/**`
- `bench/**`
- `dashboards/**`
- `ops/signrail-verify/**`
- `ops/upgrade/**`
- `ops/bench/**`
- `ops/security/**`
- `ops/chaos/**`
- `tools/**`
- `fixtures/upgrade/**`
- `fixtures/security/**`
- `fixtures/chaos/**`
- `fixtures/sso/**`
- `fixtures/benchmarks/**`
- `configs/signrail.example.toml`

## Local Commands

- `just fast`
- `just check`
- `just score`
- `just security`
- `just artifact-support`
- `just redline-consumer-test`

`ops/ci/redline-consumer.sh` emits checksummed Redline compatibility evidence
only from clean, forge-equal `main`, a passing family receipt, and the exact
immutable engine tag resolved by `Cargo.lock`.
