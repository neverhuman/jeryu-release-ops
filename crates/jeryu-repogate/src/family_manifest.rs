//! Typed validation for the independent Jeryu split-family authority.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use crate::GateOutcome;

/// Canonical authority-manifest path relative to the release control plane.
pub const FAMILY_MANIFEST_RELATIVE_PATH: &str = "repos.manifest.toml";
const SPLIT_ROOT: &str = "/home/ubuntu/jain-split/jeryu-split";
const AUTHORITY_PATH: &str =
    "/home/ubuntu/jain-split/jeryu-split/jeryu-release-ops/repos.manifest.toml";
const REDLINE_ROOT: &str = "/home/ubuntu/jain-split/jain-redline";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema_version: String,
    repo_family: String,
    release_identity: String,
    release_lineage: String,
    status: String,
    formal_ga: bool,
    split_root: String,
    manifest_authority: String,
    required_repos: Vec<String>,
    retired_histories: Vec<String>,
    control_plane: Repository,
    nested_families: NestedFamilies,
    repo: Vec<Repository>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct NestedFamilies {
    redline: RedlineFamily,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RedlineFamily {
    family: String,
    consumer: String,
    source_authority: String,
    container_path: String,
    control_plane: String,
    manifest_path: String,
    dependency_resolution: String,
    required: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Repository {
    name: String,
    path: String,
    jeryu_slug: String,
    remote: String,
    required_check: String,
    default_branch: String,
    identity_status: IdentityStatus,
    current_tag: Option<String>,
    inventory_status: String,
    runtime_authority: String,
    product_name: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
enum IdentityStatus {
    Pending,
    Bound,
}

const CONTROL_PLANE_TAG: &str = "jeryu-release-ops-v5.0.0-split.0";
const PRODUCT_AUTHORITIES: [(&str, &str, Option<&str>, Option<&str>); 10] = [
    ("jeryu", "library", None, Some("jeryu-v5.0.0-split.0")),
    (
        "jeryu-cache",
        "library",
        None,
        Some("jeryu-cache-v5.0.0-split.0"),
    ),
    (
        "jeryu-ci-runner",
        "shadow-only",
        None,
        Some("jeryu-ci-runner-v5.0.0-split.0"),
    ),
    (
        "jeryu-core",
        "library",
        None,
        Some("jeryu-core-v5.0.0-split.1"),
    ),
    (
        "jeryu-deploy",
        "shadow-only",
        None,
        Some("jeryu-deploy-v5.0.0-split.0"),
    ),
    (
        "jeryu-intelligence",
        "library",
        None,
        Some("jeryu-intelligence-v5.0.0-split.0"),
    ),
    (
        "jeryu-jira",
        "library",
        Some("Work"),
        Some("jeryu-jira-v5.0.0-split.0"),
    ),
    (
        "jeryu-tool",
        "library",
        None,
        Some("jeryu-tool-v5.1.0-split.0"),
    ),
    ("jeryu-tool-finder", "library", None, None),
    (
        "jeryu-web",
        "retirement-pending",
        None,
        Some("jeryu-web-v5.0.0-split.0"),
    ),
];

/// Validate the authority manifest rooted at `root`.
pub fn run_family_manifest(root: &Path) -> Result<GateOutcome> {
    let path = root.join(FAMILY_MANIFEST_RELATIVE_PATH);
    let raw = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let manifest: Manifest =
        toml::from_str(&raw).with_context(|| format!("parse {}", path.display()))?;
    validate(&manifest)?;
    Ok(GateOutcome {
        stdout: vec![format!(
            "Jeryu family authority ok: {} active checkouts, {} retired histories",
            manifest.repo.len() + 1,
            manifest.retired_histories.len()
        )],
        exit_code: 0,
    })
}

fn validate(manifest: &Manifest) -> Result<()> {
    if manifest.schema_version != "1"
        || manifest.repo_family != "jeryu-split"
        || manifest.release_identity != "jeryu-split"
        || manifest.release_lineage != "v5"
    {
        bail!("Jeryu family identity must remain independent jeryu-split with v5 lineage");
    }
    if manifest.status != "candidate" || manifest.formal_ga {
        bail!("Jeryu family authority must remain fail-closed candidate metadata");
    }
    if manifest.split_root != SPLIT_ROOT || manifest.manifest_authority != AUTHORITY_PATH {
        bail!("Jeryu split root or authority path is not canonical");
    }
    if manifest.control_plane.name != "jeryu-release-ops"
        || manifest.control_plane.runtime_authority != "control-plane"
        || manifest.control_plane.product_name.is_some()
    {
        bail!("Jeryu authority must use jeryu-release-ops as its control plane");
    }

    let mut names = BTreeSet::new();
    let mut paths = BTreeSet::new();
    let mut slugs = BTreeSet::new();
    let mut remotes = BTreeSet::new();
    for repo in &manifest.repo {
        let (_, expected_runtime, expected_product_name, expected_tag) = PRODUCT_AUTHORITIES
            .iter()
            .find(|(name, _, _, _)| *name == repo.name)
            .ok_or_else(|| anyhow::anyhow!("{} is not a governed Jeryu repository", repo.name))?;
        if repo.runtime_authority != *expected_runtime
            || repo.product_name.as_deref() != *expected_product_name
        {
            bail!("{} has the wrong runtime or product identity", repo.name);
        }
        validate_repository(repo, *expected_tag)?;
    }
    validate_repository(&manifest.control_plane, Some(CONTROL_PLANE_TAG))?;
    for repo in manifest
        .repo
        .iter()
        .chain(std::iter::once(&manifest.control_plane))
    {
        if !names.insert(repo.name.clone())
            || !paths.insert(repo.path.clone())
            || !slugs.insert(repo.jeryu_slug.clone())
            || !remotes.insert(repo.remote.clone())
        {
            bail!(
                "Jeryu authority contains a duplicate repository identity, path, slug, or origin"
            );
        }
    }
    let retired = manifest
        .retired_histories
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    if retired.len() != manifest.retired_histories.len()
        || (!retired.is_empty() && retired != BTreeSet::from(["jeryu-web".to_owned()]))
    {
        bail!("retired_histories may contain only the completed jeryu-web retirement");
    }

    let mut expected_names = PRODUCT_AUTHORITIES
        .iter()
        .map(|(name, _, _, _)| (*name).to_owned())
        .collect::<BTreeSet<_>>();
    if retired.contains("jeryu-web") {
        expected_names.remove("jeryu-web");
    }
    let product_names = manifest
        .repo
        .iter()
        .map(|repo| repo.name.clone())
        .collect::<BTreeSet<_>>();
    if product_names.len() != manifest.repo.len() || product_names != expected_names {
        bail!("Jeryu authority product rows do not match the governed family inventory");
    }

    let required = manifest
        .required_repos
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    if required.len() != manifest.required_repos.len() || required != names {
        bail!("required_repos must exactly match active product and control-plane identities");
    }
    if retired.iter().any(|retired| names.contains(retired)) {
        bail!("an active repository cannot also be a retired history");
    }

    let redline = &manifest.nested_families.redline;
    if redline.family != "redline-split"
        || redline.consumer != "jeryu-split"
        || redline.source_authority != "jain-redline"
        || redline.container_path != REDLINE_ROOT
        || redline.control_plane != format!("{REDLINE_ROOT}/redline-split-ops")
        || redline.manifest_path != format!("{REDLINE_ROOT}/redline-split-ops/repos.manifest.toml")
        || redline.dependency_resolution != "immutable-local-forge-tags"
        || !redline.required
    {
        bail!("Redline dependency must resolve only through canonical jain-redline authority");
    }
    Ok(())
}

fn validate_repository(repo: &Repository, expected_tag: Option<&str>) -> Result<()> {
    let expected_path = PathBuf::from(SPLIT_ROOT).join(&repo.name);
    let expected_slug = format!("jeryu/{}", repo.name);
    let expected_remote = format!("http://127.0.0.1:8787/git/{expected_slug}.git");
    let expected_check = format!("{}/required", repo.name);
    if repo.path != expected_path.to_string_lossy()
        || repo.jeryu_slug != expected_slug
        || repo.remote != expected_remote
        || repo.required_check != expected_check
        || repo.default_branch != "main"
        || repo.inventory_status != "active"
    {
        bail!(
            "{} has noncanonical path, forge identity, check, branch, or inventory status",
            repo.name
        );
    }
    let current_tag = match (
        repo.identity_status,
        repo.current_tag.as_deref(),
        expected_tag,
    ) {
        (IdentityStatus::Pending, None, None) => return Ok(()),
        (IdentityStatus::Bound, Some(actual), Some(expected)) if actual == expected => actual,
        (IdentityStatus::Pending, Some(_), _) => {
            bail!("{} is pending but carries a bound release tag", repo.name)
        }
        (IdentityStatus::Bound, None, _) => {
            bail!("{} is bound without a release tag", repo.name)
        }
        (_, _, _) => bail!("{} release identity does not match authority", repo.name),
    };
    let tag_prefix = format!("{}-v5.", repo.name);
    let Some(version_and_revision) = current_tag.strip_prefix(&tag_prefix) else {
        bail!(
            "{} must retain its immutable v5 split-tag lineage",
            repo.name
        );
    };
    let Some((version, revision)) = version_and_revision.split_once("-split.") else {
        bail!(
            "{} must retain its immutable v5 split-tag lineage",
            repo.name
        );
    };
    if version.split('.').count() != 2
        || version
            .split('.')
            .any(|part| part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()))
        || revision.is_empty()
        || !revision.bytes().all(|byte| byte.is_ascii_digit())
    {
        bail!(
            "{} must retain its immutable v5 split-tag lineage",
            repo.name
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn canonical() -> Manifest {
        toml::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../repos.manifest.toml"
        )))
        .expect("canonical manifest parses")
    }

    #[test]
    fn canonical_authority_passes() {
        validate(&canonical()).unwrap();
    }

    #[test]
    fn rejects_old_root_duplicate_redline_and_slug_alias() {
        let raw = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../repos.manifest.toml"
        ));
        for hostile in [
            raw.replace(SPLIT_ROOT, "/home/ubuntu/jeryu-split"),
            raw.replace(
                REDLINE_ROOT,
                "/home/ubuntu/jain-split/jeryu-split/jeryu-redline",
            ),
            raw.replacen("jeryu/jeryu-cache", "veox/jeryu-cache", 1),
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
        let raw = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../repos.manifest.toml"
        ));
        assert!(toml::from_str::<Manifest>(&format!("unknown = true\n{raw}")).is_err());
        assert!(toml::from_str::<Manifest>(&format!("{raw}\nunknown = true\n")).is_err());

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

        let raw = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../repos.manifest.toml"
        ));
        assert!(
            toml::from_str::<Manifest>(&raw.replacen(
                "identity_status = \"bound\"",
                "identity_status = \"released\"",
                1,
            ))
            .is_err()
        );
    }
}
