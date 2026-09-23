//! Direct coverage for `sign-release` argument parsing, defaults, and the
//! signing seed contract.

use super::*;

#[test]
fn sign_release_accepts_a_positional_artifact_path() {
    let root = temp_store_root("positional-artifact");
    let out_dir = root.join("out");
    let artifact = temp_artifact("positional-bundle.tar.gz", b"positional bundle");
    let summary = run_cli(
        &[
            "sign-release",
            &artifact.display().to_string(),
            "--repo",
            "neverhuman/veox-shared",
            "--sha",
            "abc123",
            "--version",
            "v1.0.0",
            "--rollback-target",
            "abc122",
            "--store-root",
            &root.display().to_string(),
            "--out-dir",
            &out_dir.display().to_string(),
        ],
        &[("JERYU_SIGNRAIL_ED25519_SEED", SEED)],
    )
    .unwrap_or_else(|err| panic!("sign-release failed: {err}"));
    assert!(summary.contains("\"signature_coverage_percent\":100"));
    assert!(out_dir.join("release.json").is_file());
}

#[test]
fn sign_release_applies_documented_defaults() {
    let fixture = sign_fixture("sign-defaults", &[]);
    for stage in ["local", "dev-canary", "prod"] {
        let path = fixture
            .out_dir
            .join("stage-receipts")
            .join(format!("{stage}.json"));
        let receipt = fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("read {} failed: {err}", path.display()));
        assert!(receipt.contains("\"test_status\":\"artifact-support-passed\""));
        assert!(receipt.contains("\"release_version\":\"v1.0.0\""));
    }
    let release = fixture.release_json();
    // --tree-sha defaults to --sha, and the unset provenance digests carry
    // explicit not-recorded markers rather than empty strings.
    assert!(release.contains("\"tree_sha\":\"abc123\""));
    assert!(release.contains("\"jeryu_ci_ir_hash\":\"sha256:not-recorded\""));
    assert!(release.contains("\"runner_class\":\"release-hermetic\""));
    assert!(release.contains("\"runner_rootfs_digest\":\"sha256:runner-rootfs-not-recorded\""));
    assert!(release.contains("\"toolchain_digest\":\"sha256:toolchain-not-recorded\""));
    assert!(release.contains("\"cargo_lock_digest\":\"sha256:cargo-lock-not-recorded\""));
}

#[test]
fn sign_release_emits_one_receipt_per_requested_stage() {
    let fixture = sign_fixture("sign-stages", &["--stage", "prod"]);
    let receipts = fixture.out_dir.join("stage-receipts");
    assert!(receipts.join("prod.json").is_file());
    assert!(!receipts.join("local.json").exists());
    assert!(!receipts.join("dev-canary.json").exists());
}

#[test]
fn sign_release_honours_an_explicit_key_id() {
    let fixture = sign_fixture("sign-key-id", &["--key-id", "release-station-1"]);
    assert!(
        fixture
            .release_json()
            .contains("\"key_id\":\"release-station-1\"")
    );
}

#[test]
fn sign_release_derives_media_type_from_the_artifact_extension() {
    let root = temp_store_root("sign-media-type");
    let out_dir = root.join("out");
    let artifact = root.join("manifest-bundle.json");
    fs::create_dir_all(&root).unwrap_or_else(|err| panic!("create store root failed: {err}"));
    fs::write(&artifact, b"{}").unwrap_or_else(|err| panic!("write artifact failed: {err}"));
    run_cli(
        &[
            "sign-release",
            "--artifact",
            &artifact.display().to_string(),
            "--repo",
            "neverhuman/veox-shared",
            "--sha",
            "abc123",
            "--version",
            "v1.0.0",
            "--rollback-target",
            "abc122",
            "--store-root",
            &root.display().to_string(),
            "--out-dir",
            &out_dir.display().to_string(),
        ],
        &[("JERYU_SIGNRAIL_ED25519_SEED", SEED)],
    )
    .unwrap_or_else(|err| panic!("sign-release failed: {err}"));
    let release = fs::read_to_string(out_dir.join("release.json"))
        .unwrap_or_else(|err| panic!("read release failed: {err}"));
    assert!(release.contains("\"media_type\":\"application/json\""));
}

#[test]
fn sign_release_falls_back_to_the_store_root_environment() {
    let root = temp_store_root("sign-env-store");
    let out_dir = root.join("out");
    let artifact = temp_artifact("env-store-bundle.tar.gz", b"env store bundle");
    run_cli(
        &[
            "sign-release",
            "--artifact",
            &artifact.display().to_string(),
            "--repo",
            "neverhuman/veox-shared",
            "--sha",
            "abc123",
            "--version",
            "v1.0.0",
            "--rollback-target",
            "abc122",
            "--out-dir",
            &out_dir.display().to_string(),
        ],
        &[
            ("JERYU_SIGNRAIL_ED25519_SEED", SEED),
            ("SIGNRAIL_STORE_ROOT", &root.display().to_string()),
        ],
    )
    .unwrap_or_else(|err| panic!("sign-release failed: {err}"));
    assert!(root.join("releases").is_dir());
}

