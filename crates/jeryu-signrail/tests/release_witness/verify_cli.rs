//! Direct coverage for `verify-release` argument parsing and the checks it
//! must fail closed on.

use super::*;

fn verify_args<'a>(
    release: &'a str,
    stage: &'a str,
    root: &'a str,
    pubkey: &'a str,
) -> Vec<&'a str> {
    vec![
        "verify-release",
        "--release",
        release,
        "--stage",
        stage,
        "--store-root",
        root,
        "--pubkey-file",
        pubkey,
    ]
}

#[test]
fn verify_release_reports_a_human_readable_summary_without_json() {
    let fixture = sign_fixture("verify-human", &[]);
    let pubkey = fixture.write_pubkey(SEED);
    let verified = run_cli(
        &verify_args(
            &fixture.out_dir.join("release.json").display().to_string(),
            "dev-canary",
            &fixture.root.display().to_string(),
            &pubkey.display().to_string(),
        ),
        &[],
    )
    .unwrap_or_else(|err| panic!("verify-release failed: {err}"));
    assert_eq!(
        verified,
        "verified release neverhuman/veox-shared@abc123 stage dev-canary \
         (1 artifacts, 100% signature coverage)"
    );
}

#[test]
fn verify_release_rejects_a_public_key_that_did_not_sign_the_release() {
    let fixture = sign_fixture("verify-wrong-key", &[]);
    let other = jeryu_signrail::Ed25519Signer::from_seed_hex(None, &"77".repeat(32))
        .unwrap_or_else(|err| panic!("signer failed: {err}"));
    let pubkey = fixture.root.join("other.pub");
    fs::write(&pubkey, other.public_key_hex())
        .unwrap_or_else(|err| panic!("write pubkey failed: {err}"));
    let err = run_cli(
        &verify_args(
            &fixture.out_dir.join("release.json").display().to_string(),
            "prod",
            &fixture.root.display().to_string(),
            &pubkey.display().to_string(),
        ),
        &[],
    )
    .unwrap_err();
    assert!(err.starts_with("verification failed:"), "{err}");
    assert!(err.contains("provenance signature mismatch"));
}

#[test]
fn verify_release_reads_the_last_token_of_the_public_key_file() {
    let fixture = sign_fixture("verify-pubkey-tokens", &[]);
    let signer = jeryu_signrail::Ed25519Signer::from_seed_hex(None, SEED)
        .unwrap_or_else(|err| panic!("signer failed: {err}"));
    let pubkey = fixture.root.join("ssh-style.pub");
    fs::write(
        &pubkey,
        format!("signrail-ed25519 {}\n", signer.public_key_hex()),
    )
    .unwrap_or_else(|err| panic!("write pubkey failed: {err}"));
    run_cli(
        &verify_args(
            &fixture.out_dir.join("release.json").display().to_string(),
            "prod",
            &fixture.root.display().to_string(),
            &pubkey.display().to_string(),
        ),
        &[],
    )
    .unwrap_or_else(|err| panic!("verify-release failed: {err}"));
}

#[test]
fn verify_release_rejects_an_empty_public_key_file() {
    let fixture = sign_fixture("verify-empty-pubkey", &[]);
    let pubkey = fixture.root.join("empty.pub");
    fs::write(&pubkey, "  \n").unwrap_or_else(|err| panic!("write pubkey failed: {err}"));
    let err = run_cli(
        &verify_args(
            &fixture.out_dir.join("release.json").display().to_string(),
            "prod",
            &fixture.root.display().to_string(),
            &pubkey.display().to_string(),
        ),
        &[],
    )
    .unwrap_err();
    assert!(err.contains("empty public key file"), "{err}");
}

#[test]
fn verify_release_requires_a_receipt_for_the_requested_stage() {
    let fixture = sign_fixture("verify-missing-stage", &["--stage", "local"]);
    let pubkey = fixture.write_pubkey(SEED);
    let err = run_cli(
        &verify_args(
            &fixture.out_dir.join("release.json").display().to_string(),
            "prod",
            &fixture.root.display().to_string(),
            &pubkey.display().to_string(),
        ),
        &[],
    )
    .unwrap_err();
    assert!(err.starts_with("verification failed:"), "{err}");
    assert!(err.contains("stage receipt missing or unreadable"));
}

