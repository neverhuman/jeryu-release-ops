#![forbid(unsafe_code)]
#![doc = "Phase 11 domain types and the layers built on them: audit, operations, compliance export, lifecycle, tenant guard, replay verification, and the orchestration kernel."]

pub mod audit;
pub mod compliance_export;
mod ids;
mod json;
pub mod kernel;
pub mod lifecycle;
pub mod ops;
pub mod replay_verifier;
mod report;
mod severity;
pub mod tenant;
mod validation;

pub use ids::{Digest, ExportFormat, TenantId, UpgradeRing, Version};
pub use json::{json_array, now_unix_seconds, quote};
pub use report::{Finding, PolicyDecision, Report};
pub use severity::{HealthState, Severity};
pub use validation::{ValidationError, validate_slug};

/// Phase 11 release marker.
pub const PHASE: u8 = 11;
/// Product name used in receipts.
pub const PRODUCT: &str = "Jeryu";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_parse_rejects_bad_input() {
        assert_eq!(
            Version::parse("0.11.0")
                .unwrap_or_else(|_| Version::new(0, 0, 0))
                .to_string(),
            "0.11.0"
        );
        assert!(Version::parse("0.11").is_err());
        assert!(Version::parse("0.11.x").is_err());
    }

    #[test]
    fn report_fails_closed_for_blocked() {
        let mut report = Report::new("readiness");
        report.push(Finding::new(
            "tenant.broad",
            Severity::Blocked,
            "broad permission denied",
        ));
        assert!(report.fails_closed());
        assert!(report.to_json().contains("tenant.broad"));
    }

    #[test]
    fn quote_escapes_control_characters() {
        assert_eq!(quote("a\"b\\c\n"), "\"a\\\"b\\\\c\\n\"");
    }

    #[test]
    fn one_readiness_run_carries_a_receipt_from_every_module() {
        let tenant = TenantId::new("tenant-a").unwrap_or_else(|_| panic!("valid"));
        let readiness = kernel::readiness(tenant, "ops-bot");
        for kind in [
            audit::AuditKind::TenantDecision,
            audit::AuditKind::Operation,
            audit::AuditKind::UpgradePlan,
            audit::AuditKind::RollbackPlan,
            audit::AuditKind::ReplayVerification,
            audit::AuditKind::ComplianceExport,
            audit::AuditKind::Readiness,
        ] {
            assert!(
                readiness.audit_receipts_json.contains(kind.as_str()),
                "no {} receipt in the shared ledger",
                kind.as_str()
            );
        }
    }
}
