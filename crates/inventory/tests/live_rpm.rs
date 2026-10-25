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