#[test]
fn sign_release_derives_the_store_root_from_home() {
    let home = temp_store_root("sign-home-store");
    let out_dir = home.join("out");
    let artifact = temp_artifact("home-store-bundle.tar.gz", b"home store bundle");
    run_cli(
        &[
            "sign-release",
            "--artifact",
            &artifact.display().to_string(),
            "--repo",
            "neverhuman/veox-shared",
            "--sha",
            "abc123",
            "--version",
            "v1.0.0",
            "--rollback-target",
            "abc122",
            "--out-dir",
            &out_dir.display().to_string(),
        ],
        &[
            ("JERYU_SIGNRAIL_ED25519_SEED", SEED),
            ("HOME", &home.display().to_string()),
        ],
    )
    .unwrap_or_else(|err| panic!("sign-release failed: {err}"));
    assert!(home.join(".local/share/jeryu/signrail/releases").is_dir());
}

#[test]
fn sign_release_requires_a_store_root_it_can_resolve() {
    let artifact = temp_artifact("no-store-bundle.tar.gz", b"no store bundle");
    let err = run_cli(
        &[
            "sign-release",
            "--artifact",
            &artifact.display().to_string(),
            "--repo",
            "neverhuman/veox-shared",
            "--sha",
            "abc123",
            "--version",
            "v1.0.0",
            "--rollback-target",
            "abc122",
        ],
        &[("JERYU_SIGNRAIL_ED25519_SEED", SEED)],
    )
    .unwrap_err();
    assert!(err.starts_with("invalid input:"), "{err}");
    assert!(err.contains("SIGNRAIL_STORE_ROOT or HOME is required"));
}

#[test]
fn sign_release_reads_the_forge_seed_variable_under_github_actions() {
    let root = temp_store_root("sign-forge-seed");
    let out_dir = root.join("out");
    let artifact = temp_artifact("forge-seed-bundle.tar.gz", b"forge seed bundle");
    let args = vec![
        "sign-release".to_string(),
        "--artifact".to_string(),
        artifact.display().to_string(),
        "--repo".to_string(),
        "neverhuman/veox-shared".to_string(),
        "--sha".to_string(),
        "abc123".to_string(),
        "--version".to_string(),
        "v1.0.0".to_string(),
        "--rollback-target".to_string(),
        "abc122".to_string(),
        "--store-root".to_string(),
        root.display().to_string(),
        "--out-dir".to_string(),
        out_dir.display().to_string(),
    ];
    let borrowed = args.iter().map(String::as_str).collect::<Vec<_>>();

    let err = run_cli(
        &borrowed,
        &[
            ("GITHUB_ACTIONS", "true"),
            ("JERYU_SIGNRAIL_ED25519_SEED", SEED),
        ],
    )
    .unwrap_err();
    assert!(err.starts_with("signing unavailable:"), "{err}");
    assert!(err.contains("SIGNRAIL_ED25519_SEED is required"));
    assert!(!err.contains("JERYU_SIGNRAIL_ED25519_SEED"));

    run_cli(
        &borrowed,
        &[
            ("GITHUB_ACTIONS", "true"),
            ("SIGNRAIL_ED25519_SEED", SEED),
            (
                "GITHUB_WORKFLOW_REF",
                "acme/jeryu/.github/workflows/release.yml@refs/tags/v1.0.0",
            ),
            ("GITHUB_RUN_ID", "run-77"),
            ("RUNNER_NAME", "forge-runner-3"),
        ],
    )
    .unwrap_or_else(|err| panic!("sign-release failed: {err}"));
    let release = fs::read_to_string(out_dir.join("release.json"))
        .unwrap_or_else(|err| panic!("read release failed: {err}"));
    assert!(release.contains("run-77"));
    assert!(release.contains("forge-runner-3"));
    assert!(release.contains("refs/tags/v1.0.0"));
}

#[test]
fn sign_release_rejects_a_blank_seed() {
    let root = temp_store_root("sign-blank-seed");
    let artifact = temp_artifact("blank-seed-bundle.tar.gz", b"blank seed bundle");
    let err = run_cli(
        &[
            "sign-release",
            "--artifact",
            &artifact.display().to_string(),
            "--repo",
            "neverhuman/veox-shared",
            "--sha",
            "abc123",
            "--version",
            "v1.0.0",
            "--rollback-target",
            "abc122",
            "--store-root",
            &root.display().to_string(),
        ],
        &[("JERYU_SIGNRAIL_ED25519_SEED", "   ")],
    )
    .unwrap_err();
    assert!(err.starts_with("signing unavailable:"), "{err}");
}

