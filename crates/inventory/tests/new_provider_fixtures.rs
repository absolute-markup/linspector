//! Filesystem/API fixtures are synthetic; they do not claim an Arch/Snap runtime test.
use linspector_inventory::{
    model::*,
    platform::{self, Facts},
    providers::*,
};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};
static COUNT: AtomicUsize = AtomicUsize::new(0);
struct Root(PathBuf);
impl Root {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "linspector-pacman-{}-{}",
            std::process::id(),
            COUNT.fetch_add(1, Ordering::SeqCst)
        ));
        for name in ["example-lib-1.0-1", "example-app-1.0-1"] {
            let dest = root.join("var/lib/pacman/local").join(name);
            fs::create_dir_all(&dest).unwrap();
            for file in ["desc", "files"] {
                fs::copy(
                    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                        .join("../../fixtures/pacman/local")
                        .join(name)
                        .join(file),
                    dest.join(file),
                )
                .unwrap();
            }
        }
        fs::write(root.join("var/lib/pacman/local/ALPM_DB_VERSION"), "9\n").unwrap();
        fs::create_dir_all(root.join("usr/share/applications")).unwrap();
        fs::create_dir_all(root.join("usr/lib/systemd/system")).unwrap();
        fs::write(
            root.join("usr/share/applications/example.desktop"),
            "[Desktop Entry]\nType=Application\nName=Example Studio\nExec=NEVER_EXECUTE\n",
        )
        .unwrap();
        fs::write(
            root.join("usr/lib/systemd/system/example.service"),
            "[Service]\nExecStart=NEVER_EXECUTE\n",
        )
        .unwrap();
        Self(root)
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn pacman_root_keeps_metadata_ownership_and_dependencies_without_changes() {
    let root = Root::new();
    let file = root.0.join("var/lib/pacman/local/example-lib-1.0-1/desc");
    fn fingerprint(path: &std::path::Path) -> Vec<(PathBuf, Vec<u8>)> {
        let mut result = vec![];
        for item in fs::read_dir(path).unwrap() {
            let p = item.unwrap().path();
            if p.is_dir() {
                result.extend(fingerprint(&p));
            } else {
                result.push((p.clone(), fs::read(p).unwrap()));
            }
        }
        result.sort_by(|a, b| a.0.cmp(&b.0));
        result
    }
    let database = root.0.join("var/lib/pacman/local");
    let before = fingerprint(&database);
    let provider = inspect_pacman_root(&root.0).unwrap();
    let inventory = provider.inventory().unwrap();
    assert_eq!(inventory.entries.len(), 2);
    assert!(inventory.warnings.is_empty());
    let lib = inventory
        .entries
        .iter()
        .find(|e| e.package.name == "example-lib")
        .unwrap();
    let app = inventory
        .entries
        .iter()
        .find(|e| e.package.name == "example-app")
        .unwrap();
    assert_eq!(lib.package.version_display, "2:1.0~rc1-3");
    assert_eq!(lib.package.installed_bytes, Some(1234));
    assert_eq!(app.category, "Applications");
    assert_eq!(app.applications[0].name, "Example Studio");
    assert_eq!(app.services[0].unit, "example.service");
    assert_eq!(
        provider
            .dependencies(&lib.package.id, true)
            .unwrap()
            .reverse_candidates,
        vec![app.package.id.clone()]
    );
    assert_eq!(
        provider.metadata(&app.package.id).unwrap().key,
        app.package.key
    );
    assert!(provider.prepare_removal(&lib.package.id).is_err());
    assert_eq!(fingerprint(&database), before);
    assert!(file.exists());
}
#[test]
fn pacman_locks_and_out_of_root_database_links_fail_instead_of_empty_success() {
    let root = Root::new();
    let p = inspect_pacman_root(&root.0).unwrap();
    let lock = root.0.join("var/lib/pacman/db.lck");
    fs::write(&lock, "").unwrap();
    assert!(p.inventory().unwrap_err().to_string().contains("locked"));
    fs::remove_file(lock).unwrap();
    let file = root.0.join("var/lib/pacman/local/example-app-1.0-1/desc");
    fs::remove_file(&file).unwrap();
    std::os::unix::fs::symlink("/etc/os-release", &file).unwrap();
    assert!(p.inventory().is_err());
}
#[test]
fn snap_fixture_lists_inactive_revisions_local_revisions_and_installed_components_only() {
    let text = include_str!("../../../fixtures/snap/installed.json");
    let inventory = inspect_snap_fixture(text).unwrap();
    assert_eq!(inventory.entries.len(), 5);
    assert!(!inventory.capabilities.removal_execution);
    let entries = &inventory.entries;
    assert!(entries.iter().any(|e| e.package.id.native == "example@11"));
    assert!(entries
        .iter()
        .any(|e| e.package.id.native == "local-demo_instance@x1"));
    assert!(!entries
        .iter()
        .any(|e| e.package.name.contains("available-only")));
    let component = entries
        .iter()
        .find(|e| e.package.name == "example+extras")
        .unwrap();
    assert_eq!(component.package.installed_bytes, Some(100));
    assert_eq!(component.package.requires, vec!["snap:example@12"]);
    assert_eq!(
        component.package.native_metadata["snap.install-date"],
        "2026-10-26T13:00:00Z"
    );
    assert_eq!(component.package.native_metadata["snap.type"], "component");
    assert!(component.package.license.is_none());
    assert!(entries
        .iter()
        .all(|e| e.package.id.scope == "fixture:system"
            && e.package.provenance.contains("Synthetic")));
    assert!(inspect_snap_fixture(&text.replace("1234", "-1")).is_err());
}
#[test]
fn arch_suse_snap_routing_is_specific_and_never_grants_foreign_execution() {
    let facts = |release: &str| Facts {
        os_release: release.into(),
        tools: [
            "pacman",
            "rpm",
            "zypper",
            "dnf5",
            "snap",
            "flatpak",
            "pkexec",
            "linspector-helper",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect(),
        systemd_running: true,
        ..Default::default()
    };
    let arch = platform::identify(&facts("ID=arch\n"));
    let provider = select(&arch).unwrap();
    assert_eq!(provider.id(), "pacman");
    assert!(!provider.capabilities().removal_execution);
    let suse = platform::identify(&facts("ID=opensuse-tumbleweed\n"));
    let provider = select(&suse).unwrap();
    assert_eq!(provider.id(), "rpm");
    assert!(provider.capabilities().removal_preview);
    assert!(!provider.capabilities().removal_execution);
    assert!(!suse.capabilities.service_startup);
    let id = PackageId {
        provider: "snap".into(),
        scope: "system".into(),
        native: "example@12".into(),
    };
    let provider = for_package(&suse, &id).unwrap();
    assert_eq!(provider.id(), "snap");
    assert!(provider.prepare_removal(&id).is_err());
    let mut f = facts("ID=opensuse-microos\n");
    f.transactional = true;
    assert!(!platform::identify(&f).capabilities.removal_preview);
}
