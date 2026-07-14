# Release process

`jeryu-release-ops` is a reviewed source member of the Jeryu split family;
`jeryu-deploy` remains the binary and production promotion authority. Run
`just release-readiness` before proposing a source tag or using this repository
as release evidence. That command binds the fast, check, score, security,
artifact-support, and Redline consumer contract lanes.

The Redline compatibility producer is `ops/ci/redline-consumer.sh`. It may run
only from clean, forge-equal `main` after a fresh Redline family receipt has
verified every immutable family tag. The committed lock must resolve
`redline-core-v4.1.0-jain.4` to
`3567bdced0ca1fe3671c9ebda876c914e2fc2c9e`; the producer then executes the
transaction, rollback, checkpoint, and reopen contract and writes the evidence,
test log, and checksum sidecar together.

Evidence is never accepted from a review branch, a floating ref, a historical
TOML assertion, a waived consumer, or a manually declared check. This repository
does not push images, move tags, change routes, or mutate Jain production.