#[test]
fn verify_release_requires_the_release_to_be_in_the_store() {
    let fixture = sign_fixture("verify-missing-store", &[]);
    let pubkey = fixture.write_pubkey(SEED);
    let empty_root = temp_store_root("verify-empty-store");
    let err = run_cli(
        &verify_args(
            &fixture.out_dir.join("release.json").display().to_string(),
            "prod",
            &empty_root.display().to_string(),
            &pubkey.display().to_string(),
        ),
        &[],
    )
    .unwrap_err();
    assert!(err.starts_with("verification failed:"), "{err}");
    assert!(err.contains("stored release JSON missing"));
    // The store path is derived from the release id with `/` flattened.
    assert!(err.contains("neverhuman_veox-shared@abc123.json"));
}

#[test]
fn verify_release_rejects_a_stage_receipt_for_another_commit() {
    let fixture = sign_fixture("verify-receipt-sha", &[]);
    let pubkey = fixture.write_pubkey(SEED);
    let receipt_path = fixture
        .root
        .join("receipts")
        .join("neverhuman_veox-shared@abc123-prod.json");
    let receipt = fs::read_to_string(&receipt_path)
        .unwrap_or_else(|err| panic!("read receipt failed: {err}"));
    fs::write(
        &receipt_path,
        receipt.replace("\"sha\":\"abc123\"", "\"sha\":\"deadbee\""),
    )
    .unwrap_or_else(|err| panic!("write receipt failed: {err}"));
    let err = run_cli(
        &verify_args(
            &fixture.out_dir.join("release.json").display().to_string(),
            "prod",
            &fixture.root.display().to_string(),
            &pubkey.display().to_string(),
        ),
        &[],
    )
    .unwrap_err();
    assert!(err.contains("stage receipt sha mismatch"), "{err}");
}

#[test]
fn verify_release_rejects_a_stage_receipt_below_full_signature_coverage() {
    let fixture = sign_fixture("verify-receipt-coverage", &[]);
    let pubkey = fixture.write_pubkey(SEED);
    let receipt_path = fixture
        .root
        .join("receipts")
        .join("neverhuman_veox-shared@abc123-prod.json");
    let receipt = fs::read_to_string(&receipt_path)
        .unwrap_or_else(|err| panic!("read receipt failed: {err}"));
    fs::write(
        &receipt_path,
        receipt.replace(
            "\"signature_coverage_percent\":100",
            "\"signature_coverage_percent\":50",
        ),
    )
    .unwrap_or_else(|err| panic!("write receipt failed: {err}"));
    let err = run_cli(
        &verify_args(
            &fixture.out_dir.join("release.json").display().to_string(),
            "prod",
            &fixture.root.display().to_string(),
            &pubkey.display().to_string(),
        ),
        &[],
    )
    .unwrap_err();
    assert!(err.starts_with("policy block:"), "{err}");
    assert!(err.contains("stage receipt signature coverage is not 100%"));
}

#[test]
fn verify_release_rejects_a_release_with_unsigned_artifacts() {
    let fixture = sign_fixture("verify-coverage-gap", &[]);
    let pubkey = fixture.write_pubkey(SEED);
    let release_path = fixture.out_dir.join("release.json");
    let release = fixture.release_json();
    fs::write(
        &release_path,
        release.replace("\"provenance\":[", "\"provenance\":[] , \"unused\":["),
    )
    .unwrap_or_else(|err| panic!("write release failed: {err}"));
    let err = run_cli(
        &verify_args(
            &release_path.display().to_string(),
            "prod",
            &fixture.root.display().to_string(),
            &pubkey.display().to_string(),
        ),
        &[],
    )
    .unwrap_err();
    assert!(err.starts_with("policy block:"), "{err}");
    assert!(err.contains("signature coverage is not 100%"));
}

