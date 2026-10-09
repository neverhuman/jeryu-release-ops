//! The Redline edge must stay a development-only dependency.
//!
//! `redlinedb` is a sixth hosted repository. Consumers of this repository —
//! `jeryu-deploy` builds `jeryu-repogate`, `jeryu-bench` and `jeryu-obs` from a
//! `jeryu-release-ops-v5.0.0-split.N` tag — pin exactly five hosted sources, and
//! Cargo only resolves a Git dependency's development dependencies when that
//! dependency is the workspace being built. Keeping `redlinedb` under
//! `[dev-dependencies]` is therefore what lets the consumer compile this
//! repository at all, and it is the one thing the compatibility lane cannot
//! notice on its own.
//!
//! Every workspace member is checked, not only the three crates the consumer
//! names: those crates' path dependencies are members too, and a normal
//! Redline edge in any of them would reach the consumer just the same.

use std::path::{Path, PathBuf};

const PACKAGE: &str = "redlinedb";
/// The transport `crates/jeryu-obs/Cargo.toml` pins today. It moves with that
/// pin when the family switches to hosted transport for dependency resolution
/// (`dependency_resolution` in `repos.manifest.toml`); until then any other
/// source is drift.
const REPOSITORY: &str = "http://127.0.0.1:8787/git/jeryu/redline-core.git";
const TAG: &str = "redline-core-v4.1.0-jain.6";
const DEPENDENCY_TABLES: [&str; 3] = ["dependencies", "build-dependencies", "dev-dependencies"];
const CONSUMED_CRATES: [&str; 3] = ["jeryu-repogate", "jeryu-bench", "jeryu-obs"];

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("crate must be nested beneath the workspace root")
        .to_owned()
}

/// Reject any manifest that offers `redlinedb` to a consumer: only a
/// development table may name it, only under its own key (a renamed entry,
/// `alias = { package = "redlinedb", .. }`, is refused wherever it appears),
/// and only at the governed immutable source. Returns whether the manifest
/// carries the governed development edge.
fn validate_scope(relative: &str, manifest: &str) -> Result<bool, String> {
    let manifest: toml::Value = toml::from_str(manifest)
        .map_err(|error| format!("{relative}: invalid manifest TOML: {error}"))?;
    let mut tables: Vec<(String, &toml::Value)> = Vec::new();
    for table in DEPENDENCY_TABLES {
        if let Some(value) = manifest.get(table) {
            tables.push((table.to_owned(), value));
        }
    }
    // `[target.<cfg>.*]` repeats the three dependency tables per platform and
    // is held to exactly the same rules.
    if let Some(targets) = manifest.get("target").and_then(toml::Value::as_table) {
        for (cfg, platform) in targets {
            for table in DEPENDENCY_TABLES {
                if let Some(value) = platform.get(table) {
                    tables.push((format!("target.{cfg}.{table}"), value));
                }
            }
        }
    }

    let mut governed = false;
    for (table, value) in tables {
        let Some(entries) = value.as_table() else {
            continue;
        };
        for (key, entry) in entries {
            let package = entry.get("package").and_then(toml::Value::as_str);
            if key != PACKAGE && package != Some(PACKAGE) {
                continue;
            }
            if !table.ends_with("dev-dependencies") {
                return Err(format!(
                    "{relative}: `{table}` names {PACKAGE}, which would make Redline a sixth \
                     hosted source for every consumer of this repository"
                ));
            }
            if key != PACKAGE {
                return Err(format!(
                    "{relative}: `{table}.{key}` renames {PACKAGE}; it must be declared under \
                     its own name so its source and tag are checked"
                ));
            }
            validate_dev_dependency(relative, entry)?;
            governed = true;
        }
    }
    Ok(governed)
}

fn validate_dev_dependency(relative: &str, dependency: &toml::Value) -> Result<(), String> {
    let dependency = dependency
        .as_table()
        .ok_or_else(|| format!("{relative}: {PACKAGE} must be a table dependency"))?;
    let mut keys: Vec<_> = dependency.keys().map(String::as_str).collect();
    keys.sort_unstable();
    if keys != ["git", "tag"] {
        return Err(format!(
            "{relative}: {PACKAGE} must contain exactly git/tag, found {keys:?}"
        ));
    }
    if dependency.get("git").and_then(toml::Value::as_str) != Some(REPOSITORY) {
        return Err(format!(
            "{relative}: {PACKAGE} must use the canonical loopback Redline repository"
        ));
    }
    if dependency.get("tag").and_then(toml::Value::as_str) != Some(TAG) {
        return Err(format!(
            "{relative}: {PACKAGE} must use exact immutable tag {TAG}"
        ));
    }
    Ok(())
}

