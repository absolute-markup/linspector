//! Opt-in integration checks against the host RPM DB; never change packages.
#[test]
#[ignore = "requires an RPM host; run with --ignored"]
fn inventory_matches_rpm_and_reverse_query_works() {
    let p = linspector_inventory::installed().unwrap();
    let output = std::process::Command::new("/usr/bin/rpm")
        .args(["-qa", "--qf", "%{NAME}\\n"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(
        p.len(),
        String::from_utf8(output.stdout).unwrap().lines().count()
    );
    let rpm = p.iter().find(|p| p.name == "rpm").unwrap();
    assert!(!rpm.title.is_empty());
    assert!(!rpm.description.is_empty());
    assert!(!rpm.requires.is_empty());
    let reverse = linspector_inventory::reverse_candidates(rpm).unwrap();
    assert!(reverse.iter().all(|r| !r.contains("no package requires")));
    let identities: std::collections::BTreeSet<_> = p.iter().map(|p| p.identity()).collect();
    assert!(
        reverse.iter().all(|r| identities.contains(r)),
        "reverse rows must be installed identities"
    );
    assert!(!reverse.is_empty(), "host RPM has consumers");
}

#[test]
fn uploaded_list_retains_multilib_versions_and_toolchain() {
    let data: serde_json::Value =
        serde_json::from_str(include_str!("../../../fixtures/fedora44-dnf-list.json")).unwrap();
    let rows = data.as_array().unwrap();
    assert_eq!(rows.len(), 3012);
    for name in [
        "gcc-c++",
        "cmake",
        "ninja-build",
        "pkgconf-pkg-config",
        "qt6-qtbase-devel",
        "qt6-qtdeclarative-devel",
        "qt6-qttools-devel",
        "clang",
        "lld",
    ] {
        assert!(rows.iter().any(|r| r["name"] == name), "missing {name}");
    }
    assert!(rows.iter().any(|r| r["arch"] == "i686"));
    assert!(rows
        .iter()
        .any(|r| r["evr"].as_str().unwrap().contains(':')));
    assert!(rows.iter().any(|r| r["repository_display"] == "<unknown>"));
}

#[test]
#[ignore = "requires an RPM host; read-only scan"]
fn live_catalog_discovers_apps_and_disk_without_fixture() {
    let s = linspector_inventory::system::scan().unwrap();
    assert_eq!(
        s.entries
            .iter()
            .filter(|e| e.package.id.provider == "rpm")
            .count(),
        linspector_inventory::installed().unwrap().len()
    );
    assert!(s.entries.iter().any(|e| !e.applications.is_empty()));
    assert!(s.entries.iter().any(|e| e.category == "Libraries"));
    assert!(s.disks.iter().any(|d| d.mount == "/" && d.total > 0));
    assert!(s.source.starts_with("Live"));
}

#[test]
#[ignore = "requires Fedora DNF5; stores a plan but executes no removal"]
fn live_dnf_preview_does_not_remove_any_packages() {
    let before = linspector_inventory::installed().unwrap();
    let p = before
        .iter()
        .find(|p| p.name == "gcc-c++")
        .expect("toolchain fixture host");
    let plan = linspector_inventory::management::preview(p).unwrap();
    assert_eq!(plan.requested, p.identity());
    assert!(plan.rows.iter().all(|r| r.action == "Remove"));
    assert!(plan.rows.iter().any(|r| r.nevra.starts_with("gcc-c++-")));
    let after = linspector_inventory::installed().unwrap();
    assert_eq!(
        before.iter().map(|p| p.identity()).collect::<Vec<_>>(),
        after.iter().map(|p| p.identity()).collect::<Vec<_>>()
    );
}

#[test]
#[ignore = "requires Linux procfs and ordinary system utilities; read-only"]
fn live_system_report_includes_real_uid_account_and_memory() {
    let report = linspector_inventory::host::collect();
    assert!(!report.demo);
    assert!(!report.os.is_empty());
    assert!(report.memory_total > 0);
    assert!(report.memory_available <= report.memory_total);
    assert!(report.logical_cpus > 0);
    let account = report
        .sections
        .iter()
        .find(|s| s.title == "Account & session")
        .unwrap();
    let actual_uid = std::process::Command::new("/usr/bin/id")
        .args(["-ru"])
        .output()
        .unwrap();
    let uid = String::from_utf8(actual_uid.stdout).unwrap();
    assert!(account
        .fields
        .iter()
        .any(|f| f.label == "User ID" && f.value == uid.trim()));
    for label in ["Username", "Home directory", "Login shell"] {
        assert!(account
            .fields
            .iter()
            .any(|f| f.label == label && f.value != "Unavailable"));
    }
    for title in [
        "Operating system",
        "Processor",
        "Memory & swap",
        "Network interfaces",
        "Hardware & firmware",
        "Graphics & displays",
        "Storage devices",
        "Power & battery",
        "Security & platform state",
    ] {
        assert!(report
            .sections
            .iter()
            .any(|s| s.title == title && !s.fields.is_empty()));
    }
}

#[test]
#[ignore = "requires RPM host; compares read-only provider against native RPM inventory"]
fn selected_provider_preserves_all_rpm_identities_and_metadata() {
    let provider = linspector_inventory::providers::current().unwrap();
    let inventory = provider.inventory().unwrap();
    let native = linspector_inventory::installed().unwrap();
    let expected: std::collections::BTreeSet<_> = native.iter().map(|p| p.identity()).collect();
    let actual: std::collections::BTreeSet<_> = inventory
        .entries
        .iter()
        .map(|e| e.package.id.native.clone())
        .collect();
    assert_eq!(actual, expected);
    let gcc = inventory
        .entries
        .iter()
        .find(|e| e.package.name == "gcc-c++")
        .unwrap();
    let metadata = provider.metadata(&gcc.package.id).unwrap();
    assert_eq!(metadata.key, gcc.package.key);
    assert_eq!(metadata.description, gcc.package.description);
    assert_eq!(metadata.requires, gcc.package.requires);
    assert!(metadata.installed_bytes.unwrap() > 0);
    let dependencies = provider.dependencies(&gcc.package.id, false).unwrap();
    assert_eq!(dependencies.requires, gcc.package.requires);
    assert!(!dependencies.reverse_queried);
    let rpm = inventory
        .entries
        .iter()
        .find(|e| e.package.name == "rpm")
        .unwrap();
    let reverse = provider.dependencies(&rpm.package.id, true).unwrap();
    assert!(reverse.reverse_queried);
    assert!(!reverse.reverse_candidates.is_empty());
    assert!(reverse
        .reverse_candidates
        .iter()
        .all(|id| id.provider == "rpm" && expected.contains(&id.native)));
}

#[test]
#[ignore = "requires mutable Fedora 44 DNF5; prepares/discards a plan, never executes it"]
fn provider_removal_review_is_bound_to_selected_native_package() {
    let before = linspector_inventory::installed().unwrap();
    let provider = linspector_inventory::providers::current().unwrap();
    let inventory = provider.inventory().unwrap();
    let gcc = inventory
        .entries
        .iter()
        .find(|e| e.package.name == "gcc-c++")
        .unwrap();
    let plan = provider.prepare_removal(&gcc.package.id).unwrap();
    assert_eq!(plan.review().requested, gcc.package.id);
    assert!(plan
        .review()
        .changes
        .iter()
        .any(|r| r.native_id.starts_with("gcc-c++-")));
    assert!(plan
        .review()
        .changes
        .iter()
        .all(|r| r.action == linspector_inventory::model::ChangeAction::Remove));
    drop(plan); // cancellation path: native plan cleanup, never replay
    let after = linspector_inventory::installed().unwrap();
    assert_eq!(
        before.iter().map(|p| p.identity()).collect::<Vec<_>>(),
        after.iter().map(|p| p.identity()).collect::<Vec<_>>()
    );
}
