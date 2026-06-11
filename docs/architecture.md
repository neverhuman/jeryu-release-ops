# Architecture

`jeryu-release-ops` is part of the Jeryu split family.

The public portal is `neverhuman/jeryu`. Release authority remains
`neverhuman/jeryu-deploy`; split member repositories own bounded product
surfaces and consume sibling crates from pinned public Git tags.

## Boundaries

- Profile: `rust-workspace`
- Required check: `jeryu-release-ops/required`
- Local release source of truth: `agent/boundaries.toml`

## Owned Surface

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
