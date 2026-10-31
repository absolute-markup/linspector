//! Tests native read-only dpkg queries. These never run an installation/removal.
use linspector_inventory::{
    platform::{self, Facts},
    providers::{self, ErrorKind},
};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};
static SEQUENCE: AtomicUsize = AtomicUsize::new(0);
struct FixtureRoot(PathBuf);
impl FixtureRoot {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "linspector-dpkg-test-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::SeqCst)
        ));
        fs::create_dir_all(path.join("var/lib/dpkg/info")).unwrap();
        fs::write(
            path.join("var/lib/dpkg/status"),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../fixtures/dpkg/status"
            )),
        )
        .unwrap();
        fs::write(path.join("var/lib/dpkg/arch"), "amd64\ni386\n").unwrap();
        fs::write(path.join("var/lib/dpkg/info/format"), "1\n").unwrap();
        for name in [
            "sample-lib:amd64",
            "sample-lib:i386",
            "sample-dev",
            "damaged-tool",
            "removed-tool",
        ] {
            fs::write(path.join(format!("var/lib/dpkg/info/{name}.list")), "/.\n").unwrap();
        }
        fs::write(path.join("var/lib/dpkg/info/sample-app.list"),"/.\n/usr/share/applications/sample.desktop\n/usr/share/applications/hidden.desktop\n/usr/share/applications/escape.desktop\n/usr/lib/systemd/system/sample.service\n").unwrap();
        fs::create_dir_all(path.join("usr/share/applications")).unwrap();
        fs::create_dir_all(path.join("usr/lib/systemd/system")).unwrap();
        fs::write(
            path.join("usr/share/applications/sample.desktop"),
            "[Desktop Entry]\nType=Application\nName=Sample Studio\nExec=NEVER_EXECUTE_THIS\n",
        )
        .unwrap();
        fs::write(
            path.join("usr/share/applications/hidden.desktop"),
            "[Desktop Entry]\nType=Application\nName=Hidden app\nNoDisplay=true\n",
        )
        .unwrap();
        fs::write(
            path.join("usr/lib/systemd/system/sample.service"),
            "[Service]\nExecStart=NEVER_EXECUTE_THIS\n",
        )
        .unwrap();
        // This absolute symlink escapes the inspection root and must never be read.
        std::os::unix::fs::symlink(
            "/etc/os-release",
            path.join("usr/share/applications/escape.desktop"),
        )
        .unwrap();
        Self(path)
    }
}
impl Drop for FixtureRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn snapshot(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    fn walk(p: &Path, out: &mut Vec<(PathBuf, Vec<u8>)>) {
        let mut entries: Vec<_> = fs::read_dir(p).unwrap().filter_map(Result::ok).collect();
        entries.sort_by_key(|e| e.path());
        for e in entries {
            if e.file_type().unwrap().is_dir() {
                walk(&e.path(), out);
            } else if e.file_type().unwrap().is_file() {
                out.push((e.path(), fs::read(e.path()).unwrap()));
            }
        }
    }
    let mut out = vec![];
    walk(&root.join("var/lib/dpkg"), &mut out);
    out
}
#[test]
#[ignore = "requires /usr/bin/dpkg-query; native read-only fixture validation"]
fn native_query_preserves_apps_multilib_status_and_database_bytes() {
    let root = FixtureRoot::new();
    let before = snapshot(&root.0);
    let provider = providers::inspect_dpkg_root(&root.0).unwrap();
    let inv = provider.inventory().unwrap();
    assert_eq!(inv.entries.len(), 5);
    assert!(!inv.capabilities.removal_execution);
    let app = inv
        .entries
        .iter()
        .find(|e| e.package.name == "sample-app")
        .unwrap();
    assert_eq!(
        app.category, "Applications",
        "ownership warnings: {:?}",
        inv.warnings
    );
    assert_eq!(app.applications.len(), 1);
    assert_eq!(app.applications[0].name, "Sample Studio");
    assert_eq!(app.services.len(), 1);
    assert_eq!(app.services[0].unit, "sample.service");
    assert!(app.package.id.scope.starts_with("root:"));
    assert!(inv
        .warnings
        .iter()
        .any(|s| s.contains("not in a fully configured")));
    let lib = inv
        .entries
        .iter()
        .find(|e| e.package.name == "sample-lib" && e.package.arch == "amd64")
        .unwrap();
    assert_eq!(
        provider.metadata(&lib.package.id).unwrap().description,
        lib.package.description
    );
    let deps = provider.dependencies(&lib.package.id, true).unwrap();
    assert!(deps
        .reverse_candidates
        .iter()
        .any(|id| id.native.starts_with("sample-app:")));
    assert!(matches!(
        provider.prepare_removal(&app.package.id),
        Err(providers::ProviderError {
            kind: ErrorKind::Unsupported,
            ..
        })
    ));
    assert_eq!(before, snapshot(&root.0));
    // Missing ownership data must warn without hiding other valid application entries.
    fs::remove_file(root.0.join("var/lib/dpkg/info/sample-lib:amd64.list")).unwrap();
    let partial = provider.inventory().unwrap();
    assert_eq!(
        partial
            .entries
            .iter()
            .find(|e| e.package.name == "sample-app")
            .unwrap()
            .category,
        "Applications"
    );
    assert!(partial
        .warnings
        .iter()
        .any(|s| s.contains("ownership discovery incomplete")));
}
#[test]
fn apt_detection_selects_readonly_dpkg_and_never_grants_fedora_operations() {
    for (id, like) in [
        ("ubuntu", "debian"),
        ("debian", ""),
        ("linuxmint", "ubuntu debian"),
    ] {
        let p = platform::identify(&Facts {
            os_release: format!("ID={id}\nID_LIKE=\"{like}\"\nVERSION_ID=24.04\n"),
            tools: ["dpkg-query", "rpm", "dnf5", "pkexec", "systemctl"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            systemd_running: true,
            ..Default::default()
        });
        let provider = providers::select(&p).unwrap();
        assert_eq!(provider.id(), "dpkg");
        assert!(provider.capabilities().inventory);
        assert!(!provider.capabilities().removal_preview);
        assert!(!p.capabilities.service_startup);
    }
}
#[test]
#[ignore = "set LINSPECTOR_DPKG_ROOT to an extracted Ubuntu/Debian root; read-only"]
fn real_root_matches_native_show_and_survives_metadata_and_dependency_queries() {
    let root = PathBuf::from(
        std::env::var_os("LINSPECTOR_DPKG_ROOT").expect("LINSPECTOR_DPKG_ROOT is required"),
    );
    let before = snapshot(&root);
    let provider = providers::inspect_dpkg_root(&root).unwrap();
    let inv = provider.inventory().unwrap();
    let out = Command::new("/usr/bin/dpkg-query")
        .arg(format!(
            "--admindir={}",
            root.join("var/lib/dpkg").display()
        ))
        .args([
            "--show",
            "--showformat=${Package}\t${Architecture}\t${Version}\t${db:Status-Status}\n",
        ])
        .env("LC_ALL", "C.UTF-8")
        .output()
        .unwrap();
    assert!(out.status.success());
    let reference: BTreeSet<_> = String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .filter_map(|l| {
            let f: Vec<_> = l.split('\t').collect();
            (f.len() == 4
                && matches!(
                    f[3],
                    "installed"
                        | "unpacked"
                        | "half-installed"
                        | "half-configured"
                        | "triggers-awaited"
                        | "triggers-pending"
                ))
            .then(|| format!("{}:{}={}", f[0], f[1], f[2]))
        })
        .collect();
    assert_eq!(
        inv.entries
            .iter()
            .map(|e| e.package.id.native.clone())
            .collect::<BTreeSet<_>>(),
        reference
    );
    assert!(!inv.entries.is_empty());
    assert!(inv.entries.iter().any(|e| e.package.name == "dpkg"));
    let target = inv
        .entries
        .iter()
        .find(|e| e.package.name == "libc6")
        .unwrap();
    let metadata = provider.metadata(&target.package.id).unwrap();
    assert_eq!(metadata.version_display, target.package.version_display);
    assert!(metadata.installed_bytes.unwrap() > 0);
    let deps = provider.dependencies(&target.package.id, true).unwrap();
    assert!(!deps.reverse_candidates.is_empty());
    assert!(deps
        .reverse_candidates
        .iter()
        .all(|id| reference.contains(&id.native)));
    assert_eq!(before, snapshot(&root));
    println!(
        "Validated {} package identities in {}",
        inv.entries.len(),
        root.display()
    );
}
