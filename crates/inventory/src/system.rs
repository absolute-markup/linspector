//! System/storage coordinator. Package discovery is delegated to a selected provider.
pub use crate::model::{Application, Entry, Service};
use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Disk {
    pub source: String,
    pub filesystem: String,
    pub total: u64,
    pub used: u64,
    pub available: u64,
    pub mount: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SourceStatus {
    pub provider: String,
    pub label: String,
    pub complete: bool,
    pub entry_count: Option<usize>,
    pub message: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Snapshot {
    pub platform: crate::platform::Platform,
    pub inventory_complete: bool,
    pub sources: Vec<SourceStatus>,
    pub provider_label: String,
    pub host: crate::host::Report,
    pub entries: Vec<Entry>,
    pub disks: Vec<Disk>,
    pub warnings: Vec<String>,
    pub source: String,
}
fn output(program: &str, args: &[&str]) -> Result<String, String> {
    let out = Command::new(program)
        .args(args)
        .env("LC_ALL", "C")
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).into());
    }
    String::from_utf8(out.stdout).map_err(|e| e.to_string())
}
pub fn parse_disks(text: &str) -> Vec<Disk> {
    text.lines()
        .skip(1)
        .filter_map(|line| {
            let f: Vec<_> = line.split_whitespace().collect();
            if f.len() < 6 || (!f[0].starts_with("/dev/") && f[5] != "/") {
                return None;
            }
            Some(Disk {
                source: f[0].into(),
                filesystem: f[1].into(),
                total: f[2].parse().ok()?,
                used: f[3].parse().ok()?,
                available: f[4].parse().ok()?,
                mount: f[5..].join(" "),
            })
        })
        .collect()
}
fn assemble(
    mut platform: crate::platform::Platform,
    host: crate::host::Report,
    disks: Result<Vec<Disk>, String>,
    inventory: Result<crate::providers::Inventory, String>,
) -> Snapshot {
    let mut warnings = vec![];
    let mut provider_label = platform.package_system.clone();
    let mut source =
        "Live system details and available storage · package inventory unavailable".to_string();
    let (entries, complete) = match inventory {
        Ok(inventory) => {
            provider_label = inventory.provider_label;
            source = format!("{} + mounted filesystems", inventory.source);
            platform.capabilities.removal_preview &= inventory.capabilities.removal_preview;
            platform.capabilities.package_removal &= inventory.capabilities.removal_execution;
            platform.capabilities.reverse_dependencies &= inventory.capabilities.reverse_candidates;
            let entries = inventory.entries;
            let discovery_warnings = inventory.warnings;
            if !discovery_warnings.is_empty() {
                platform.capabilities.service_startup = false;
                platform.capabilities.service_reason =
                    "Package service ownership discovery is incomplete.".into();
            }
            warnings.extend(discovery_warnings);
            (entries, true)
        }
        Err(e) => {
            warnings.push(e.clone());
            platform.capabilities.package_inventory = false;
            platform.capabilities.reverse_dependencies = false;
            platform.capabilities.package_removal = false;
            platform.capabilities.removal_preview = false;
            platform.capabilities.service_startup = false;
            platform.capabilities.inventory_reason = e;
            platform.capabilities.removal_reason =
                "Package inventory is unavailable. A successful scan is required before changes."
                    .into();
            (vec![], false)
        }
    };
    let disks = match disks {
        Ok(disks) => disks,
        Err(e) => {
            warnings.push(format!("Storage discovery unavailable: {e}"));
            vec![]
        }
    };
    let provider = entries
        .first()
        .map(|e| e.package.id.provider.clone())
        .unwrap_or_else(|| platform.family.clone());
    let sources = vec![SourceStatus {
        provider,
        label: provider_label.clone(),
        complete,
        entry_count: complete.then_some(entries.len()),
        message: if complete {
            "Native package inventory loaded".into()
        } else {
            platform.capabilities.inventory_reason.clone()
        },
    }];
    Snapshot {
        sources,
        platform,
        inventory_complete: complete,
        host,
        entries,
        disks,
        warnings,
        source,
        provider_label,
    }
}
fn inventory_for(
    platform: &crate::platform::Platform,
    discover: impl FnOnce() -> Result<crate::providers::Inventory, String>,
) -> Result<crate::providers::Inventory, String> {
    if platform.capabilities.package_inventory {
        discover()
    } else {
        Err(platform.capabilities.inventory_reason.clone())
    }
}
pub fn scan() -> Result<Snapshot, String> {
    let platform = crate::platform::detect();
    // These run even when the distribution has no implemented package provider.
    let host = crate::host::collect();
    let disks = output(
        "/usr/bin/df",
        &["-B1", "--output=source,fstype,size,used,avail,target"],
    )
    .map(|s| parse_disks(&s));
    let inventory = inventory_for(&platform, || {
        crate::providers::select(&platform)
            .and_then(|p| p.inventory())
            .map_err(|e| e.to_string())
    });
    let extra = if platform.capabilities.flatpak_inventory {
        Some(
            crate::providers::flatpak(&platform)
                .and_then(|p| p.inventory())
                .map_err(|e| e.to_string()),
        )
    } else {
        None
    };
    let snap = if platform.capabilities.snap_inventory {
        Some(
            crate::providers::snap(&platform)
                .and_then(|p| p.inventory())
                .map_err(|e| e.to_string()),
        )
    } else {
        None
    };
    let mut snapshot = assemble(platform, host, disks, inventory);
    append_source(&mut snapshot, "flatpak", "Flatpak", extra);
    append_source(&mut snapshot, "snap", "Snap", snap);
    Ok(snapshot)
}
/// A secondary-source failure never erases another source's entries/capabilities.
fn append_source(
    snapshot: &mut Snapshot,
    provider: &str,
    label: &str,
    result: Option<Result<crate::providers::Inventory, String>>,
) {
    let Some(result) = result else {
        return;
    };
    let status = match result {
        Ok(inventory) => {
            let count = inventory.entries.len();
            let message = if inventory.warnings.is_empty() {
                "Installed refs loaded".into()
            } else {
                inventory.warnings.join("\n")
            };
            snapshot.warnings.extend(inventory.warnings);
            snapshot.entries.extend(inventory.entries);
            snapshot.source = format!("{} + {}", snapshot.source, inventory.source);
            SourceStatus {
                provider: provider.into(),
                label: label.into(),
                complete: true,
                entry_count: Some(count),
                message,
            }
        }
        Err(error) => {
            snapshot.inventory_complete = false;
            snapshot
                .warnings
                .push(format!("{label} inventory unavailable: {error}"));
            SourceStatus {
                provider: provider.into(),
                label: label.into(),
                complete: false,
                entry_count: None,
                message: error,
            }
        }
    };
    snapshot.sources.push(status);
    snapshot.provider_label = snapshot
        .sources
        .iter()
        .filter(|s| s.complete)
        .map(|s| s.label.as_str())
        .collect::<Vec<_>>()
        .join(" + ");
    snapshot.entries.sort_by_key(|e| e.package.identity());
}
pub fn demo() -> Result<Snapshot, String> {
    let p: Vec<crate::Package> =
        serde_json::from_str(include_str!("../../../fixtures/sample-inventory.json"))
            .map_err(|e| e.to_string())?;
    Ok(Snapshot {
        platform: crate::platform::Platform {
            name: "Synthetic demo".into(),
            ..Default::default()
        },
        inventory_complete: true,
        sources: vec![SourceStatus {
            provider: "demo".into(),
            label: "Demo".into(),
            complete: true,
            entry_count: Some(p.len()),
            message: "Synthetic fixture".into(),
        }],
        provider_label: "Demo".into(),
        host: crate::host::Report {
            demo: true,
            ..Default::default()
        },
        entries: crate::providers::demo_entries(p),
        disks: vec![],
        warnings: vec!["Synthetic demo. System actions are unavailable.".into()],
        source: "Synthetic demo — not a system scan".into(),
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unsupported_provider_never_runs_rpm_and_preserves_other_sections() {
        let platform = crate::platform::identify(&crate::platform::Facts {
            os_release: "ID=ubuntu\nID_LIKE=debian\n".into(),
            ..Default::default()
        });
        let inventory = inventory_for(&platform, || panic!("must not invoke RPM on Ubuntu"));
        let host = crate::host::Report {
            hostname: "fixture-host".into(),
            memory_total: 1024,
            ..Default::default()
        };
        let disks = vec![Disk {
            source: "/dev/test".into(),
            filesystem: "ext4".into(),
            total: 1000,
            used: 100,
            available: 900,
            mount: "/".into(),
        }];
        let snapshot = assemble(platform, host, Ok(disks), inventory);
        assert_eq!(snapshot.host.hostname, "fixture-host");
        assert_eq!(snapshot.disks[0].total, 1000);
        assert!(!snapshot.inventory_complete);
        assert!(!snapshot.platform.capabilities.package_removal);
        assert!(snapshot.entries.is_empty());
        assert!(snapshot.warnings[0].contains("dpkg-query"));
    }
    #[test]
    fn query_failure_disables_actions_without_losing_system_details() {
        let mut platform = crate::platform::Platform::default();
        platform.capabilities.package_inventory = true;
        platform.capabilities.package_removal = true;
        platform.capabilities.service_startup = true;
        let s = assemble(
            platform,
            crate::host::Report {
                hostname: "still-readable".into(),
                ..Default::default()
            },
            Ok(vec![]),
            Err("RPM database temporarily locked".into()),
        );
        assert_eq!(s.host.hostname, "still-readable");
        assert!(!s.platform.capabilities.package_removal);
        assert!(!s.platform.capabilities.service_startup);
        assert!(s.warnings[0].contains("locked"));
    }
    #[test]
    fn disk_metrics_are_not_added_and_spaces_survive() {
        let p=parse_disks("Filesystem Type bytes used avail mount\n/dev/sda btrfs 1000 300 600 /\n/dev/sda btrfs 1000 300 600 /home\n/dev/sdb ext4 500 200 280 /media/My Drive\ntmpfs tmpfs 100 1 99 /run\n");
        assert_eq!(p.len(), 3);
        assert_eq!(p[2].mount, "/media/My Drive");
        assert_eq!(p[0].available, 600);
    }
}

#[cfg(test)]
mod multiple_sources_tests {
    use super::*;
    fn inventory() -> crate::providers::Inventory {
        let packages: Vec<crate::Package> =
            serde_json::from_str(include_str!("../../../fixtures/sample-inventory.json")).unwrap();
        crate::providers::Inventory {
            entries: crate::providers::demo_entries(packages),
            warnings: vec![],
            source: "Test provider".into(),
            provider_label: "RPM".into(),
            capabilities: crate::model::ProviderCapabilities {
                inventory: true,
                removal_preview: true,
                removal_execution: true,
                ..Default::default()
            },
        }
    }
    #[test]
    fn secondary_failure_preserves_native_entries_and_native_preview_capability() {
        let mut platform = crate::platform::Platform::default();
        platform.capabilities.package_inventory = true;
        platform.capabilities.removal_preview = true;
        let mut snapshot = assemble(platform, Default::default(), Ok(vec![]), Ok(inventory()));
        let keys: std::collections::BTreeSet<_> = snapshot
            .entries
            .iter()
            .map(|e| e.package.key.clone())
            .collect();
        append_source(
            &mut snapshot,
            "flatpak",
            "Flatpak",
            Some(Err("Flatpak inaccessible".into())),
        );
        assert_eq!(
            snapshot
                .entries
                .iter()
                .map(|e| e.package.key.clone())
                .collect::<std::collections::BTreeSet<_>>(),
            keys
        );
        assert!(snapshot.platform.capabilities.removal_preview);
        assert!(!snapshot.inventory_complete);
        assert_eq!(snapshot.sources[1].entry_count, None);
    }
    #[test]
    fn secondary_success_survives_native_failure_without_granting_native_management() {
        let mut snapshot = assemble(
            Default::default(),
            Default::default(),
            Ok(vec![]),
            Err("Native provider unsupported".into()),
        );
        let mut extra = inventory();
        for e in &mut extra.entries {
            e.package.id.provider = "flatpak".into();
            e.package.key = e.package.id.key();
            e.capabilities = Default::default();
        }
        append_source(&mut snapshot, "flatpak", "Flatpak", Some(Ok(extra)));
        assert_eq!(snapshot.entries.len(), 3);
        assert!(!snapshot.inventory_complete);
        assert!(snapshot.sources[1].complete);
        assert!(!snapshot.platform.capabilities.package_removal);
        assert_eq!(snapshot.sources[0].entry_count, None);
    }
    #[test]
    fn empty_secondary_installation_is_zero_and_failed_installation_is_unknown() {
        let mut snapshot = assemble(
            Default::default(),
            Default::default(),
            Ok(vec![]),
            Ok(inventory()),
        );
        let mut empty = inventory();
        empty.entries.clear();
        append_source(&mut snapshot, "flatpak", "Flatpak", Some(Ok(empty)));
        assert_eq!(snapshot.sources[1].entry_count, Some(0));
        assert!(snapshot.sources[1].complete);
    }
}
