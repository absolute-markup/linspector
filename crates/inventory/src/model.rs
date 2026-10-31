//! Provider-neutral records. Native versions/identifiers are opaque outside their provider.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackageId {
    pub provider: String,
    /// System or a provider-defined user/installation scope.
    pub scope: String,
    pub native: String,
}
impl PackageId {
    /// JSON tuple encoding avoids collisions when native identifiers contain delimiters.
    pub fn key(&self) -> String {
        serde_json::to_string(&(&self.provider, &self.scope, &self.native))
            .expect("strings serialize")
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PackageRecord {
    pub id: PackageId,
    pub key: String,
    pub name: String,
    pub title: String,
    pub description: String,
    pub version_display: String,
    pub arch: String,
    pub license: Option<String>,
    pub vendor: Option<String>,
    pub url: Option<String>,
    pub source_package: Option<String>,
    pub repository: Option<String>,
    pub installed_bytes: Option<u64>,
    pub install_time: Option<u64>,
    pub requires: Vec<String>,
    pub provides: Vec<String>,
    pub provenance: String,
    /// Namespaced native details are retained without imposing them on other providers.
    pub native_metadata: BTreeMap<String, String>,
}
impl PackageRecord {
    pub fn identity(&self) -> String {
        self.id.key()
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ProviderCapabilities {
    pub inventory: bool,
    pub metadata: bool,
    pub dependencies: bool,
    pub reverse_candidates: bool,
    pub removal_preview: bool,
    pub removal_execution: bool,
    pub reasons: BTreeMap<String, String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DependencyReport {
    pub package: PackageId,
    pub requires: Vec<String>,
    pub provides: Vec<String>,
    pub reverse_candidates: Vec<PackageId>,
    pub reverse_queried: bool,
    pub limitations: Vec<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChangeAction {
    Remove,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlannedChange {
    pub native_id: String,
    pub action: ChangeAction,
    pub reason: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RemovalReview {
    pub executor_label: String,
    pub requested: PackageId,
    pub changes: Vec<PlannedChange>,
    pub notes: Vec<String>,
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_ids_versions_and_scopes_are_not_split_or_conflated() {
        let a = PackageId {
            provider: "one".into(),
            scope: "system".into(),
            native: "name:epoch/branch::x86_64".into(),
        };
        let mut b = a.clone();
        b.provider = "two".into();
        assert_ne!(a.key(), b.key());
        b = a.clone();
        b.scope = "user:1000".into();
        assert_ne!(a.key(), b.key());
        let decoded: (String, String, String) = serde_json::from_str(&a.key()).unwrap();
        assert_eq!(decoded.2, a.native);
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Service {
    pub unit: String,
    pub scope: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Application {
    pub name: String,
    pub desktop_file: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entry {
    #[serde(flatten)]
    pub package: PackageRecord,
    pub category: String,
    #[serde(default)]
    pub capabilities: ProviderCapabilities,
    pub applications: Vec<Application>,
    pub services: Vec<Service>,
}
