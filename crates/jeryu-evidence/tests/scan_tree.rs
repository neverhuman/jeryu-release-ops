//! Fixture-tree tests for the zero-evidence guard.
//!
//! These drive [`scan`] over a multi-directory workspace that mixes planted
//! markers with near-misses, rather than exercising one file at a time.
//!
//! Forbidden tokens are injected via hex-decoded bytes so this test source
//! never contains the literal markers (preserving the self-clean property).

use std::fs;
use std::path::{Path, PathBuf};

use jeryu_evidence::{Finding, scan};
use tempfile::TempDir;

/// Every forbidden marker, hex-encoded, in the order the scanner stores them.
const MARKER_HEX: &[&str] = &[
    "6769746c6162",
    "676974206c6162",
    "6769742d6c6162",
    "2e6769746c61622d63692e796d6c",
    "676c6162",
    "6a6974666f726765",
    "6e6974726f",
    "63726174657661756c74",
    "6d6972726f727661756c74",
    "62656e63686c6162",
];

/// One near-miss per marker: the same token with a single byte changed, so it
/// reads like a brand name without being one. Each entry is paired with the
/// [`MARKER_HEX`] entry it shadows.
const NEAR_MISS_HEX: &[&str] = &[
    "6769746c6163",
    "676974206c6163",
    "6769742d6c6163",
    "2e67697a6c61622d63692e796d6c",
    "676c6163",
    "6a6974666f72677a",
    "6e69747265",
    "63726174657661756c7a",
    "6d6972726f727661756c7a",
    "62656e63686c6163",
];

/// Decode a hex marker into its raw bytes for fixture injection.
fn marker(hexed: &str) -> Vec<u8> {
    hex::decode(hexed).expect("valid hex marker")
}

/// Write `contents` to `root/rel`, creating parent directories as needed.
fn plant(root: &Path, rel: &str, contents: &[u8]) {
    let path = root.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("mkdir fixture parent");
    }
    fs::write(&path, contents).expect("write fixture file");
}

/// A line of prose with `token` spliced into the middle.
fn line_with(token: &[u8]) -> Vec<u8> {
    let mut out = b"the release notes mention ".to_vec();
    out.extend_from_slice(token);
    out.extend_from_slice(b" in passing\n");
    out
}

/// Findings as sorted `(rel, line)` pairs, so assertions do not depend on
/// directory walk order.
fn sorted(findings: &[Finding]) -> Vec<(PathBuf, usize)> {
    let mut out: Vec<(PathBuf, usize)> = findings
        .iter()
        .map(|finding| (finding.rel.clone(), finding.line))
        .collect();
    out.sort();
    out
}

#[test]
fn fixture_tree_reports_only_the_planted_markers() {
    let dir = TempDir::new().expect("tempdir");
    let root = dir.path();

    // Clean product source and docs.
    plant(root, "README.md", b"# workspace\n\na hosted forge, nothing more\n");
    plant(root, "src/main.rs", b"fn main() {\n    println!(\"ok\");\n}\n");
    plant(root, "docs/guide/intro.md", b"an introduction\nwith two lines\n");

    // Near-misses: close enough to read like brand names, but not markers.
    let mut near = b"names we are allowed to use:\n".to_vec();
    for hexed in NEAR_MISS_HEX {
        near.extend_from_slice(&line_with(&marker(hexed)));
    }
    plant(root, "docs/guide/naming.md", &near);

    // Planted markers, each on a known line of a known file.
    let mut pipeline = b"stages:\n  - build\n".to_vec();
    pipeline.extend_from_slice(&line_with(&marker(MARKER_HEX[3])));
    plant(root, "ci/pipeline.yml", &pipeline);

    let mut vendor = b"third-party inventory\n".to_vec();
    vendor.extend_from_slice(&line_with(&marker(MARKER_HEX[6])));
    plant(root, "vendor/inventory.txt", &vendor);

    let mut nested = b"deep note\nstill fine\n".to_vec();
    nested.extend_from_slice(&line_with(&marker(MARKER_HEX[9])));
    plant(root, "docs/guide/deep/vendors.md", &nested);

    // Skipped subtrees and generated files: planted but never reported.
    plant(root, "target/debug/build.log", &line_with(&marker(MARKER_HEX[0])));
    plant(root, "web/node_modules/pkg/readme.md", &line_with(&marker(MARKER_HEX[0])));
    plant(root, "web/dist/bundle.js", &line_with(&marker(MARKER_HEX[0])));
    plant(root, "web/app.tsbuildinfo", &line_with(&marker(MARKER_HEX[0])));
    plant(root, "AGENT_CHAT.md", &line_with(&marker(MARKER_HEX[0])));

    let findings = scan(root).expect("scan ok");
    assert_eq!(
        sorted(&findings),
        vec![
            (PathBuf::from("ci/pipeline.yml"), 3),
            (PathBuf::from("docs/guide/deep/vendors.md"), 3),
            (PathBuf::from("vendor/inventory.txt"), 2),
        ],
        "unexpected findings: {findings:?}"
    );
}

#[test]
fn every_marker_is_detected_in_its_own_file() {
    let dir = TempDir::new().expect("tempdir");
    let root = dir.path();

    for (index, hexed) in MARKER_HEX.iter().enumerate() {
        plant(
            root,
            &format!("case/{index}/notes.md"),
            &line_with(&marker(hexed)),
        );
    }

    let findings = scan(root).expect("scan ok");
    let expected: Vec<(PathBuf, usize)> = (0..MARKER_HEX.len())
        .map(|index| (PathBuf::from(format!("case/{index}/notes.md")), 1))
        .collect();
    let mut expected = expected;
    expected.sort();
    assert_eq!(sorted(&findings), expected, "a marker went unreported");
}

#[test]
fn near_misses_alone_produce_no_findings() {
    let dir = TempDir::new().expect("tempdir");
    let root = dir.path();

    for (index, hexed) in NEAR_MISS_HEX.iter().enumerate() {
        plant(
            root,
            &format!("case/{index}/naming.md"),
            &line_with(&marker(hexed)),
        );
    }

    let findings = scan(root).expect("scan ok");
    assert!(
        findings.is_empty(),
        "a near-miss token was reported as a marker: {findings:?}"
    );
}

#[test]
fn exempt_file_name_applies_only_at_the_scan_root() {
    let dir = TempDir::new().expect("tempdir");
    let root = dir.path();

    plant(root, "AGENT_CHAT.md", &line_with(&marker(MARKER_HEX[0])));
    plant(root, "agent/AGENT_CHAT.md", &line_with(&marker(MARKER_HEX[0])));

    let findings = scan(root).expect("scan ok");
    assert_eq!(
        sorted(&findings),
        vec![(PathBuf::from("agent/AGENT_CHAT.md"), 1)],
        "the exemption is scoped to the root file: {findings:?}"
    );
}

#[test]
fn missing_root_reports_the_offending_path() {
    let dir = TempDir::new().expect("tempdir");
    let missing = dir.path().join("absent");

    let err = scan(&missing).expect_err("scan of a missing root fails");
    let rendered = err.to_string();
    assert!(
        rendered.contains(&missing.display().to_string()),
        "error should name the unreadable path: {rendered}"
    );
}
