//! Tenant quotas, RBAC, isolation checks, and fail-closed policy decisions.

use crate::audit::{AuditKind, AuditLedger, record};
use crate::{Finding, PolicyDecision, Severity, TenantId, quote};

/// Operator role.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Viewer,
    Operator,
    Auditor,
    Admin,
    BreakGlass,
}

impl Role {
    /// Stable string.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Viewer => "viewer",
            Self::Operator => "operator",
            Self::Auditor => "auditor",
            Self::Admin => "admin",
            Self::BreakGlass => "break_glass",
        }
    }
}

/// Tenant-scoped action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TenantAction {
    ReadEvidence,
    ExportCompliance,
    PlanUpgrade,
    ApplyUpgrade,
    PlanRollback,
    UpdateQuota,
    ReadReplay,
}

impl TenantAction {
    /// Stable string.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ReadEvidence => "read_evidence",
            Self::ExportCompliance => "export_compliance",
            Self::PlanUpgrade => "plan_upgrade",
            Self::ApplyUpgrade => "apply_upgrade",
            Self::PlanRollback => "plan_rollback",
            Self::UpdateQuota => "update_quota",
            Self::ReadReplay => "read_replay",
        }
    }
}

/// Quota limits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuotaLimit {
    pub max_repos: u32,
    pub max_runners: u32,
    pub max_storage_gib: u32,
    pub max_audit_exports_per_day: u32,
}

impl Default for QuotaLimit {
    fn default() -> Self {
        Self {
            max_repos: 10_000,
            max_runners: 1_000,
            max_storage_gib: 50_000,
            max_audit_exports_per_day: 100,
        }
    }
}

/// Current quota usage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuotaUsage {
    pub repos: u32,
    pub runners: u32,
    pub storage_gib: u32,
    pub audit_exports_today: u32,
}

impl QuotaUsage {
    /// Empty usage.
    pub fn zero() -> Self {
        Self {
            repos: 0,
            runners: 0,
            storage_gib: 0,
            audit_exports_today: 0,
        }
    }
}

/// Tenant policy input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TenantPolicyInput {
    pub tenant: TenantId,
    /// Tenant owning the resource the action targets. `None` means the actor's own tenant.
    pub resource_tenant: Option<TenantId>,
    pub actor: String,
    pub role: Role,
    pub action: TenantAction,
    pub quota: QuotaLimit,
    pub usage: QuotaUsage,
    pub break_glass_ticket: Option<String>,
}

impl TenantPolicyInput {
    /// JSON representation.
    pub fn to_json(&self) -> String {
        format!(
            "{{\"tenant\":{},\"resource_tenant\":{},\"actor\":{},\"role\":{},\"action\":{},\"repos\":{},\"runners\":{},\"storage_gib\":{},\"audit_exports_today\":{}}}",
            quote(self.tenant.as_str()),
            quote(
                self.resource_tenant
                    .as_ref()
                    .unwrap_or(&self.tenant)
                    .as_str()
            ),
            quote(&self.actor),
            quote(self.role.as_str()),
            quote(self.action.as_str()),
            self.usage.repos,
            self.usage.runners,
            self.usage.storage_gib,
            self.usage.audit_exports_today
        )
    }
}

/// Evaluate tenant access and quota policy. Missing or broad privileges deny by default.
pub fn decide(input: &TenantPolicyInput, ledger: &mut AuditLedger) -> PolicyDecision {
    if input.actor.trim().is_empty() {
        return deny("tenant.actor_missing", "actor is required");
    }
    if let Some(resource_tenant) = &input.resource_tenant
        && resource_tenant != &input.tenant
    {
        return deny(
            "tenant.cross_tenant_denied",
            "actor may not act on another tenant's resource",
        );
    }
    if over_quota(&input.usage, &input.quota) {
        return deny("tenant.quota_exceeded", "tenant is over quota");
    }
    if input.role == Role::BreakGlass
        && input
            .break_glass_ticket
            .as_ref()
            .map(|s| s.trim().is_empty())
            .unwrap_or(true)
    {
        return deny(
            "tenant.break_glass_ticket_missing",
            "break-glass access requires a ticket",
        );
    }
    if !role_allows(input.role, input.action) {
        return deny("tenant.role_denied", "role does not allow requested action");
    }

    let receipt_id = record(
        ledger,
        &input.tenant,
        AuditKind::TenantDecision,
        &input.actor,
        input.action.as_str(),
        vec![
            format!("role={}", input.role.as_str()),
            "decision=allow".to_string(),
        ],
    );
    PolicyDecision::Allow {
        reason: "tenant policy allowed action".to_string(),
        receipt_id,
    }
}

