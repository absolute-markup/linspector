//! Read-only Flatpak inventory. Native ref kind, architecture, branch and installation
//! scope remain distinct. No run/install/uninstall/override commands are used.
use super::*;
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    process::Command,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Mutex,
    },
    time::{Duration, Instant},
};
const COLUMNS: &str = "ref,installation,name,description,version,origin,runtime,active,size";
const FULL_COLUMNS: &str =
    "ref:f,installation:f,name:f,description:f,version:f,origin:f,runtime:f,active:f,size:f";
#[derive(Default, Debug, Deserialize)]
struct Row {
    #[serde(rename = "ref")]
    reference: String,
    installation: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    version: String,
    #[serde(default)]
    origin: String,
    #[serde(default)]
    runtime: String,
    #[serde(default)]
    active_commit: String,
    #[serde(default)]
    installed_size: String,
}
pub(super) struct FlatpakProvider {
    uid: String,
}
fn failure(message: impl Into<String>) -> ProviderError {
    ProviderError::new(ErrorKind::OperationFailed, message)
}
fn command() -> Command {
    let mut c = crate::process::command("/usr/bin/flatpak");
    // Flatpak's native installation configuration belongs to the invoking user.
    // Do not accidentally enumerate root's HOME after clearing command environment.
    for name in [
        "HOME",
        "XDG_DATA_HOME",
        "XDG_CONFIG_HOME",
        "XDG_CACHE_HOME",
        "XDG_RUNTIME_DIR",
        "FLATPAK_USER_DIR",
        "FLATPAK_SYSTEM_DIR",
        "FLATPAK_CONFIG_DIR",
        "FLATPAK_SYSTEM_CACHE_DIR",
    ] {
        if let Some(value) = std::env::var_os(name) {
            c.env(name, value);
        }
    }
    c
}
impl FlatpakProvider {
    pub(super) fn current() -> Result<Self> {
        let status = fs::read_to_string("/proc/self/status").map_err(|e| failure(e.to_string()))?;
        let uid = status
            .lines()
            .find_map(|l| l.strip_prefix("Uid:"))
            .and_then(|s| s.split_whitespace().next())
            .filter(|s| s.bytes().all(|b| b.is_ascii_digit()))
            .ok_or_else(|| failure("Cannot establish Flatpak user scope"))?;
        Ok(Self { uid: uid.into() })
    }
    fn checked(&self, id: &PackageId) -> Result<()> {
        if id.provider != "flatpak"
            || !(id.scope == format!("user:{}", self.uid)
                || id.scope == "system"
                || id.scope.starts_with("system:"))
        {
            return Err(ProviderError::new(
                ErrorKind::WrongProvider,
                "This Flatpak provider cannot inspect another source/user scope",
            ));
        }
        canonical_ref(&id.native, None)?;
        Ok(())
    }
    fn list(&self, kind: &str) -> Result<Vec<Entry>> {
        let mut c = command();
        c.args([
            "list",
            "--all",
            if kind == "app" { "--app" } else { "--runtime" },
            "--json",
        ]);
        c.arg(format!("--columns={COLUMNS}"));
        let output = crate::process::large_query(&mut c).map_err(failure)?;
        let rows = if output.success() {
            parse_json(&output.query_text().map_err(failure)?)?
        } else if (output.stderr.contains("Unknown option")
            || output.stderr.contains("unrecognized option"))
            && output.stderr.contains("--json")
        {
            // Older Flatpak versions have explicit full-width TSV columns, not JSON.
            let mut c = command();
            c.args([
                "list",
                "--all",
                if kind == "app" { "--app" } else { "--runtime" },
            ]);
            c.arg(format!("--columns={FULL_COLUMNS}"));
            parse_tsv(
                &crate::process::large_query(&mut c)
                    .map_err(failure)?
                    .query_text()
                    .map_err(failure)?,
            )?
        } else {
            return Err(failure(format!(
                "Flatpak {kind} inventory unavailable: {}",
                output.stderr.trim()
            )));
        };
        rows.into_iter()
            .map(|row| normalize(row, kind, &self.uid, self.capabilities()))
            .collect()
    }
    fn records(&self) -> Result<Vec<Entry>> {
        // Atomic per provider: failed app/runtime discovery is not a false empty inventory.
        let mut entries = self.list("app")?;
        entries.extend(self.list("runtime")?);
        let mut seen = BTreeSet::new();
        if entries.iter().any(|e| !seen.insert(e.package.key.clone())) {
            return Err(failure("Duplicate Flatpak ref in the same installation"));
        }
        entries.sort_by_key(|e| e.package.identity());
        Ok(entries)
    }
    fn details(&self, entry: &mut Entry) -> Result<()> {
        let native = &entry.package.native_metadata;
        let mut c = command();
        c.arg("info");
        c.arg(scope_argument(&entry.package.id.scope)?);
        c.args(["--show-commit", "--show-size", &entry.package.id.native]);
        let output = crate::process::run(&mut c, Some(Duration::from_secs(5)))
            .map_err(failure)?
            .query_text()
            .map_err(failure)?;
        let (commit, size) = parse_details(&output)?;
        let previous = native
            .get("flatpak.commit_abbreviated")
            .map(String::as_str)
            .unwrap_or("");
        if !previous.is_empty() && previous != "-" && !commit.starts_with(previous) {
            return Err(failure("Flatpak changed during discovery; scan again"));
        }
        entry.package.installed_bytes = Some(size);
        entry
            .package
            .native_metadata
            .insert("flatpak.commit".into(), commit);
        Ok(())
    }
}
fn parse_json(text: &str) -> Result<Vec<Row>> {
    serde_json::from_str(text).map_err(|e| failure(format!("Invalid Flatpak JSON inventory: {e}")))
}
fn parse_tsv(text: &str) -> Result<Vec<Row>> {
    let mut rows = vec![];
    for line in text.lines().filter(|l| !l.is_empty()) {
        let mut f: Vec<_> = line.split('\t').collect();
        if f.len() < 2 || f.len() > 9 {
            return Err(failure("Invalid Flatpak full-column inventory"));
        }
        // Native printing omits empty trailing columns. Interior empty fields remain tabs.
        f.resize(9, "");
        rows.push(Row {
            reference: f[0].into(),
            installation: f[1].into(),
            name: f[2].into(),
            description: f[3].into(),
            version: f[4].into(),
            origin: f[5].into(),
            runtime: f[6].into(),
            active_commit: f[7].into(),
            installed_size: f[8].into(),
        });
    }
    Ok(rows)
}
fn canonical_ref(value: &str, kind: Option<&str>) -> Result<String> {
    let parts: Vec<_> = value.split('/').collect();
    let (actual, fields) = if parts.len() == 4 {
        (parts[0], &parts[1..])
    } else if parts.len() == 3 {
        (
            kind.ok_or_else(|| failure("Flatpak identity must include its native kind"))?,
            parts.as_slice(),
        )
    } else {
        return Err(failure("Invalid Flatpak ref"));
    };
    if !matches!(actual, "app" | "runtime")
        || kind.is_some_and(|k| k != actual)
        || fields.iter().any(|s| {
            s.is_empty()
                || s.len() > 4096
                || s.starts_with('-')
                || s.chars().any(|c| c.is_control() || c.is_whitespace())
        })
    {
        return Err(failure("Invalid Flatpak ref kind or components"));
    }
    Ok(format!(
        "{actual}/{}/{}/{}",
        fields[0], fields[1], fields[2]
    ))
}
fn installation_scope(display: &str, uid: &str) -> Result<(String, String)> {
    if display == "user" {
        return Ok((format!("user:{uid}"), "user".into()));
    }
    if display == "system" {
        return Ok(("system".into(), "system".into()));
    }
    // The native installation column formats named scopes as "system (NAME)".
    let name = if let Some(rest) = display.strip_prefix("system (") {
        rest.strip_suffix(')')
            .ok_or_else(|| failure("Malformed named Flatpak installation"))?
    } else {
        display
    };
    if name.is_empty() || name.len() > 4096 || name.chars().any(char::is_control) {
        return Err(failure("Invalid named Flatpak installation"));
    }
    Ok((format!("system:{name}"), name.into()))
}
fn scope_argument(scope: &str) -> Result<String> {
    if scope.starts_with("user:") {
        Ok("--user".into())
    } else if scope == "system" {
        Ok("--system".into())
    } else if let Some(name) = scope.strip_prefix("system:") {
        Ok(format!("--installation={name}"))
    } else {
        Err(failure("Invalid Flatpak scope"))
    }
}
fn normalize(row: Row, kind: &str, uid: &str, capabilities: ProviderCapabilities) -> Result<Entry> {
    let reference = canonical_ref(&row.reference, Some(kind))?;
    let fields: Vec<_> = reference.split('/').collect();
    if row.installation.is_empty() || row.installation.chars().any(char::is_control) {
        return Err(failure("Missing or invalid Flatpak installation"));
    }
    let (scope, installation) = installation_scope(&row.installation, uid)?;
    let id = PackageId {
        provider: "flatpak".into(),
        scope,
        native: reference.clone(),
    };
    let title = if row.name.is_empty() {
        fields[1].into()
    } else {
        row.name
    };
    let requires = if row.runtime.is_empty() {
        vec![]
    } else {
        vec![canonical_ref(&row.runtime, Some("runtime"))?]
    };
    let version = if row.version.is_empty() {
        fields[3].into()
    } else {
        row.version
    };
    let native_metadata = BTreeMap::from([
        ("flatpak.kind".into(), kind.into()),
        ("flatpak.branch".into(), fields[3].into()),
        ("flatpak.installation".into(), installation),
        ("flatpak.installation_display".into(), row.installation),
        ("flatpak.commit_abbreviated".into(), row.active_commit),
        ("flatpak.size_display".into(), row.installed_size),
    ]);
    Ok(Entry {
        package: PackageRecord {
            key: id.key(),
            id,
            name: fields[1].into(),
            title,
            description: row.description,
            version_display: version,
            arch: fields[2].into(),
            license: None,
            vendor: None,
            url: None,
            source_package: None,
            repository: (!row.origin.is_empty()).then_some(row.origin),
            installed_bytes: None,
            install_time: None,
            requires,
            provides: vec![reference.clone()],
            provenance: "Installed Flatpak refs (read-only native query)".into(),
            native_metadata,
        },
        category: if kind == "app" {
            "Applications"
        } else if fields[1].ends_with(".Sdk") || fields[1].ends_with(".Debug") {
            "Development"
        } else {
            "Libraries"
        }
        .into(),
        applications: vec![],
        services: vec![],
        capabilities,
    })
}
fn parse_details(text: &str) -> Result<(String, u64)> {
    let fields: Vec<_> = text.split_whitespace().collect();
    if fields.len() != 2
        || fields[0].len() != 64
        || !fields[0].bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err(failure("Invalid native Flatpak commit/size result"));
    }
    Ok((
        fields[0].into(),
        fields[1]
            .parse()
            .map_err(|_| failure("Invalid Flatpak installed byte size"))?,
    ))
}
fn dependencies(entries: &[Entry], id: &PackageId, reverse: bool) -> Result<DependencyReport> {
    let p = &entries
        .iter()
        .find(|e| e.package.id == *id)
        .ok_or_else(|| {
            ProviderError::new(
                ErrorKind::NotFound,
                "Flatpak ref is no longer installed in this scope",
            )
        })?
        .package;
    let candidates = if reverse {
        entries
            .iter()
            .filter(|e| {
                e.package.id != *id
                    && e.package.id.scope == id.scope
                    && e.package.requires.contains(&id.native)
            })
            .map(|e| e.package.id.clone())
            .collect()
    } else {
        vec![]
    };
    Ok(DependencyReport{package:id.clone(),requires:p.requires.clone(),provides:p.provides.clone(),reverse_candidates:candidates,reverse_queried:reverse,limitations:vec!["Flatpak relationships show declared app runtimes and same-installation reverse candidates. Extension attachment, SDK/build dependencies and cross-installation runtime fallback are not resolved. This is not an uninstall plan or removal-safety proof.".into()]})
}
impl PackageProvider for FlatpakProvider {
    fn id(&self) -> &str {
        "flatpak"
    }
    fn label(&self) -> &str {
        "Flatpak"
    }
    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities{inventory:true,metadata:true,dependencies:true,reverse_candidates:true,removal_preview:false,removal_execution:false,reasons:BTreeMap::from([("removal".into(),"Flatpak inspection is read-only in this increment; removal and permission changes are not implemented.".into())])}
    }
    fn inventory(&self) -> Result<Inventory> {
        let entries = self.records()?;
        // At most four read-only detail probes run concurrently. A scan-wide budget
        // keeps a large/broken installation from queuing unbounded per-ref work.
        let next = AtomicUsize::new(0);
        let results = Mutex::new(Vec::new());
        let start = Instant::now();
        std::thread::scope(|scope| {
            for _ in 0..4 {
                scope.spawn(|| loop {
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    let Some(original) = entries.get(index) else {
                        break;
                    };
                    let mut entry = original.clone();
                    let warning = if start.elapsed() < Duration::from_secs(20) {
                        self.details(&mut entry).err().map(|e| e.to_string())
                    } else {
                        Some("Flatpak detail scan budget reached".into())
                    };
                    results.lock().unwrap().push((entry, warning));
                });
            }
        });
        let mut results = results.into_inner().unwrap();
        results.sort_by_key(|(e, _)| e.package.identity());
        let unknown = results.iter().filter(|(_, w)| w.is_some()).count();
        let mut warnings = vec![];
        if unknown > 0 {
            warnings.push(format!("{unknown} Flatpak ref(s) have unavailable commit/byte-size details. Their installed refs remain listed; totals include only known footprints."));
        }
        if unknown > 0 {
            let reasons: BTreeSet<_> = results
                .iter()
                .filter_map(|(_, warning)| warning.as_deref())
                .collect();
            warnings.extend(
                reasons
                    .into_iter()
                    .take(3)
                    .map(|message| format!("Flatpak detail query: {message}")),
            );
        }
        Ok(Inventory {
            entries: results.into_iter().map(|(e, _)| e).collect(),
            warnings,
            source: "Live installed Flatpak apps/runtimes (user and system installations)".into(),
            provider_label: self.label().into(),
            capabilities: self.capabilities(),
        })
    }
    fn metadata(&self, id: &PackageId) -> Result<PackageRecord> {
        self.checked(id)?;
        let mut entry = self
            .records()?
            .into_iter()
            .find(|e| e.package.id == *id)
            .ok_or_else(|| {
                ProviderError::new(
                    ErrorKind::NotFound,
                    "Flatpak ref changed installation or was removed",
                )
            })?;
        self.details(&mut entry)?;
        Ok(entry.package)
    }
    fn dependencies(&self, id: &PackageId, reverse: bool) -> Result<DependencyReport> {
        self.checked(id)?;
        dependencies(&self.records()?, id, reverse)
    }
    // No mutation implementation: the shared unsupported default is intentional.
}

