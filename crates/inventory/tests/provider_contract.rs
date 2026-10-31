//! Contract checks use an in-memory provider, never a second distribution backend.
use linspector_inventory::{
    model::*,
    platform::{self, Facts},
    providers::*,
    system::Entry,
};
use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};
fn record() -> PackageRecord {
    let id = PackageId {
        provider: "fixture".into(),
        scope: "user:1000".into(),
        native: "opaque::name/branch:arch".into(),
    };
    PackageRecord {
        key: id.key(),
        id,
        name: "example".into(),
        title: "Example package".into(),
        description: "Contract fixture".into(),
        version_display: "1:2.0~rc1+vendor-3".into(),
        arch: "test-arch".into(),
        license: None,
        vendor: None,
        url: None,
        source_package: None,
        repository: None,
        installed_bytes: None,
        install_time: None,
        requires: vec!["dependency >= opaque-version".into()],
        provides: vec!["a-native-capability".into()],
        provenance: "In-memory contract fixture".into(),
        native_metadata: BTreeMap::new(),
    }
}
struct MemoryProvider;
impl PackageProvider for MemoryProvider {
    fn id(&self) -> &str {
        "fixture"
    }
    fn label(&self) -> &str {
        "In-memory fixture"
    }
    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            inventory: true,
            metadata: true,
            dependencies: true,
            ..Default::default()
        }
    }
    fn inventory(&self) -> Result<Inventory> {
        Ok(Inventory {
            entries: vec![Entry {
                package: record(),
                category: "Packages".into(),
                capabilities: self.capabilities(),
                applications: vec![],
                services: vec![],
            }],
            warnings: vec![],
            source: "Test memory only".into(),
            provider_label: self.label().into(),
            capabilities: self.capabilities(),
        })
    }
    fn metadata(&self, id: &PackageId) -> Result<PackageRecord> {
        let p = record();
        if id.provider != self.id() {
            return Err(ProviderError::new(ErrorKind::WrongProvider, "Wrong source"));
        }
        if p.id != *id {
            return Err(ProviderError::new(
                ErrorKind::NotFound,
                "Unknown native identity or scope",
            ));
        }
        Ok(p)
    }
    fn dependencies(&self, id: &PackageId, include_reverse: bool) -> Result<DependencyReport> {
        let p = self.metadata(id)?;
        if include_reverse {
            return Err(ProviderError::new(
                ErrorKind::Unsupported,
                "Fixture has no reverse lookup",
            ));
        }
        Ok(DependencyReport {
            package: id.clone(),
            requires: p.requires,
            provides: p.provides,
            reverse_candidates: vec![],
            reverse_queried: false,
            limitations: vec![],
        })
    }
}
#[test]
fn object_safe_contract_handles_non_rpm_identity_and_optional_metadata() {
    let provider: Box<dyn PackageProvider> = Box::new(MemoryProvider);
    let inventory = provider.inventory().unwrap();
    let p = &inventory.entries[0].package;
    assert_eq!(p.version_display, "1:2.0~rc1+vendor-3");
    assert!(p.installed_bytes.is_none());
    assert!(p.repository.is_none());
    assert_eq!(provider.metadata(&p.id).unwrap().key, p.key);
    assert_eq!(
        provider.dependencies(&p.id, false).unwrap().requires,
        p.requires
    );
    let json = serde_json::to_string(&inventory.entries).unwrap();
    let roundtrip: Vec<Entry> = serde_json::from_str(&json).unwrap();
    assert_eq!(roundtrip[0].package.id, p.id);
    assert!(!json.contains("epoch"));
    assert!(!json.contains("source_rpm"));
}
#[test]
fn readonly_provider_has_no_accidental_mutation_default() {
    let provider: Box<dyn PackageProvider> = Box::new(MemoryProvider);
    assert!(!provider.capabilities().removal_execution);
    assert!(matches!(
        provider.prepare_removal(&record().id),
        Err(ProviderError {
            kind: ErrorKind::Unsupported,
            ..
        })
    ));
    assert_eq!(
        provider.dependencies(&record().id, true).unwrap_err().kind,
        ErrorKind::Unsupported
    );
}
#[test]
fn provider_and_scope_mismatch_do_not_resolve_by_display_name() {
    let mut id = record().id;
    id.provider = "different".into();
    assert_eq!(
        MemoryProvider.metadata(&id).unwrap_err().kind,
        ErrorKind::WrongProvider
    );
    id = record().id;
    id.scope = "system".into();
    assert_eq!(
        MemoryProvider.metadata(&id).unwrap_err().kind,
        ErrorKind::NotFound
    );
}
struct TestPlan {
    review: RemovalReview,
    executions: Arc<AtomicUsize>,
    drops: Arc<AtomicUsize>,
}
impl Drop for TestPlan {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}
impl PreparedRemoval for TestPlan {
    fn review(&self) -> &RemovalReview {
        &self.review
    }
    fn execute(self: Box<Self>) -> Result<String> {
        self.executions.fetch_add(1, Ordering::SeqCst);
        Ok("Test action only".into())
    }
}
#[test]
fn reviewing_and_discarding_a_plan_never_executes_it() {
    let executions = Arc::new(AtomicUsize::new(0));
    let drops = Arc::new(AtomicUsize::new(0));
    let new_plan = || -> Box<dyn PreparedRemoval> {
        Box::new(TestPlan {
            review: RemovalReview {
                executor_label: "Test executor".into(),
                requested: record().id,
                changes: vec![PlannedChange {
                    native_id: "opaque-native-id".into(),
                    action: ChangeAction::Remove,
                    reason: "Test".into(),
                }],
                notes: vec![],
            },
            executions: executions.clone(),
            drops: drops.clone(),
        })
    };
    let plan = new_plan();
    assert_eq!(plan.review().changes.len(), 1);
    drop(plan);
    assert_eq!(executions.load(Ordering::SeqCst), 0);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    new_plan().execute().unwrap();
    assert_eq!(executions.load(Ordering::SeqCst), 1);
    assert_eq!(drops.load(Ordering::SeqCst), 2);
}
#[test]
fn factory_selects_rpm_inspection_without_giving_zypper_hosts_dnf_actions() {
    let p = platform::identify(&Facts {
        os_release: "ID=opensuse-tumbleweed\n".into(),
        tools: ["rpm".into(), "dnf5".into(), "pkexec".into()]
            .into_iter()
            .collect(),
        ..Default::default()
    });
    let provider = select(&p).unwrap();
    assert_eq!(provider.id(), "rpm");
    assert!(provider.capabilities().inventory);
    assert!(!provider.capabilities().removal_preview);
    for release in [
        "ID=ubuntu\nID_LIKE=debian\n",
        "ID=arch\n",
        "ID=unidentified\n",
    ] {
        let p = platform::identify(&Facts {
            os_release: release.into(),
            tools: ["rpm".into(), "dnf5".into()].into_iter().collect(),
            ..Default::default()
        });
        assert!(matches!(
            select(&p),
            Err(ProviderError {
                kind: ErrorKind::Unavailable,
                ..
            })
        ));
    }
}

#[test]
fn mixed_source_routing_never_sends_flatpak_to_rpm_or_grants_mutations() {
    let platform = platform::identify(&Facts {
        os_release: "ID=fedora\nVERSION_ID=44\n".into(),
        tools: ["rpm", "dnf5", "pkexec", "flatpak"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        ..Default::default()
    });
    let id = PackageId {
        provider: "flatpak".into(),
        scope: "system".into(),
        native: "app/org.example.App/x86_64/stable".into(),
    };
    let provider = for_package(&platform, &id).unwrap();
    assert_eq!(provider.id(), "flatpak");
    assert!(!provider.capabilities().removal_preview);
    assert!(matches!(
        provider.prepare_removal(&id),
        Err(ProviderError {
            kind: ErrorKind::Unsupported,
            ..
        })
    ));
    let mut foreign = id.clone();
    foreign.provider = "dpkg".into();
    assert!(matches!(
        for_package(&platform, &foreign),
        Err(ProviderError {
            kind: ErrorKind::WrongProvider,
            ..
        })
    ));
    let mut sandbox = platform;
    sandbox.capabilities.flatpak_inventory = false;
    assert!(for_package(&sandbox, &id).is_err());
}
