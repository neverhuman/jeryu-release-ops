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

use std::path::{Path, PathBuf};

const PACKAGE: &str = "redlinedb";
const REPOSITORY: &str = "http://127.0.0.1:8787/git/jeryu/redline-core.git";
const TAG: &str = "redline-core-v4.1.0-jain.6";
const CONSUMED_CRATES: [&str; 3] = ["jeryu-repogate", "jeryu-bench", "jeryu-obs"];

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("crate must be nested beneath the workspace root")
        .to_owned()
}

/// Reject any manifest that offers `redlinedb` to a consumer: only the
/// development table may name it, and only at the governed immutable source.
fn validate_scope(relative: &str, manifest: &str) -> Result<(), String> {
    let manifest: toml::Value = toml::from_str(manifest)
        .map_err(|error| format!("{relative}: invalid manifest TOML: {error}"))?;
    for table in ["dependencies", "build-dependencies", "target"] {
        if manifest
            .get(table)
            .map(|value| value.to_string().contains(PACKAGE))
            .unwrap_or(false)
        {
            return Err(format!(
                "{relative}: `{table}` names {PACKAGE}, which would make Redline a sixth \
                 hosted source for every consumer of this repository"
            ));
        }
    }

    let Some(dependency) = manifest
        .get("dev-dependencies")
        .and_then(|table| table.get(PACKAGE))
    else {
        return Ok(());
    };
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

#[test]
fn redline_is_reachable_only_from_development_targets() {
    let root = workspace_root();
    let mut seen = false;
    for crate_name in CONSUMED_CRATES {
        let relative = format!("crates/{crate_name}/Cargo.toml");
        let manifest =
            std::fs::read_to_string(root.join(&relative)).expect("read consumed crate manifest");
        seen |= manifest.contains(PACKAGE);
        validate_scope(&relative, &manifest).expect("validate Redline dependency scope");
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
    validate_scope("fixture", &governed).expect("governed fixture must pass");

    let hostile = [
        governed.replace("[dev-dependencies]", "[dependencies]"),
        governed.replace("[dev-dependencies]", "[build-dependencies]"),
        governed.replace("[dev-dependencies]", "[target.'cfg(unix)'.dependencies]"),
        format!(
            "{governed}[dependencies]\n{PACKAGE} = {{ git = \"{REPOSITORY}\", tag = \"{TAG}\" }}\n"
        ),
        governed.replace(REPOSITORY, "https://github.com/neverhuman/redline-core.git"),
        governed.replace(TAG, "redline-core-v4.1.0-jain.7"),
        governed.replace(&format!(", tag = \"{TAG}\""), ""),
        governed.replace(&format!("tag = \"{TAG}\""), "branch = \"main\""),
    ];
    for manifest in hostile {
        assert!(
            validate_scope("fixture", &manifest).is_err(),
            "hostile manifest unexpectedly passed: {manifest}"
        );
    }
}
