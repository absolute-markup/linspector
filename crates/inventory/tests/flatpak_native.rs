//! Opt-in read-only native Flatpak checks. Fixture creation is a separate /tmp-only step.
use linspector_inventory::{
    model::{Entry, PackageId},
    platform, providers,
};
use serde_json::Value;
use std::{collections::BTreeMap, path::Path, process::Command};
fn output(command: &mut Command) -> String {
    let result = command.env("LC_ALL", "C").output().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    String::from_utf8(result.stdout).unwrap()
}
fn native_snapshot(command: impl Fn() -> Command) -> String {
    output(command().args([
        "list",
        "--all",
        "--json",
        "--columns=ref,installation,active,version",
    ]))
}
fn reference_ref(reference: &str, kind: &str) -> String {
    if reference.starts_with("app/") || reference.starts_with("runtime/") {
        reference.into()
    } else {
        format!("{kind}/{reference}")
    }
}
fn scope_flag(scope: &str) -> String {
    if scope.starts_with("user:") {
        "--user".into()
    } else if scope == "system" {
        "--system".into()
    } else {
        format!("--installation={}", scope.strip_prefix("system:").unwrap())
    }
}
#[test]
#[ignore = "requires installed Flatpak refs; read-only, no applications launched"]
fn live_inventory_matches_native_refs_sizes_and_runtime_candidates_without_changes() {
    let before = native_snapshot(|| Command::new("/usr/bin/flatpak"));
    let provider = providers::flatpak(&platform::detect()).unwrap();
    let inventory = provider.inventory().unwrap();
    assert!(inventory.warnings.is_empty(), "{:?}", inventory.warnings);
    let mut expected = BTreeMap::new();
    for kind in ["app", "runtime"] {
        let rows: Vec<Value> =
            serde_json::from_str(&output(Command::new("/usr/bin/flatpak").args([
                "list",
                "--all",
                if kind == "app" { "--app" } else { "--runtime" },
                "--json",
                "--columns=ref,installation,version",
            ])))
            .unwrap();
        for row in rows {
            let label = row["installation"].as_str().unwrap();
            let scope = if label == "user" {
                let uid = output(Command::new("/usr/bin/id").arg("-ru"));
                format!("user:{}", uid.trim())
            } else if label == "system" {
                "system".into()
            } else {
                format!(
                    "system:{}",
                    label
                        .strip_prefix("system (")
                        .unwrap()
                        .strip_suffix(')')
                        .unwrap()
                )
            };
            let reference = reference_ref(row["ref"].as_str().unwrap(), kind);
            expected.insert(
                PackageId {
                    provider: "flatpak".into(),
                    scope,
                    native: reference,
                },
                row["version"].as_str().unwrap().to_string(),
            );
        }
    }
    assert_eq!(inventory.entries.len(), expected.len());
    assert!(!inventory.entries.is_empty());
    for entry in &inventory.entries {
        let version = &expected[&entry.package.id];
        if !version.is_empty() {
            assert_eq!(entry.package.version_display, *version);
        }
        let info = output(Command::new("/usr/bin/flatpak").args([
            "info",
            &scope_flag(&entry.package.id.scope),
            "--show-commit",
            "--show-size",
            &entry.package.id.native,
        ]));
        let fields: Vec<_> = info.split_whitespace().collect();
        assert_eq!(
            entry.package.installed_bytes,
            Some(fields[1].parse().unwrap())
        );
        assert_eq!(entry.package.native_metadata["flatpak.commit"], fields[0]);
        assert!(!entry.capabilities.removal_preview);
        assert!(!entry.capabilities.removal_execution);
    }
    let app = inventory
        .entries
        .iter()
        .find(|e| !e.package.requires.is_empty())
        .unwrap();
    assert_eq!(
        provider.metadata(&app.package.id).unwrap().installed_bytes,
        app.package.installed_bytes
    );
    let runtime = inventory
        .entries
        .iter()
        .find(|e| {
            e.package.id.scope == app.package.id.scope
                && app.package.requires.contains(&e.package.id.native)
        })
        .unwrap();
    let report = provider.dependencies(&runtime.package.id, true).unwrap();
    let reference = output(Command::new("/usr/bin/flatpak").args([
        "list",
        &scope_flag(&runtime.package.id.scope),
        "--all",
        "--app",
        &format!(
            "--app-runtime={}",
            runtime.package.id.native.strip_prefix("runtime/").unwrap()
        ),
        "--columns=ref:f",
    ]));
    let reference: std::collections::BTreeSet<_> = reference
        .lines()
        .filter(|l| !l.is_empty())
        .map(|r| reference_ref(r, "app"))
        .collect();
    assert_eq!(
        report
            .reverse_candidates
            .iter()
            .map(|id| id.native.clone())
            .collect::<std::collections::BTreeSet<_>>(),
        reference
    );
    assert_eq!(before, native_snapshot(|| Command::new("/usr/bin/flatpak")));
    println!(
        "Native live Flatpak: {} refs, exact sizes/commits and runtime candidates; unchanged",
        inventory.entries.len()
    );
}
fn isolated_command(program: &str, root: &Path) -> Command {
    let mut c = Command::new(program);
    for (key, child) in [
        ("HOME", "home"),
        ("XDG_DATA_HOME", "data"),
        ("XDG_CONFIG_HOME", "user-config"),
        ("XDG_CACHE_HOME", "cache"),
        ("XDG_RUNTIME_DIR", "runtime"),
        ("FLATPAK_USER_DIR", "user"),
        ("FLATPAK_SYSTEM_DIR", "system"),
        ("FLATPAK_CONFIG_DIR", "config"),
        ("FLATPAK_SYSTEM_CACHE_DIR", "system-cache"),
    ] {
        c.env(key, root.join(child));
    }
    c
}
#[test]
#[ignore = "set LINSPECTOR_FLATPAK_FIXTURE_ROOT from scripts/create_flatpak_fixture.py; reads only"]
fn isolated_native_fixture_preserves_user_default_named_scopes_and_parallel_branches() {
    let root = std::path::PathBuf::from(
        std::env::var_os("LINSPECTOR_FLATPAK_FIXTURE_ROOT").expect("fixture root required"),
    );
    let report: Value =
        serde_json::from_slice(&std::fs::read(root.join("fixture-report.json")).unwrap()).unwrap();
    assert_eq!(report["synthetic"], true);
    let before = native_snapshot(|| isolated_command("/usr/bin/flatpak", &root));
    let exported: Value = serde_json::from_str(&output(
        isolated_command(env!("CARGO_BIN_EXE_linspector-inventory"), &root).arg("--flatpak"),
    ))
    .unwrap();
    let entries: Vec<Entry> = serde_json::from_value(exported["entries"].clone()).unwrap();
    assert_eq!(entries.len(), 12);
    assert_eq!(exported["warnings"], serde_json::json!([]));
    let mut counts = BTreeMap::new();
    let mut keys = std::collections::BTreeSet::new();
    for entry in &entries {
        *counts.entry(entry.package.id.scope.clone()).or_insert(0) += 1;
        keys.insert(entry.package.key.clone());
        assert!(
            entry.package.id.native.ends_with("/stable")
                || entry.package.id.native.ends_with("/beta")
        );
        assert!(entry.package.installed_bytes.unwrap() > 0);
        assert_eq!(entry.package.native_metadata["flatpak.commit"].len(), 64);
        assert!(!entry.capabilities.removal_execution);
        let info = output(isolated_command("/usr/bin/flatpak", &root).args([
            "info",
            &scope_flag(&entry.package.id.scope),
            "--show-size",
            &entry.package.id.native,
        ]));
        assert_eq!(
            entry.package.installed_bytes,
            Some(info.trim().parse().unwrap())
        );
    }
    assert_eq!(keys.len(), 12);
    assert_eq!(counts.len(), 3);
    assert_eq!(counts["system"], 4);
    assert_eq!(counts["system:fixture-extra"], 4);
    assert!(counts
        .iter()
        .any(|(k, v)| k.starts_with("user:") && *v == 4));
    assert_eq!(
        before,
        native_snapshot(|| isolated_command("/usr/bin/flatpak", &root))
    );
}
