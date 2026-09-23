//! Breaking public-API detection — the major-bump signal.
//!
//! Two layered signals are combined; either firing forces a MAJOR bump:
//!
//! * SIGNAL A (zero deps, always available): the conventional-commit `!` /
//!   `BREAKING CHANGE` marker, already carried on
//!   [`crate::commits::ConventionalCommit::breaking`] and handled in
//!   [`crate::classify`].
//! * SIGNAL B (real public-API diff): this module. It REUSES
//!   `jeryu_rustjet`'s [`PublicApiDetector`]/[`PublicApiChange`] over the
//!   workspace manifest to find changed public surfaces, then SHELLS OUT to
//!   `cargo semver-checks check-release` — the exact tool the rustjet
//!   classifier's `semver` lane already names (`classifier/derive.rs`). The
//!   engine never reimplements API diffing.
//!
//! SIGNAL B is a gate, so a missing `cargo-semver-checks` is an ERROR, not a
//! quiet downgrade to SIGNAL A: a host without the tool would otherwise ship a
//! breaking change as a patch and nothing would say so. An operator who
//! knowingly wants the SIGNAL-A-only decision sets
//! [`ALLOW_MISSING_ENV`]`=1`, and the report says which choice was made.

use std::ffi::{OsStr, OsString};
use std::path::Path;
use std::process::Command;

use anyhow::{Result, bail};
use jeryu_rustjet::manifest::WorkspaceManifest;
use jeryu_rustjet::public_api::{PublicApiChange, PublicApiDetector};

/// Environment variable that turns a missing `cargo-semver-checks` from a hard
/// error into an explicit, recorded SIGNAL-A-only decision.
pub const ALLOW_MISSING_ENV: &str = "JERYU_WSVERSION_ALLOW_MISSING_SEMVER_CHECKS";

/// Outcome of the public-API breaking-change probe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiSurfaceReport {
    /// `true` if a breaking public-API change was detected (forces MAJOR).
    pub breaking: bool,
    /// Human-readable explanation recorded in the gate outcome.
    pub detail: String,
    /// Candidate public-surface changes found by the rustjet detector.
    pub candidates: Vec<PublicApiChange>,
    /// Whether `cargo-semver-checks` was available and run.
    pub tool_ran: bool,
}

/// The `cargo` front-end used to reach `cargo semver-checks`. Production always
/// uses [`SemverChecks::cargo`]; tests point `program` at a stand-in binary so
/// the real invocation path — argument list, exit-status reading, spawn failure
/// — is exercised without installing the subcommand.
#[derive(Debug, Clone)]
pub struct SemverChecks {
    program: OsString,
}

impl SemverChecks {
    /// The production front-end: whatever `cargo` is on `PATH`.
    #[must_use]
    pub fn cargo() -> Self {
        Self {
            program: OsString::from("cargo"),
        }
    }

    /// A front-end at an explicit path, for tests and for hosts that pin cargo.
    #[must_use]
    pub fn with_program(program: impl AsRef<OsStr>) -> Self {
        Self {
            program: program.as_ref().to_os_string(),
        }
    }

    /// Probe for the subcommand by asking cargo for its version. A clean exit
    /// means `cargo-semver-checks` is installed.
    fn available(&self, root: &Path) -> bool {
        Command::new(&self.program)
            .args(["semver-checks", "--version"])
            .current_dir(root)
            .output()
            .map(|out| out.status.success())
            .unwrap_or(false)
    }
}

/// Detect candidate public-API surface changes by replaying `changed_files`
/// through `jeryu_rustjet`'s [`PublicApiDetector`] against the workspace
/// manifest. This is the zero-dependency surface signal; it does not by itself
/// decide breakage (a changed public file may be additive), but it scopes which
/// packages are worth running `cargo semver-checks` against.
///
/// # Errors
/// Returns an error if the workspace manifest cannot be loaded.
pub fn detect_public_api_candidates(
    root: &Path,
    changed_files: &[String],
) -> Result<Vec<PublicApiChange>> {
    let manifest = WorkspaceManifest::load(root)
        .map_err(|e| anyhow::anyhow!("load workspace manifest: {e}"))?;
    let detector = PublicApiDetector::new();
    let mut changes = Vec::new();
    for file in changed_files {
        for package in manifest.packages.values() {
            if let Some(inside) = package.path_inside_package(file)
                && let Some(change) = detector.detect(package, inside)
            {
                changes.push(change);
            }
        }
    }
    changes.sort_by(|a, b| {
        (a.package.clone(), a.path.clone()).cmp(&(b.package.clone(), b.path.clone()))
    });
    changes.dedup();
    Ok(changes)
}