#[test]
fn verify_release_rejects_a_release_without_artifacts() {
    let fixture = sign_fixture("verify-no-artifacts", &[]);
    let pubkey = fixture.write_pubkey(SEED);
    let release_path = fixture.root.join("empty-artifacts.json");
    fs::write(
        &release_path,
        "{\"id\":\"neverhuman/veox-shared@abc123\",\"commit_sha\":\"abc123\",\
         \"artifacts\":[],\"provenance\":[]}",
    )
    .unwrap_or_else(|err| panic!("write release failed: {err}"));
    let err = run_cli(
        &verify_args(
            &release_path.display().to_string(),
            "prod",
            &fixture.root.display().to_string(),
            &pubkey.display().to_string(),
        ),
        &[],
    )
    .unwrap_err();
    assert!(err.starts_with("policy block:"), "{err}");
    assert!(err.contains("release has no artifacts to verify"));
}

#[test]
fn verify_release_rejects_malformed_release_json() {
    let fixture = sign_fixture("verify-bad-json", &[]);
    let pubkey = fixture.write_pubkey(SEED);
    let release_path = fixture.root.join("not-json.json");
    fs::write(&release_path, "{").unwrap_or_else(|err| panic!("write release failed: {err}"));
    let err = run_cli(
        &verify_args(
            &release_path.display().to_string(),
            "prod",
            &fixture.root.display().to_string(),
            &pubkey.display().to_string(),
        ),
        &[],
    )
    .unwrap_err();
    assert!(err.starts_with("invalid input:"), "{err}");
    assert!(err.contains("release JSON parse failed"));
}

#[test]
fn verify_release_accepts_only_the_declared_stages() {
    let err = run_cli(
        &verify_args("release.json", "staging", "store", "key.pub"),
        &[],
    )
    .unwrap_err();
    assert!(err.starts_with("invalid input:"), "{err}");
    assert!(err.contains("--stage must be local, dev-canary, or prod (got staging)"));

    for stage in ["local", "dev-canary", "prod"] {
        let err = run_cli(&["verify-release", "--stage", stage], &[]).unwrap_err();
        assert!(
            err.contains("usage: jeryu_signrail verify-release"),
            "expected a usage report for stage {stage}, got {err}"
        );
    }
}

#[test]
fn verify_release_requires_every_path_flag() {
    let all = [
        ("--release", "release.json"),
        ("--store-root", "store"),
        ("--pubkey-file", "key.pub"),
    ];
    for skipped in all.iter().map(|(flag, _)| *flag) {
        let mut args = vec!["verify-release", "--stage", "prod"];
        for (flag, value) in &all {
            if *flag != skipped {
                args.push(flag);
                args.push(value);
            }
        }
        let err = run_cli(&args, &[]).unwrap_err();
        assert!(err.starts_with("invalid input:"), "{err}");
        assert!(
            err.contains("usage: jeryu_signrail verify-release"),
            "expected a usage report when {skipped} is absent, got {err}"
        );
    }
}

#[test]
fn verify_release_rejects_unknown_options_and_dangling_values() {
    let unknown = run_cli(&["verify-release", "--nope", "x"], &[]).unwrap_err();
    assert!(
        unknown.contains("unknown verify-release option --nope"),
        "{unknown}"
    );
    assert!(unknown.contains("usage: jeryu_signrail verify-release"));

    let dangling = run_cli(&["verify-release", "--stage"], &[]).unwrap_err();
    assert!(dangling.contains("missing value for --stage"), "{dangling}");

    let help = run_cli(&["verify-release", "--help"], &[]).unwrap_err();
    assert!(
        help.contains("usage: jeryu_signrail verify-release"),
        "{help}"
    );
}

#[test]
fn verify_release_requires_a_stage() {
    let err = run_cli(
        &[
            "verify-release",
            "--release",
            "release.json",
            "--store-root",
            "store",
            "--pubkey-file",
            "key.pub",
        ],
        &[],
    )
    .unwrap_err();
    assert!(err.contains("missing required --stage"), "{err}");
}
