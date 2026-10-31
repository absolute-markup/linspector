//! Existing Fedora/RPM behaviour behind the shared provider contract.
use super::{rpm_query, *};
use crate::{
    system::{Application, Entry, Service},
    Package,
};
use std::collections::BTreeMap;
#[derive(Clone, Debug)]
pub(super) struct RawEntry {
    pub package: Package,
    pub category: String,
    pub applications: Vec<Application>,
    pub services: Vec<Service>,
}
pub(super) struct RpmProvider {
    platform: Platform,
}
impl RpmProvider {
    pub(super) fn new(platform: Platform) -> Self {
        Self { platform }
    }
    fn checked(&self, id: &PackageId) -> Result<()> {
        if id.provider != "rpm" || id.scope != "system" {
            return Err(ProviderError::new(
                ErrorKind::WrongProvider,
                "This RPM provider cannot handle another source or installation scope",
            ));
        }
        if id.native.is_empty() {
            return Err(ProviderError::new(
                ErrorKind::NotFound,
                "Missing native package identifier",
            ));
        }
        if !self.platform.capabilities.package_inventory {
            return Err(ProviderError::new(
                ErrorKind::Unavailable,
                "RPM inspection is unavailable",
            ));
        }
        Ok(())
    }
    fn raw(&self, id: &PackageId) -> Result<Package> {
        self.checked(id)?;
        rpm_query::installed()
            .map_err(operation)?
            .into_iter()
            .find(|p| p.identity() == id.native)
            .ok_or_else(|| {
                ProviderError::new(
                    ErrorKind::NotFound,
                    "Package changed or is no longer installed; refresh the inventory",
                )
            })
    }
}
fn operation(message: String) -> ProviderError {
    ProviderError::new(ErrorKind::OperationFailed, message)
}
fn known(s: &str) -> Option<String> {
    if s.is_empty() || s == "(none)" {
        None
    } else {
        Some(s.into())
    }
}
pub(super) fn normalize(p: Package) -> PackageRecord {
    let id = PackageId {
        provider: "rpm".into(),
        scope: "system".into(),
        native: p.identity(),
    };
    let key = id.key();
    let native_metadata = BTreeMap::from([
        ("rpm.epoch".into(), p.epoch.clone()),
        ("rpm.version".into(), p.version.clone()),
        ("rpm.release".into(), p.release.clone()),
    ]);
    PackageRecord {
        id,
        key,
        name: p.name,
        title: p.title,
        description: p.description,
        version_display: format!("{}:{}-{}", p.epoch, p.version, p.release),
        arch: p.arch,
        license: known(&p.license),
        vendor: known(&p.vendor),
        url: known(&p.url),
        source_package: known(&p.source_rpm),
        repository: if p.repository.starts_with("Unknown") {
            None
        } else {
            known(&p.repository)
        },
        installed_bytes: p.installed_bytes.parse().ok(),
        install_time: p.install_time.parse().ok(),
        requires: p.requires,
        provides: p.provides,
        provenance: p.provenance,
        native_metadata,
    }
}
impl PackageProvider for RpmProvider {
    fn id(&self) -> &str {
        "rpm"
    }
    fn label(&self) -> &str {
        "RPM"
    }
    fn capabilities(&self) -> ProviderCapabilities {
        let c = &self.platform.capabilities;
        let mut reasons = BTreeMap::new();
        if !c.package_removal {
            reasons.insert("removal".into(), c.removal_reason.clone());
        }
        ProviderCapabilities {
            inventory: c.package_inventory,
            metadata: c.package_inventory,
            dependencies: c.package_inventory,
            reverse_candidates: c.reverse_dependencies,
            removal_preview: c.removal_preview,
            removal_execution: c.package_removal,
            reasons,
        }
    }
    fn inventory(&self) -> Result<Inventory> {
        if !self.capabilities().inventory {
            return Err(ProviderError::new(
                ErrorKind::Unavailable,
                "RPM inventory is unavailable",
            ));
        }
        let (raw, warnings) = super::rpm_catalog::raw_inventory().map_err(operation)?;
        Ok(Inventory {
            entries: raw
                .into_iter()
                .map(|e| Entry {
                    package: normalize(e.package),
                    category: e.category,
                    capabilities: self.capabilities(),
                    applications: e.applications,
                    services: e.services,
                })
                .collect(),
            warnings,
            source: "Live RPM database + package-owned desktop entries".into(),
            provider_label: self.label().into(),
            capabilities: self.capabilities(),
        })
    }
    fn metadata(&self, id: &PackageId) -> Result<PackageRecord> {
        self.raw(id).map(normalize)
    }
    fn dependencies(&self, id: &PackageId, include_reverse: bool) -> Result<DependencyReport> {
        let p = self.raw(id)?;
        if include_reverse && !self.capabilities().reverse_candidates {
            return Err(ProviderError::new(
                ErrorKind::Unsupported,
                "Reverse dependency candidates are unavailable",
            ));
        }
        let reverse = if include_reverse {
            rpm_query::reverse_candidates(&p).map_err(operation)?
        } else {
            vec![]
        };
        Ok(DependencyReport{package:id.clone(),requires:p.requires,provides:p.provides,
            reverse_candidates:reverse.into_iter().map(|native|PackageId{provider:"rpm".into(),scope:"system".into(),native}).collect(),reverse_queried:include_reverse,
            limitations:vec!["RPM capability candidates are not solver-validated dependency edges; file/rich dependencies and version/architecture differences may affect results.".into()]})
    }
    fn prepare_removal(&self, id: &PackageId) -> Result<Box<dyn PreparedRemoval>> {
        self.checked(id)?;
        if !self.capabilities().removal_preview {
            return Err(ProviderError::new(
                ErrorKind::Unsupported,
                self.platform.capabilities.removal_reason.clone(),
            ));
        }
        // Fresh metadata and native management policy are checked before planning.
        let raw = self.raw(id)?;
        if self.platform.family == "zypper-rpm" {
            return super::zypper::preview(&raw);
        }
        let native = super::dnf::preview(&raw).map_err(operation)?;
        let review = RemovalReview {
            executor_label: "DNF5".into(),
            requested: id.clone(),
            changes: native
                .rows
                .iter()
                .map(|r| PlannedChange {
                    native_id: r.nevra.clone(),
                    action: ChangeAction::Remove,
                    reason: r.reason.clone(),
                })
                .collect(),
            notes: vec![
                "All listed packages will be removed; unused dependencies are kept.".into(),
            ],
        };
        Ok(Box::new(RpmRemoval { review, native }))
    }
}
struct RpmRemoval {
    review: RemovalReview,
    native: super::dnf::RemovalPlan,
}
impl PreparedRemoval for RpmRemoval {
    fn review(&self) -> &RemovalReview {
        &self.review
    }
    fn execute(self: Box<Self>) -> Result<String> {
        super::dnf::execute(self.native).map_err(operation)
    }
}
pub(super) fn demo_entries(packages: Vec<Package>) -> Vec<Entry> {
    packages
        .into_iter()
        .map(|p| Entry {
            category: super::rpm_catalog::category(&p, &[]),
            capabilities: ProviderCapabilities::default(),
            package: normalize(p),
            applications: vec![],
            services: vec![],
        })
        .collect()
}
pub(super) fn protected_name(name: &str) -> bool {
    super::dnf::protected(name)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Package {
        serde_json::from_str::<Vec<Package>>(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/sample-inventory.json"
        )))
        .unwrap()
        .remove(0)
    }
    #[test]
    fn normalization_keeps_native_versions_identity_and_unknowns() {
        let mut p = fixture();
        p.epoch = "2".into();
        p.version = "1.0~rc1+vendor".into();
        p.release = "3.fc44".into();
        p.installed_bytes = "".into();
        p.repository = "Unknown — unavailable".into();
        let id = p.identity();
        let requires = p.requires.clone();
        let n = normalize(p);
        assert_eq!(n.id.native, id);
        assert_eq!(n.version_display, "2:1.0~rc1+vendor-3.fc44");
        assert_eq!(n.native_metadata["rpm.epoch"], "2");
        assert_eq!(n.requires, requires);
        assert_eq!(n.installed_bytes, None);
        assert_eq!(n.repository, None);
        assert_eq!(n.key, n.id.key());
    }
    #[test]
    fn foreign_id_and_readonly_removal_rejected_before_query() {
        let platform = crate::platform::identify(&crate::platform::Facts {
            os_release: "ID=opensuse-tumbleweed\n".into(),
            tools: ["rpm".into()].into_iter().collect(),
            ..Default::default()
        });
        let provider = RpmProvider::new(platform);
        let foreign = PackageId {
            provider: "other".into(),
            scope: "system".into(),
            native: "x".into(),
        };
        assert_eq!(
            provider.metadata(&foreign).unwrap_err().kind,
            ErrorKind::WrongProvider
        );
        let id = PackageId {
            provider: "rpm".into(),
            ..foreign
        };
        assert!(matches!(
            provider.prepare_removal(&id),
            Err(ProviderError {
                kind: ErrorKind::Unsupported,
                ..
            })
        ));
    }
}