/// Every `[workspace] members` entry of the root manifest.
fn workspace_members(root: &Path) -> Vec<String> {
    let workspace = std::fs::read_to_string(root.join("Cargo.toml")).expect("read workspace root");
    let workspace: toml::Value = toml::from_str(&workspace).expect("parse workspace root");
    workspace["workspace"]["members"]
        .as_array()
        .expect("workspace members are an explicit list")
        .iter()
        .map(|member| {
            member
                .as_str()
                .expect("workspace member is a path")
                .to_owned()
        })
        .collect()
}

#[test]
fn redline_is_reachable_only_from_development_targets() {
    let root = workspace_root();
    let members = workspace_members(&root);
    for crate_name in CONSUMED_CRATES {
        assert!(
            members.contains(&format!("crates/{crate_name}")),
            "consumed crate {crate_name} is no longer a workspace member"
        );
    }
    let mut seen = false;
    for member in &members {
        assert!(
            !member.contains('*'),
            "glob member {member}: list members explicitly so each is checked"
        );
        let relative = format!("{member}/Cargo.toml");
        let manifest =
            std::fs::read_to_string(root.join(&relative)).expect("read workspace member manifest");
        seen |= validate_scope(&relative, &manifest).expect("validate Redline dependency scope");
    }
    assert!(
        seen,
        "no consumed crate names {PACKAGE}: this contract has lost its subject"
    );

    let workspace = std::fs::read_to_string(root.join("Cargo.toml")).expect("read workspace root");
    assert!(
        !workspace.contains(PACKAGE),
        "a workspace dependency on {PACKAGE} would reach every member's public graph"
    );
}

#[test]
fn hostile_dependency_scopes_fail_closed() {
    let governed =
        format!("[dev-dependencies]\n{PACKAGE} = {{ git = \"{REPOSITORY}\", tag = \"{TAG}\" }}\n");
    assert!(validate_scope("fixture", &governed).expect("governed fixture must pass"));
    // Mentioning the name is not the edge: the subject check reads the table.
    assert!(
        !validate_scope(
            "fixture",
            &format!("# {PACKAGE} was here\n[package]\ndescription = \"{PACKAGE}\"\n")
        )
        .unwrap()
    );

    // An alias keeps the real name in `package` and points the edge anywhere.
    let renamed = |table: &str| {
        format!(
            "[{table}]\nredline = {{ package = \"{PACKAGE}\", \
             git = \"https://attacker.example/redline.git\", branch = \"main\" }}\n"
        )
    };

    let hostile = [
        governed.replace("[dev-dependencies]", "[dependencies]"),
        governed.replace("[dev-dependencies]", "[build-dependencies]"),
        governed.replace("[dev-dependencies]", "[target.'cfg(unix)'.dependencies]"),
        governed.replace(
            "[dev-dependencies]",
            "[target.'cfg(unix)'.build-dependencies]",
        ),
        governed.replace(TAG, "redline-core-v4.1.0-jain.7").replace(
            "[dev-dependencies]",
            "[target.'cfg(unix)'.dev-dependencies]",
        ),
        governed
            .replace(
                &format!("{PACKAGE} = {{"),
                &format!("redline = {{ package = \"{PACKAGE}\","),
            )
            .replace("[dev-dependencies]", "[dependencies]"),
        format!(
            "{governed}[dependencies]\n{PACKAGE} = {{ git = \"{REPOSITORY}\", tag = \"{TAG}\" }}\n"
        ),
        governed.replace(REPOSITORY, "https://github.com/neverhuman/redline-core.git"),
        governed.replace(TAG, "redline-core-v4.1.0-jain.7"),
        governed.replace(&format!(", tag = \"{TAG}\""), ""),
        governed.replace(&format!("tag = \"{TAG}\""), "branch = \"main\""),
        renamed("dev-dependencies"),
        renamed("target.'cfg(unix)'.dev-dependencies"),
        renamed("dependencies"),
        // Even a rename that keeps the governed source and tag is refused.
        governed.replace(
            &format!("{PACKAGE} = {{"),
            &format!("redline = {{ package = \"{PACKAGE}\","),
        ),
    ];
    // A development-only edge for one platform is as harmless as the
    // top-level one, and is held to the same source and tag.
    validate_scope(
        "fixture",
        &governed.replace(
            "[dev-dependencies]",
            "[target.'cfg(unix)'.dev-dependencies]",
        ),
    )
    .expect("a target-specific development edge is still development-only");

    for manifest in hostile {
        assert!(
            validate_scope("fixture", &manifest).is_err(),
            "hostile manifest unexpectedly passed: {manifest}"
        );
    }
}