fn deny(code: &str, message: &str) -> PolicyDecision {
    PolicyDecision::Deny {
        reason: message.to_string(),
        finding: Finding::new(code, Severity::Blocked, message),
    }
}

fn over_quota(usage: &QuotaUsage, quota: &QuotaLimit) -> bool {
    usage.repos > quota.max_repos
        || usage.runners > quota.max_runners
        || usage.storage_gib > quota.max_storage_gib
        || usage.audit_exports_today > quota.max_audit_exports_per_day
}

fn role_allows(role: Role, action: TenantAction) -> bool {
    match role {
        Role::Viewer => matches!(
            action,
            TenantAction::ReadEvidence | TenantAction::ReadReplay
        ),
        Role::Auditor => matches!(
            action,
            TenantAction::ReadEvidence | TenantAction::ExportCompliance | TenantAction::ReadReplay
        ),
        Role::Operator => matches!(
            action,
            TenantAction::ReadEvidence
                | TenantAction::PlanUpgrade
                | TenantAction::PlanRollback
                | TenantAction::ReadReplay
        ),
        Role::Admin => !matches!(action, TenantAction::ApplyUpgrade),
        Role::BreakGlass => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewer_cannot_update_quota() {
        let mut ledger = AuditLedger::new();
        let input = TenantPolicyInput {
            tenant: TenantId::new("tenant-a").unwrap_or_else(|_| panic!("valid")),
            resource_tenant: None,
            actor: "alice".to_string(),
            role: Role::Viewer,
            action: TenantAction::UpdateQuota,
            quota: QuotaLimit::default(),
            usage: QuotaUsage::zero(),
            break_glass_ticket: None,
        };
        assert!(!decide(&input, &mut ledger).is_allowed());
    }

    #[test]
    fn auditor_can_export_with_receipt() {
        let mut ledger = AuditLedger::new();
        let input = TenantPolicyInput {
            tenant: TenantId::new("tenant-a").unwrap_or_else(|_| panic!("valid")),
            resource_tenant: None,
            actor: "auditor".to_string(),
            role: Role::Auditor,
            action: TenantAction::ExportCompliance,
            quota: QuotaLimit::default(),
            usage: QuotaUsage::zero(),
            break_glass_ticket: None,
        };
        assert!(decide(&input, &mut ledger).is_allowed());
        assert_eq!(ledger.receipts().len(), 1);
    }

    #[test]
    fn empty_actor_denies_without_receipt() {
        let mut ledger = AuditLedger::new();
        let input = TenantPolicyInput {
            tenant: TenantId::new("tenant-a").unwrap_or_else(|_| panic!("valid")),
            resource_tenant: None,
            actor: "  ".to_string(),
            role: Role::Admin,
            action: TenantAction::UpdateQuota,
            quota: QuotaLimit::default(),
            usage: QuotaUsage::zero(),
            break_glass_ticket: None,
        };
        let decision = decide(&input, &mut ledger);
        assert!(!decision.is_allowed());
        assert!(format!("{decision:?}").contains("tenant.actor_missing"));
        assert!(ledger.receipts().is_empty());
    }

    #[test]
    fn quota_excess_denies_before_role_check() {
        let mut ledger = AuditLedger::new();
        let input = TenantPolicyInput {
            tenant: TenantId::new("tenant-a").unwrap_or_else(|_| panic!("valid")),
            resource_tenant: None,
            actor: "admin".to_string(),
            role: Role::Admin,
            action: TenantAction::UpdateQuota,
            quota: QuotaLimit {
                max_repos: 1,
                max_runners: 1,
                max_storage_gib: 1,
                max_audit_exports_per_day: 1,
            },
            usage: QuotaUsage {
                repos: 2,
                runners: 0,
                storage_gib: 0,
                audit_exports_today: 0,
            },
            break_glass_ticket: None,
        };
        let decision = decide(&input, &mut ledger);
        assert!(!decision.is_allowed());
        assert!(format!("{decision:?}").contains("tenant.quota_exceeded"));
    }

    #[test]
    fn break_glass_requires_ticket_then_allows_sensitive_action() {
        let mut ledger = AuditLedger::new();
        let mut input = TenantPolicyInput {
            tenant: TenantId::new("tenant-a").unwrap_or_else(|_| panic!("valid")),
            resource_tenant: None,
            actor: "incident-commander".to_string(),
            role: Role::BreakGlass,
            action: TenantAction::ApplyUpgrade,
            quota: QuotaLimit::default(),
            usage: QuotaUsage::zero(),
            break_glass_ticket: None,
        };
        let denied = decide(&input, &mut ledger);
        assert!(!denied.is_allowed());
        assert!(ledger.receipts().is_empty());

        input.break_glass_ticket = Some("INC-1234".to_string());
        let allowed = decide(&input, &mut ledger);
        assert!(allowed.is_allowed());
        assert_eq!(ledger.receipts().len(), 1);
    }

    #[test]
    fn admin_cannot_apply_upgrade_without_break_glass() {
        let mut ledger = AuditLedger::new();
        let input = TenantPolicyInput {
            tenant: TenantId::new("tenant-a").unwrap_or_else(|_| panic!("valid")),
            resource_tenant: None,
            actor: "admin".to_string(),
            role: Role::Admin,
            action: TenantAction::ApplyUpgrade,
            quota: QuotaLimit::default(),
            usage: QuotaUsage::zero(),
            break_glass_ticket: None,
        };
        let decision = decide(&input, &mut ledger);
        assert!(!decision.is_allowed());
        assert!(format!("{decision:?}").contains("tenant.role_denied"));
    }

    #[test]
    fn policy_input_json_names_role_action_and_usage() {
        let input = TenantPolicyInput {
            tenant: TenantId::new("tenant-a").unwrap_or_else(|_| panic!("valid")),
            resource_tenant: None,
            actor: "auditor".to_string(),
            role: Role::Auditor,
            action: TenantAction::ReadReplay,
            quota: QuotaLimit::default(),
            usage: QuotaUsage {
                repos: 3,
                runners: 4,
                storage_gib: 5,
                audit_exports_today: 6,
            },
            break_glass_ticket: None,
        };
        let json = input.to_json();
        assert!(json.contains("\"role\":\"auditor\""));
        assert!(json.contains("\"action\":\"read_replay\""));
        assert!(json.contains("\"repos\":3"));
        assert!(json.contains("\"audit_exports_today\":6"));
    }

    fn tenant(id: &str) -> TenantId {
        TenantId::new(id).unwrap_or_else(|_| panic!("valid"))
    }

    fn cross_tenant_input(
        actor_tenant: &str,
        resource_tenant: &str,
        role: Role,
    ) -> TenantPolicyInput {
        TenantPolicyInput {
            tenant: tenant(actor_tenant),
            resource_tenant: Some(tenant(resource_tenant)),
            actor: "alice".to_string(),
            role,
            action: TenantAction::ReadEvidence,
            quota: QuotaLimit::default(),
            usage: QuotaUsage::zero(),
            break_glass_ticket: None,
        }
    }

    #[test]
    fn reading_another_tenants_evidence_is_denied_without_receipt() {
        let mut ledger = AuditLedger::new();
        let decision = decide(
            &cross_tenant_input("tenant-a", "tenant-b", Role::Viewer),
            &mut ledger,
        );
        assert!(!decision.is_allowed());
        assert!(format!("{decision:?}").contains("tenant.cross_tenant_denied"));
        assert!(ledger.receipts().is_empty());
    }

    #[test]
    fn every_role_is_denied_across_tenants() {
        let mut ledger = AuditLedger::new();
        for role in [
            Role::Viewer,
            Role::Operator,
            Role::Auditor,
            Role::Admin,
            Role::BreakGlass,
        ] {
            let mut input = cross_tenant_input("tenant-a", "tenant-b", role);
            input.break_glass_ticket = Some("INC-1234".to_string());
            let decision = decide(&input, &mut ledger);
            assert!(
                !decision.is_allowed(),
                "role {} must not reach another tenant",
                role.as_str()
            );
        }
        assert!(ledger.receipts().is_empty());
    }

    #[test]
    fn every_action_is_denied_across_tenants() {
        let mut ledger = AuditLedger::new();
        for action in [
            TenantAction::ReadEvidence,
            TenantAction::ExportCompliance,
            TenantAction::PlanUpgrade,
            TenantAction::ApplyUpgrade,
            TenantAction::PlanRollback,
            TenantAction::UpdateQuota,
            TenantAction::ReadReplay,
        ] {
            let mut input = cross_tenant_input("tenant-a", "tenant-b", Role::Admin);
            input.action = action;
            assert!(
                !decide(&input, &mut ledger).is_allowed(),
                "action {} must not cross a tenant boundary",
                action.as_str()
            );
        }
        assert!(ledger.receipts().is_empty());
    }

    #[test]
    fn isolation_denies_before_quota_and_role_checks() {
        let mut ledger = AuditLedger::new();
        let mut input = cross_tenant_input("tenant-a", "tenant-b", Role::Viewer);
        input.action = TenantAction::UpdateQuota;
        input.usage = QuotaUsage {
            repos: u32::MAX,
            runners: 0,
            storage_gib: 0,
            audit_exports_today: 0,
        };
        let decision = decide(&input, &mut ledger);
        assert!(format!("{decision:?}").contains("tenant.cross_tenant_denied"));
    }

    #[test]
    fn acting_on_own_tenant_named_explicitly_is_allowed() {
        let mut ledger = AuditLedger::new();
        let decision = decide(
            &cross_tenant_input("tenant-a", "tenant-a", Role::Viewer),
            &mut ledger,
        );
        assert!(decision.is_allowed());
        assert_eq!(ledger.receipts().len(), 1);
        assert_eq!(ledger.receipts()[0].tenant, "tenant-a");
    }

    #[test]
    fn tenant_ids_are_matched_exactly_not_by_prefix() {
        let mut ledger = AuditLedger::new();
        let decision = decide(
            &cross_tenant_input("tenant-a", "tenant-ab", Role::Admin),
            &mut ledger,
        );
        assert!(!decision.is_allowed());
        assert!(format!("{decision:?}").contains("tenant.cross_tenant_denied"));
    }

    #[test]
    fn a_shared_ledger_keeps_each_tenants_receipts_distinct() {
        let mut ledger = AuditLedger::new();
        for id in ["tenant-a", "tenant-b"] {
            let input = TenantPolicyInput {
                tenant: tenant(id),
                resource_tenant: None,
                actor: "auditor".to_string(),
                role: Role::Auditor,
                action: TenantAction::ExportCompliance,
                quota: QuotaLimit::default(),
                usage: QuotaUsage::zero(),
                break_glass_ticket: None,
            };
            assert!(decide(&input, &mut ledger).is_allowed());
        }
        let for_a: Vec<_> = ledger
            .receipts()
            .iter()
            .filter(|receipt| receipt.tenant == "tenant-a")
            .collect();
        assert_eq!(for_a.len(), 1);
        assert_eq!(ledger.receipts().len(), 2);
        assert_ne!(ledger.receipts()[0].id, ledger.receipts()[1].id);
    }

    #[test]
    fn one_tenant_over_quota_does_not_block_another() {
        let mut ledger = AuditLedger::new();
        let tight = QuotaLimit {
            max_repos: 1,
            max_runners: 1,
            max_storage_gib: 1,
            max_audit_exports_per_day: 1,
        };
        let mut over = TenantPolicyInput {
            tenant: tenant("tenant-a"),
            resource_tenant: None,
            actor: "auditor".to_string(),
            role: Role::Auditor,
            action: TenantAction::ExportCompliance,
            quota: tight,
            usage: QuotaUsage {
                repos: 2,
                runners: 0,
                storage_gib: 0,
                audit_exports_today: 0,
            },
            break_glass_ticket: None,
        };
        assert!(!decide(&over, &mut ledger).is_allowed());

        over.tenant = tenant("tenant-b");
        over.quota = QuotaLimit::default();
        over.usage = QuotaUsage::zero();
        assert!(decide(&over, &mut ledger).is_allowed());
        assert_eq!(ledger.receipts().len(), 1);
        assert_eq!(ledger.receipts()[0].tenant, "tenant-b");
    }

    #[test]
    fn policy_input_json_names_the_targeted_tenant() {
        let input = cross_tenant_input("tenant-a", "tenant-b", Role::Viewer);
        let json = input.to_json();
        assert!(json.contains("\"tenant\":\"tenant-a\""));
        assert!(json.contains("\"resource_tenant\":\"tenant-b\""));
    }
}
