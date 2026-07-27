use super::*;

const RAW: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../repos.manifest.toml"
));

fn canonical() -> Manifest {
    toml::from_str(RAW).expect("canonical manifest parses")
}

#[test]
fn canonical_authority_passes() {
    validate(&canonical()).unwrap();
}

#[test]
fn rejects_old_root_duplicate_redline_and_slug_alias() {
    for hostile in [
        RAW.replace(SPLIT_ROOT, "/home/ubuntu/jeryu-split"),
        RAW.replace(
            REDLINE_ROOT,
            "/home/ubuntu/jain-split/jeryu-split/jeryu-redline",
        ),
        RAW.replacen("jeryu/jeryu-cache", "veox/jeryu-cache", 1),
    ] {
        let manifest: Manifest = toml::from_str(&hostile).unwrap();
        assert!(validate(&manifest).is_err());
    }
}

#[test]
fn rejects_duplicate_and_retired_active_identity() {
    let mut duplicate = canonical();
    duplicate.repo[1].path = duplicate.repo[0].path.clone();
    assert!(validate(&duplicate).is_err());

    let mut retired = canonical();
    retired.retired_histories.push("jeryu-web".to_owned());
    assert!(validate(&retired).is_err());
}

#[test]
fn rejects_unknown_fields_and_self_consistent_identity_substitution() {
    assert!(toml::from_str::<Manifest>(&format!("unknown = true\n{RAW}")).is_err());
    assert!(toml::from_str::<Manifest>(&format!("{RAW}\nunknown = true\n")).is_err());

    let mut substituted = canonical();
    let repo = &mut substituted.repo[0];
    repo.name = "jeryu-unknown".to_owned();
    repo.path = format!("{SPLIT_ROOT}/jeryu-unknown");
    repo.jeryu_slug = "jeryu/jeryu-unknown".to_owned();
    repo.remote = "http://127.0.0.1:8787/git/jeryu/jeryu-unknown.git".to_owned();
    repo.required_check = "jeryu-unknown/required".to_owned();
    repo.current_tag = Some("jeryu-unknown-v5.0.0-split.0".to_owned());
    substituted.required_repos[0] = "jeryu-unknown".to_owned();
    assert!(validate(&substituted).is_err());
}

#[test]
fn accepts_only_the_governed_web_retirement_transition() {
    let mut retired = canonical();
    retired.repo.retain(|repo| repo.name != "jeryu-web");
    retired.required_repos.retain(|repo| repo != "jeryu-web");
    retired.retired_histories.push("jeryu-web".to_owned());
    validate(&retired).unwrap();

    retired.retired_histories[0] = "jeryu-core".to_owned();
    assert!(validate(&retired).is_err());
}

#[test]
fn rejects_control_plane_work_and_malformed_tag_identity() {
    let mut control = canonical();
    control.control_plane.product_name = Some("Work".to_owned());
    assert!(validate(&control).is_err());

    let mut work = canonical();
    work.repo
        .iter_mut()
        .find(|repo| repo.name == "jeryu-jira")
        .unwrap()
        .product_name = Some("Jira".to_owned());
    assert!(validate(&work).is_err());

    let mut tag = canonical();
    tag.repo[0].current_tag = Some("jeryu-v5.latest-split.next".to_owned());
    assert!(validate(&tag).is_err());
}

#[test]
fn rejects_stale_or_invented_control_plane_tags() {
    for hostile in [
        "jeryu-release-ops-v5.0.0-split.0",
        "jeryu-release-ops-v5.0.0-split.1",
        "jeryu-release-ops-v5.0.0-split.2",
        "jeryu-release-ops-v5.0.0-split.4",
    ] {
        let mut manifest = canonical();
        manifest.control_plane.predecessor_tag = hostile.to_owned();
        assert!(validate(&manifest).is_err());
    }
}

#[test]
fn control_plane_and_product_tag_roles_cannot_be_mixed() {
    assert!(
        toml::from_str::<Manifest>(&RAW.replacen("predecessor_tag", "current_tag", 1)).is_err()
    );
    assert!(
        toml::from_str::<Manifest>(&RAW.replacen(
            "current_tag = \"jeryu-v5.0.0-split.0\"",
            "predecessor_tag = \"jeryu-v5.0.0-split.0\"",
            1,
        ))
        .is_err()
    );
}

#[test]
fn pending_and_bound_release_identities_are_fail_closed() {
    let mut pending_with_tag = canonical();
    let finder = pending_with_tag
        .repo
        .iter_mut()
        .find(|repo| repo.name == "jeryu-tool-finder")
        .unwrap();
    finder.current_tag = Some("jeryu-tool-finder-v5.1.0-split.0".to_owned());
    assert!(validate(&pending_with_tag).is_err());

    let mut bound_without_tag = canonical();
    bound_without_tag.repo[0].current_tag = None;
    assert!(validate(&bound_without_tag).is_err());

    let mut invented_bound = canonical();
    invented_bound.repo[0].current_tag = Some("jeryu-v5.0.0-split.9".to_owned());
    assert!(validate(&invented_bound).is_err());

    assert!(
        toml::from_str::<Manifest>(&RAW.replacen(
            "identity_status = \"bound\"",
            "identity_status = \"released\"",
            1,
        ))
        .is_err()
    );
}

#[test]
fn rejects_weakened_compliance_contract() {
    let mut hostiles = Vec::new();
    for (from, to) in [
        ("minimum_jankurai_score = 85", "minimum_jankurai_score = 84"),
        ("max_authored_lines = 500", "max_authored_lines = 501"),
        ("target_authored_lines = 300", "target_authored_lines = 301"),
        (
            "max_raw_git_blob_bytes = 1048576",
            "max_raw_git_blob_bytes = 1048577",
        ),
        ("max_caps = 0", "max_caps = 1"),
        ("max_hard_findings = 0", "max_hard_findings = 1"),
        ("max_soft_findings = 0", "max_soft_findings = 1"),
        ("max_disabled_rules = 0", "max_disabled_rules = 1"),
        ("allowed_score_drop = 0", "allowed_score_drop = 1"),
    ] {
        hostiles.push(RAW.replacen(from, to, 1));
    }
    for hostile in hostiles {
        let manifest: Manifest = toml::from_str(&hostile).unwrap();
        assert!(validate(&manifest).is_err());
    }
}

#[test]
fn requires_explicit_lfs_declarations_and_closed_states() {
    let missing_lfs = RAW.replacen("lfs_required = false\n", "", 1);
    assert!(toml::from_str::<Manifest>(&missing_lfs).is_err());
    let missing_requirement = RAW.replacen("max_soft_findings = 0\n", "", 1);
    assert!(toml::from_str::<Manifest>(&missing_requirement).is_err());
    assert!(
        toml::from_str::<Manifest>(&RAW.replace(
            "compliance_state = \"inventory\"",
            "compliance_state = \"disabled\"",
        ))
        .is_err()
    );

    let mut changed = canonical();
    changed.repo[0].lfs_required = true;
    assert!(validate(&changed).is_err());
}

#[test]
fn compliance_state_is_one_way() {
    let enforced = RAW.replace(
        "compliance_state = \"inventory\"",
        "compliance_state = \"enforced\"",
    );
    validate_family_manifest_transition(RAW, &enforced).unwrap();
    validate_family_manifest_transition(&enforced, &enforced).unwrap();
    assert!(validate_family_manifest_transition(&enforced, RAW).is_err());
}