#[test]
fn sign_release_reports_each_missing_required_flag() {
    let artifact = temp_artifact("required-flags-bundle.tar.gz", b"required flags bundle");
    let artifact_arg = artifact.display().to_string();
    let root = temp_store_root("sign-required-flags");
    let root_arg = root.display().to_string();
    let full = [
        ("--repo", "neverhuman/veox-shared"),
        ("--sha", "abc123"),
        ("--version", "v1.0.0"),
        ("--rollback-target", "abc122"),
    ];
    for skipped in full.iter().map(|(flag, _)| *flag) {
        let mut args = vec![
            "sign-release",
            "--artifact",
            artifact_arg.as_str(),
            "--store-root",
            root_arg.as_str(),
        ];
        for (flag, value) in &full {
            if *flag != skipped {
                args.push(flag);
                args.push(value);
            }
        }
        let err = run_cli(&args, &[("JERYU_SIGNRAIL_ED25519_SEED", SEED)]).unwrap_err();
        assert!(err.starts_with("invalid input:"), "{err}");
        assert!(
            err.contains(&format!("missing required {skipped}")),
            "expected a missing {skipped} report, got {err}"
        );
        assert!(err.contains("usage: jeryu_signrail sign-release"));
    }
}

#[test]
fn sign_release_requires_an_artifact() {
    let err = run_cli(
        &[
            "sign-release",
            "--repo",
            "neverhuman/veox-shared",
            "--sha",
            "abc123",
            "--version",
            "v1.0.0",
            "--rollback-target",
            "abc122",
        ],
        &[("JERYU_SIGNRAIL_ED25519_SEED", SEED)],
    )
    .unwrap_err();
    assert!(err.starts_with("invalid input:"), "{err}");
    assert!(err.contains("usage: jeryu_signrail sign-release"));
}

#[test]
fn sign_release_rejects_a_blank_required_value() {
    let artifact = temp_artifact("blank-repo-bundle.tar.gz", b"blank repo bundle");
    let err = run_cli(
        &[
            "sign-release",
            "--artifact",
            &artifact.display().to_string(),
            "--repo",
            "  ",
            "--sha",
            "abc123",
            "--version",
            "v1.0.0",
            "--rollback-target",
            "abc122",
        ],
        &[("JERYU_SIGNRAIL_ED25519_SEED", SEED)],
    )
    .unwrap_err();
    assert!(err.contains("missing required --repo"), "{err}");
}

#[test]
fn sign_release_rejects_unknown_options_and_dangling_values() {
    let unknown = run_cli(&["sign-release", "--nope", "x"], &[]).unwrap_err();
    assert!(unknown.starts_with("invalid input:"), "{unknown}");
    assert!(unknown.contains("unknown sign-release option --nope"));
    assert!(unknown.contains("usage: jeryu_signrail sign-release"));

    let dangling = run_cli(&["sign-release", "--repo"], &[]).unwrap_err();
    assert!(dangling.contains("missing value for --repo"), "{dangling}");

    let help = run_cli(&["sign-release", "--help"], &[]).unwrap_err();
    assert!(
        help.contains("usage: jeryu_signrail sign-release"),
        "{help}"
    );
}

#[test]
fn sign_release_rejects_a_non_numeric_created_at_epoch() {
    let err = run_cli(&["sign-release", "--created-at-epoch", "yesterday"], &[]).unwrap_err();
    assert!(err.starts_with("invalid input:"), "{err}");
    assert!(err.contains("invalid --created-at-epoch"));
}

#[test]
fn sign_release_is_reproducible_for_a_fixed_epoch_and_seed() {
    let artifact = temp_artifact("repro-bundle.tar.gz", b"repro bundle bytes");
    let artifact_arg = artifact.display().to_string();
    let sign_into = |root: &std::path::Path| {
        let out_dir = root.join("out");
        run_cli(
            &[
                "sign-release",
                "--artifact",
                artifact_arg.as_str(),
                "--repo",
                "neverhuman/veox-shared",
                "--sha",
                "abc123",
                "--version",
                "v1.0.0",
                "--rollback-target",
                "abc122",
                "--store-root",
                &root.display().to_string(),
                "--out-dir",
                &out_dir.display().to_string(),
                "--created-at-epoch",
                "100",
            ],
            &[("JERYU_SIGNRAIL_ED25519_SEED", SEED)],
        )
        .unwrap_or_else(|err| panic!("sign-release failed: {err}"));
        fs::read_to_string(out_dir.join("release.json"))
            .unwrap_or_else(|err| panic!("read release failed: {err}"))
    };
    let first = sign_into(&temp_store_root("sign-repro-a"));
    let second = sign_into(&temp_store_root("sign-repro-b"));
    assert_eq!(first, second);
}
