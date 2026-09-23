use super::*;

pub(super) fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(1)
}

pub(super) fn temp_artifact(name: &str, contents: &[u8]) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "jeryu_signrail-{name}-{}-{}",
        std::process::id(),
        now()
    ));
    fs::write(&path, contents).unwrap_or_else(|err| panic!("failed to write temp artifact: {err}"));
    path
}

pub(super) fn temp_store_root(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "jeryu_signrail-store-{name}-{}-{}",
        std::process::id(),
        now()
    ))
}

pub(super) fn identity() -> OidcJobIdentity {
    OidcJobIdentity::new(
        "https://jeryu.example.invalid",
        "jeryu_signrail",
        "repo:acme/jeryu:ref:refs/tags/v1.2.3",
        "acme/jeryu",
        ".jit/release@refs/tags/v1.2.3",
        "job-release-1",
        "runner-release-hermetic-1",
        now() + 3600,
    )
}

pub(super) fn policy() -> ReleasePolicy {
    ReleasePolicy::strict(
        "https://git.example.invalid/acme/jeryu",
        "https://jeryu.example.invalid",
        "jeryu_signrail",
        now(),
    )
}

pub(super) fn unsigned_release() -> Release {
    let path = temp_artifact("release.bin", b"release artifact bytes");
    let artifact = Artifact::from_file("jeryu-linux-x86_64", &path, "application/octet-stream")
        .unwrap_or_else(|err| panic!("artifact failed: {err}"));
    let mut release = Release::new(
        "rel_01JPHASE8",
        "Jeryu 1.2.3",
        "v1.2.3",
        "https://git.example.invalid/acme/jeryu",
        "abc123commit",
        "def456tree",
        "blake3:jeryu_ci_ir",
        "release-hermetic",
        "sha256:runner-rootfs",
        "sha256:toolchain",
        "sha256:cargo-lock",
        identity(),
    );
    release.add_artifact(artifact);
    release.attach_sbom(SbomDocument::from_artifacts(
        "v1.2.3",
        &release.artifacts,
        now(),
    ));
    release.attach_rollback(RollbackMetadata::new(
        "v1.2.2",
        "jit release rollback v1.2.2",
        "sha256:config",
        "no irreversible migration",
        now(),
    ));
    release
}

pub(super) fn signed_release() -> (Release, HmacSha256Signer) {
    let mut release = unsigned_release();
    let signer = HmacSha256Signer::new("phase8-test-key", b"phase8-secret");
    release
        .sign_with(&signer, now())
        .unwrap_or_else(|err| panic!("signing failed: {err}"));
    (release, signer)
}

/// Test seed for the local Ed25519 signing path.
pub(super) const SEED: &str = "1313131313131313131313131313131313131313131313131313131313131313";

/// Run a CLI invocation with a closed environment, flattening the error to its
/// display form so tests can assert on both variant prefix and message.
pub(super) fn run_cli(args: &[&str], env: &[(&str, &str)]) -> std::result::Result<String, String> {
    let env = env
        .iter()
        .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
        .collect::<Vec<_>>();
    jeryu_signrail::cli::run_from_with_env(args.iter().map(|arg| arg.to_string()), |key| {
        env.iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value.clone())
    })
    .map_err(|err| err.to_string())
}

/// A store root plus out dir that a signed release was written into.
pub(super) struct SignedFixture {
    pub(super) root: PathBuf,
    pub(super) out_dir: PathBuf,
}

impl SignedFixture {
    pub(super) fn release_json(&self) -> String {
        let path = self.out_dir.join("release.json");
        fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("read {} failed: {err}", path.display()))
    }

    pub(super) fn write_pubkey(&self, seed: &str) -> PathBuf {
        let signer = jeryu_signrail::Ed25519Signer::from_seed_hex(None, seed)
            .unwrap_or_else(|err| panic!("signer failed: {err}"));
        let path = self.root.join("signrail.pub");
        fs::write(&path, signer.public_key_hex())
            .unwrap_or_else(|err| panic!("write pubkey failed: {err}"));
        path
    }
}

/// Sign a release with the standard flags plus `extra`, and return where it landed.
pub(super) fn sign_fixture(name: &str, extra: &[&str]) -> SignedFixture {
    let root = temp_store_root(name);
    let out_dir = root.join("out");
    let artifact = temp_artifact(&format!("{name}-bundle.tar.gz"), b"fixture bundle bytes");
    let artifact_arg = artifact.display().to_string();
    let root_arg = root.display().to_string();
    let out_dir_arg = out_dir.display().to_string();
    let mut args = vec![
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
        root_arg.as_str(),
        "--out-dir",
        out_dir_arg.as_str(),
        "--created-at-epoch",
        "100",
    ];
    args.extend_from_slice(extra);
    run_cli(&args, &[("JERYU_SIGNRAIL_ED25519_SEED", SEED)])
        .unwrap_or_else(|err| panic!("sign-release failed: {err}"));
    SignedFixture { root, out_dir }
}