#[cfg(test)]
mod tests {
    use super::*;
    fn provider() -> FlatpakProvider {
        FlatpakProvider { uid: "1000".into() }
    }
    fn fixture() -> Vec<Entry> {
        let mut entries = vec![];
        for (kind, text) in [
            (
                "app",
                include_str!("../../../../fixtures/flatpak/apps.json"),
            ),
            (
                "runtime",
                include_str!("../../../../fixtures/flatpak/runtimes.json"),
            ),
        ] {
            entries.extend(
                parse_json(text)
                    .unwrap()
                    .into_iter()
                    .map(|r| normalize(r, kind, "1000", provider().capabilities()).unwrap()),
            );
        }
        entries
    }
    #[test]
    fn scopes_kinds_branches_architectures_and_opaque_versions_are_distinct() {
        let entries = fixture();
        assert_eq!(entries.len(), 9);
        assert_eq!(
            entries
                .iter()
                .map(|e| e.package.key.clone())
                .collect::<BTreeSet<_>>()
                .len(),
            9
        );
        let user = &entries[0].package;
        let system = &entries[1].package;
        let extra = &entries[2].package;
        assert_ne!(user.id, system.id);
        assert_eq!(user.version_display, "2.1~rc1");
        assert_eq!(extra.id.scope, "system:extra");
        assert_eq!(extra.arch, "aarch64");
        assert_eq!(extra.version_display, "beta");
        assert!(user.description.contains("<plain text> &"));
        assert!(user.installed_bytes.is_none());
        assert!(user.license.is_none());
        assert!(extra.repository.is_none());
        assert!(user.source_package.is_none());
        assert_eq!(entries[6].category, "Libraries");
        assert_eq!(entries[7].category, "Development");
        assert_eq!(entries[8].category, "Development");
    }
    #[test]
    fn runtime_relationships_never_cross_installation_scopes() {
        let entries = fixture();
        let runtime = &entries[3].package.id;
        let report = dependencies(&entries, runtime, true).unwrap();
        assert_eq!(report.reverse_candidates.len(), 1);
        assert_eq!(report.reverse_candidates[0], entries[0].package.id);
        assert!(report.limitations[0].contains("cross-installation"));
        let report = dependencies(&entries, &entries[0].package.id, false).unwrap();
        assert_eq!(
            report.requires,
            vec!["runtime/org.example.Platform/x86_64/24.08"]
        );
        assert!(!report.reverse_queried);
    }
    #[test]
    fn old_full_column_tsv_preserves_empty_and_trailing_fields() {
        let text="org.example.App/x86_64/stable\tuser\tName\t\t\tremote\torg.example.Platform/x86_64/stable\taaaaaaaaaaaa\t1.2 MB\norg.example.Empty/x86_64/stable\tsystem\tEmpty\n";
        let rows = parse_tsv(text).unwrap();
        assert_eq!(rows.len(), 2);
        assert!(rows[0].version.is_empty());
        assert_eq!(rows[0].runtime, "org.example.Platform/x86_64/stable");
        assert!(rows[1].origin.is_empty());
        assert!(parse_tsv("not a tab-separated row").is_err());
        assert!(parse_json("{}").is_err());
    }
    #[test]
    fn malformed_refs_kind_mismatches_and_missing_scopes_fail() {
        for value in [
            "bad",
            "app/id/arch",
            "app/id//branch",
            "app/id/arch/bad branch",
            "runtime/-option/arch/stable",
        ] {
            assert!(canonical_ref(value, None).is_err(), "{value}");
        }
        assert!(canonical_ref("runtime/org.example.App/x86_64/stable", Some("app")).is_err());
        let mut row: Row =
            serde_json::from_str(r#"{"ref":"org.example.App/x86_64/stable","installation":""}"#)
                .unwrap();
        assert!(normalize(row, "app", "1000", provider().capabilities()).is_err());
        row=serde_json::from_str(r#"{"ref":"org.example.App/x86_64/stable","installation":"user","runtime":"malformed"}"#).unwrap();
        assert!(normalize(row, "app", "1000", provider().capabilities()).is_err());
    }
    #[test]
    fn exact_byte_sizes_require_a_full_commit_and_never_parse_human_rounding() {
        let text = format!("{} 1234567\n", "a".repeat(64));
        assert_eq!(parse_details(&text).unwrap().1, 1234567);
        for text in [
            "aaaaaaaaaaaa 123",
            "not-a-commit 1",
            "a 1.2 MB",
            &format!("{} 18446744073709551616", "a".repeat(64)),
        ] {
            assert!(parse_details(text).is_err());
        }
    }
    #[test]
    fn readonly_operations_and_foreign_users_fail_without_native_queries() {
        let p = provider();
        assert!(!p.capabilities().removal_preview);
        assert!(!p.capabilities().removal_execution);
        let mut id = fixture()[0].package.id.clone();
        assert!(matches!(
            p.prepare_removal(&id),
            Err(ProviderError {
                kind: ErrorKind::Unsupported,
                ..
            })
        ));
        id.scope = "user:9999".into();
        assert_eq!(p.metadata(&id).unwrap_err().kind, ErrorKind::WrongProvider);
        id.provider = "rpm".into();
        assert_eq!(
            p.dependencies(&id, true).unwrap_err().kind,
            ErrorKind::WrongProvider
        );
    }
}

#[cfg(test)]
mod named_scope_tests {
    use super::*;
    #[test]
    fn native_named_labels_do_not_confuse_reserved_user_or_system_names() {
        for name in ["extra", "user", "system", "disk with spaces"] {
            let (scope, actual) = installation_scope(&format!("system ({name})"), "1000").unwrap();
            assert_eq!(actual, name);
            assert_eq!(
                scope_argument(&scope).unwrap(),
                format!("--installation={name}")
            );
        }
        assert_eq!(
            scope_argument(&installation_scope("user", "1000").unwrap().0).unwrap(),
            "--user"
        );
        assert_eq!(
            scope_argument(&installation_scope("system", "1000").unwrap().0).unwrap(),
            "--system"
        );
        assert!(installation_scope("system (broken", "1000").is_err());
    }
}