/// Probe whether the change set constitutes a breaking public-API change, using
/// the `cargo` on `PATH`.
///
/// # Errors
/// Returns an error if the manifest cannot be loaded, if `cargo-semver-checks`
/// is not installed (unless [`ALLOW_MISSING_ENV`] is set to `1`), or if the tool
/// cannot be spawned.
pub fn api_breaking(
    root: &Path,
    changed_files: &[String],
    changed_pkgs: &[String],
) -> Result<ApiSurfaceReport> {
    api_breaking_with(
        root,
        changed_files,
        changed_pkgs,
        &SemverChecks::cargo(),
        std::env::var(ALLOW_MISSING_ENV).ok().as_deref() == Some("1"),
    )
}

/// The probe with its two host dependencies passed in: the cargo front-end and
/// whether a missing tool is tolerated (see [`ALLOW_MISSING_ENV`]).
///
/// `changed_pkgs` is the blast-radius package set (from
/// `jeryu_repogate::build_affected_plan(...).packages`). An empty set means
/// there is nothing to check and the probe is a no-op.
///
/// # Errors
/// Returns an error if the manifest cannot be loaded, if the tool is absent and
/// `allow_missing` is false, or if the tool cannot be spawned.
pub fn api_breaking_with(
    root: &Path,
    changed_files: &[String],
    changed_pkgs: &[String],
    tool: &SemverChecks,
    allow_missing: bool,
) -> Result<ApiSurfaceReport> {
    let candidates = detect_public_api_candidates(root, changed_files)?;

    if changed_pkgs.is_empty() {
        return Ok(ApiSurfaceReport {
            breaking: false,
            detail: "no changed packages in blast radius; no public-API check needed".into(),
            candidates,
            tool_ran: false,
        });
    }

    if !tool.available(root) {
        if !allow_missing {
            bail!(
                "cargo-semver-checks is not installed, so the public-API gate cannot run for \
                 {} changed package(s); install it (`cargo install cargo-semver-checks`) or set \
                 {ALLOW_MISSING_ENV}=1 to accept a decision made from the commit \
                 `!`/footer signal alone",
                changed_pkgs.len()
            );
        }
        return Ok(ApiSurfaceReport {
            breaking: false,
            detail: format!(
                "cargo-semver-checks not installed and {ALLOW_MISSING_ENV}=1; relied on commit \
                 `!`/footer signal only"
            ),
            candidates,
            tool_ran: false,
        });
    }

    // Mirror the rustjet classifier's `semver` lane: `cargo semver-checks
    // check-release` scoped to the changed packages.
    let mut args = vec!["semver-checks".to_string(), "check-release".to_string()];
    for pkg in changed_pkgs {
        args.push("-p".to_string());
        args.push(pkg.clone());
    }
    let status = Command::new(&tool.program)
        .args(&args)
        .current_dir(root)
        .status()
        .map_err(|e| anyhow::anyhow!("spawn cargo semver-checks: {e}"))?;
    let breaking = !status.success();
    Ok(ApiSurfaceReport {
        detail: if breaking {
            "cargo semver-checks reported a major-requiring change".into()
        } else {
            "cargo semver-checks found no breaking change".into()
        },
        breaking,
        candidates,
        tool_ran: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::{TempDir, tempdir};

    fn write(root: &Path, rel: &str, body: &str) {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, body).unwrap();
    }

    fn init_workspace(root: &Path) {
        write(
            root,
            "Cargo.toml",
            "[workspace]\nmembers = [\"crates/demo\"]\n",
        );
        write(
            root,
            "crates/demo/Cargo.toml",
            "[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        );
        write(root, "crates/demo/src/lib.rs", "pub fn api() {}\n");
    }

    /// A stand-in `cargo` that records its argv and exits with `check_exit` for
    /// `check-release`. `version_exit` decides whether the subcommand looks
    /// installed, so both the present and absent worlds are testable here.
    fn stub_cargo(dir: &TempDir, version_exit: i32, check_exit: i32) -> (PathBuf, PathBuf) {
        let bin = dir.path().join("cargo-stub.sh");
        let argv_log = dir.path().join("argv.log");
        fs::write(
            &bin,
            format!(
                "#!/usr/bin/env bash\nprintf '%s\\n' \"$*\" >> {log}\n\
                 if [[ \"$2\" == \"--version\" ]]; then exit {version_exit}; fi\n\
                 exit {check_exit}\n",
                log = argv_log.display(),
            ),
        )
        .unwrap();
        let mut perms = fs::metadata(&bin).unwrap().permissions();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            perms.set_mode(0o755);
        }
        fs::set_permissions(&bin, perms).unwrap();
        (bin, argv_log)
    }

    #[test]
    fn detects_public_surface_in_changed_lib() {
        let dir = tempdir().unwrap();
        init_workspace(dir.path());
        let candidates =
            detect_public_api_candidates(dir.path(), &["crates/demo/src/lib.rs".into()]).unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].package, "demo");
    }

    #[test]
    fn non_rust_change_yields_no_candidate() {
        let dir = tempdir().unwrap();
        init_workspace(dir.path());
        let candidates =
            detect_public_api_candidates(dir.path(), &["crates/demo/README.md".into()]).unwrap();
        assert!(candidates.is_empty());
    }

    #[test]
    fn empty_packages_means_not_breaking() {
        let dir = tempdir().unwrap();
        init_workspace(dir.path());
        let report = api_breaking(dir.path(), &[], &[]).unwrap();
        assert!(!report.breaking);
        assert!(!report.tool_ran);
    }

    #[test]
    fn clean_tool_run_is_not_breaking_and_is_scoped_to_changed_packages() {
        let dir = tempdir().unwrap();
        init_workspace(dir.path());
        let (bin, argv_log) = stub_cargo(&dir, 0, 0);
        let report = api_breaking_with(
            dir.path(),
            &["crates/demo/src/lib.rs".into()],
            &["demo".into(), "other".into()],
            &SemverChecks::with_program(&bin),
            false,
        )
        .unwrap();
        assert!(report.tool_ran);
        assert!(!report.breaking);
        assert_eq!(
            report.detail,
            "cargo semver-checks found no breaking change"
        );
        assert_eq!(report.candidates.len(), 1);
        let argv = fs::read_to_string(&argv_log).unwrap();
        assert!(argv.contains("semver-checks --version"), "argv: {argv}");
        assert!(
            argv.contains("semver-checks check-release -p demo -p other"),
            "argv: {argv}"
        );
    }

    #[test]
    fn nonzero_tool_exit_is_breaking() {
        let dir = tempdir().unwrap();
        init_workspace(dir.path());
        let (bin, _) = stub_cargo(&dir, 0, 1);
        let report = api_breaking_with(
            dir.path(),
            &["crates/demo/src/lib.rs".into()],
            &["demo".into()],
            &SemverChecks::with_program(&bin),
            false,
        )
        .unwrap();
        assert!(report.tool_ran);
        assert!(report.breaking);
        assert_eq!(
            report.detail,
            "cargo semver-checks reported a major-requiring change"
        );
    }

    #[test]
    fn absent_tool_is_an_error_by_default() {
        let dir = tempdir().unwrap();
        init_workspace(dir.path());
        let (bin, _) = stub_cargo(&dir, 1, 0);
        let err = api_breaking_with(
            dir.path(),
            &["crates/demo/src/lib.rs".into()],
            &["demo".into()],
            &SemverChecks::with_program(&bin),
            false,
        )
        .expect_err("a missing public-API gate must not pass silently");
        let msg = err.to_string();
        assert!(
            msg.contains("cargo-semver-checks is not installed"),
            "{msg}"
        );
        assert!(msg.contains(ALLOW_MISSING_ENV), "{msg}");
    }

    #[test]
    fn absent_tool_is_tolerated_only_when_explicitly_allowed() {
        let dir = tempdir().unwrap();
        init_workspace(dir.path());
        let (bin, _) = stub_cargo(&dir, 1, 0);
        let report = api_breaking_with(
            dir.path(),
            &["crates/demo/src/lib.rs".into()],
            &["demo".into()],
            &SemverChecks::with_program(&bin),
            true,
        )
        .unwrap();
        assert!(!report.tool_ran);
        assert!(!report.breaking);
        assert!(
            report.detail.contains(ALLOW_MISSING_ENV),
            "{}",
            report.detail
        );
    }

    #[test]
    fn missing_cargo_front_end_is_an_error() {
        let dir = tempdir().unwrap();
        init_workspace(dir.path());
        let absent = dir.path().join("no-such-cargo");
        let err = api_breaking_with(
            dir.path(),
            &["crates/demo/src/lib.rs".into()],
            &["demo".into()],
            &SemverChecks::with_program(&absent),
            false,
        )
        .expect_err("an unusable cargo front-end must be loud");
        assert!(
            err.to_string()
                .contains("cargo-semver-checks is not installed"),
            "{err}"
        );
    }
}
