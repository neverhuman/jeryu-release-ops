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
struct NestedFamilies {
    redline: RedlineFamily,
}

#[derive(Debug, Deserialize)]
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
struct Repository {
    name: String,
    path: String,
    jeryu_slug: String,
    remote: String,
    required_check: String,
    default_branch: String,
    current_tag: String,
    inventory_status: String,
    runtime_authority: String,
}

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
    if manifest.repo.len() != 10 {
        bail!("Jeryu authority must declare ten product rows plus its control plane");
    }

    let mut names = BTreeSet::new();
    let mut paths = BTreeSet::new();
    let mut slugs = BTreeSet::new();
    let mut remotes = BTreeSet::new();
    for repo in manifest
        .repo
        .iter()
        .chain(std::iter::once(&manifest.control_plane))
    {
        validate_repository(repo)?;
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
    let required = manifest
        .required_repos
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    if required.len() != manifest.required_repos.len() || required != names {
        bail!("required_repos must exactly match active product and control-plane identities");
    }
    if manifest
        .retired_histories
        .iter()
        .any(|retired| names.contains(retired))
    {
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

fn validate_repository(repo: &Repository) -> Result<()> {
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
    if !matches!(
        repo.runtime_authority.as_str(),
        "library" | "shadow-only" | "retirement-pending" | "control-plane"
    ) {
        bail!("{} has unsupported runtime authority", repo.name);
    }
    let tag_prefix = format!("{}-v5.", repo.name);
    if !repo.current_tag.starts_with(&tag_prefix) || !repo.current_tag.contains("-split.") {
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
}
