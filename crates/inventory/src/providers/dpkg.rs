//! Read-only installed dpkg database provider. No apt/dpkg mutation commands exist here.
use super::*;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

pub(super) struct DpkgProvider {
    root: PathBuf,
    scope: String,
}
fn operation(message: impl Into<String>) -> ProviderError {
    ProviderError::new(ErrorKind::OperationFailed, message)
}
impl DpkgProvider {
    pub(super) fn system() -> Self {
        Self {
            root: PathBuf::from("/"),
            scope: "system".into(),
        }
    }
    pub(super) fn offline(root: &Path) -> Result<Self> {
        let root = root
            .canonicalize()
            .map_err(|e| operation(format!("Cannot open inspection root: {e}")))?;
        let status = root
            .join("var/lib/dpkg/status")
            .canonicalize()
            .map_err(|e| operation(format!("No readable dpkg database: {e}")))?;
        if !status.starts_with(&root) {
            return Err(operation(
                "The dpkg database escapes the requested inspection root",
            ));
        }
        let scope = format!("root:{}", root.display());
        Ok(Self { root, scope })
    }
    fn checked(&self, id: &PackageId) -> Result<()> {
        if id.provider != "dpkg" || id.scope != self.scope {
            return Err(ProviderError::new(
                ErrorKind::WrongProvider,
                "This dpkg provider cannot inspect another source or root",
            ));
        }
        if id.native.is_empty() {
            return Err(ProviderError::new(
                ErrorKind::NotFound,
                "Missing native package identity",
            ));
        }
        Ok(())
    }
    fn query(&self, args: &[&str]) -> Result<(i32, String, String)> {
        // Explicit admindir and pager settings prevent inherited root/pager overrides.
        let mut child = Command::new("/usr/bin/dpkg-query")
            .arg(format!(
                "--admindir={}",
                self.root.join("var/lib/dpkg").display()
            ))
            .arg("--no-pager")
            .args(args)
            .env("LC_ALL", "C.UTF-8")
            .env("DPKG_COLORS", "never")
            .env_remove("DPKG_ROOT")
            .env_remove("DPKG_ADMINDIR")
            .env_remove("DPKG_PAGER")
            .env_remove("PAGER")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| operation(format!("Could not run dpkg-query: {e}")))?;
        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();
        let out_reader = std::thread::spawn(move || {
            let mut v = Vec::new();
            let r = stdout.take(128 * 1024 * 1024 + 1).read_to_end(&mut v);
            (r, v)
        });
        let err_reader = std::thread::spawn(move || {
            let mut v = Vec::new();
            let r = stderr.take(1024 * 1024 + 1).read_to_end(&mut v);
            (r, v)
        });
        let start = Instant::now();
        let status = loop {
            match child.try_wait() {
                Ok(Some(s)) => break Some(s),
                Ok(None) if start.elapsed() < Duration::from_secs(20) => {
                    std::thread::sleep(Duration::from_millis(10))
                }
                _ => {
                    let _ = child.kill();
                    let _ = child.wait();
                    break None;
                }
            }
        };
        let (r, out) = out_reader
            .join()
            .map_err(|_| operation("dpkg stdout reader stopped"))?;
        r.map_err(|e| operation(e.to_string()))?;
        let (r, err) = err_reader
            .join()
            .map_err(|_| operation("dpkg stderr reader stopped"))?;
        r.map_err(|e| operation(e.to_string()))?;
        if out.len() > 128 * 1024 * 1024 || err.len() > 1024 * 1024 {
            return Err(operation(
                "dpkg-query output exceeded the read-only query limit",
            ));
        }
        let code = status
            .and_then(|s| s.code())
            .ok_or_else(|| operation("dpkg-query timed out or terminated"))?;
        Ok((
            code,
            String::from_utf8(out).map_err(|_| operation("dpkg returned non-UTF-8 output"))?,
            String::from_utf8_lossy(&err).trim().into(),
        ))
    }
    fn records(&self) -> Result<Vec<PackageRecord>> {
        if !self.root.join("var/lib/dpkg/status").is_file() {
            return Err(operation("No readable dpkg status database is available"));
        }
        let (code, text, err) = self.query(&["--status"])?;
        if code != 0 {
            return Err(operation(format!(
                "dpkg-query could not read the installed database: {err}"
            )));
        }
        let mut records = parse_status(&text, &self.scope)?;
        if self.scope != "system" {
            for p in &mut records {
                p.provenance = "Offline root dpkg database (read-only; not the live host)".into();
            }
        }
        Ok(records)
    }
    fn associations(&self, entries: &mut [Entry]) -> Vec<String> {
        let result = self.query(&[
            "--search",
            "/usr/share/applications/*.desktop",
            "/lib/systemd/system/*.service",
            "/usr/lib/systemd/system/*.service",
            "/lib/systemd/user/*.service",
            "/usr/lib/systemd/user/*.service",
        ]);
        let (code, text, stderr) = match result {
            Ok(r) => r,
            Err(e) => return vec![format!("dpkg ownership discovery unavailable: {e}")],
        };
        let unexpected = stderr
            .lines()
            .filter(|l| !l.starts_with("dpkg-query: no path found matching pattern "))
            .collect::<Vec<_>>();
        let mut warnings = vec![];
        if code > 1 || !unexpected.is_empty() {
            warnings.push(format!(
                "dpkg ownership discovery incomplete: {}",
                unexpected.join("\n")
            ));
            if code > 1 {
                return warnings;
            }
        }
        let mut by_owner: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for (i, e) in entries.iter().enumerate() {
            by_owner.entry(e.package.name.clone()).or_default().push(i);
            by_owner
                .entry(format!("{}:{}", e.package.name, e.package.arch))
                .or_default()
                .push(i);
        }
        let mut skipped_diversion = false;
        for line in text.lines() {
            if line.starts_with("diversion ") || line.starts_with("local diversion ") {
                skipped_diversion = true;
                continue;
            }
            let Some((owners, path)) = line.split_once(": ") else {
                continue;
            };
            if !path.starts_with('/') {
                continue;
            }
            let app = super::catalog::application_name(&self.root, path);
            let service = service_file(&self.root, path);
            if app.is_none() && service.is_none() {
                continue;
            }
            for owner in owners.split(", ") {
                if let Some(indices) = by_owner.get(owner) {
                    for &i in indices {
                        if let Some(name) = &app {
                            if !entries[i]
                                .applications
                                .iter()
                                .any(|a| a.desktop_file == path)
                            {
                                entries[i].applications.push(Application {
                                    name: name.clone(),
                                    desktop_file: path.into(),
                                });
                            }
                        }
                        if let Some(unit) = &service {
                            if !entries[i].services.contains(unit) {
                                entries[i].services.push(unit.clone());
                            }
                        }
                    }
                }
            }
        }
        for e in entries {
            e.category = category(&e.package, !e.applications.is_empty());
        }
        if skipped_diversion {
            warnings.push(
                "Diverted paths were reported by dpkg; launcher associations may be incomplete."
                    .into(),
            );
        }
        warnings
    }
}
fn service_file(root: &Path, path: &str) -> Option<Service> {
    let scope =
        if path.starts_with("/lib/systemd/user/") || path.starts_with("/usr/lib/systemd/user/") {
            "user"
        } else if path.starts_with("/lib/systemd/system/")
            || path.starts_with("/usr/lib/systemd/system/")
        {
            "system"
        } else {
            return None;
        };
    let unit = Path::new(path).file_name()?.to_str()?;
    if unit.contains('@') || !unit.ends_with(".service") {
        return None;
    }
    let p = super::catalog::rooted_file(root, path)?;
    if !fs::symlink_metadata(p).ok()?.file_type().is_file() {
        return None;
    }
    Some(Service {
        unit: unit.into(),
        scope: scope.into(),
    })
}
fn category(p: &PackageRecord, application: bool) -> String {
    let section = p
        .native_metadata
        .get("dpkg.section")
        .map(String::as_str)
        .unwrap_or("")
        .rsplit('/')
        .next()
        .unwrap_or("");
    if application {
        "Applications"
    } else if p.name.ends_with("-dev") || matches!(section, "libdevel" | "devel") {
        "Development"
    } else if section == "libs" {
        "Libraries"
    } else {
        "Packages"
    }
    .into()
}
fn valid_name(s: &str) -> bool {
    s.len() >= 2
        && s.bytes()
            .next()
            .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        && s.bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || b"+.-".contains(&c))
}
fn fields(text: &str) -> Result<Vec<BTreeMap<String, String>>> {
    let mut result = vec![];
    let mut record = BTreeMap::<String, String>::new();
    let mut previous = String::new();
    for (i, line) in text.lines().enumerate() {
        let line = line.trim_end_matches('\r');
        if line.is_empty() {
            if !record.is_empty() {
                result.push(std::mem::take(&mut record));
            }
            previous.clear();
            continue;
        }
        if line.starts_with(' ') || line.starts_with('\t') {
            let Some(value) = record.get_mut(&previous) else {
                return Err(operation(format!(
                    "Invalid control continuation at line {}",
                    i + 1
                )));
            };
            value.push('\n');
            value.push_str(&line[1..]);
            continue;
        }
        let Some((key, value)) = line.split_once(':') else {
            return Err(operation(format!(
                "Invalid control field at line {}",
                i + 1
            )));
        };
        if key.is_empty() || !key.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-') {
            return Err(operation("Invalid control field name"));
        }
        let key = key.to_ascii_lowercase();
        if record.contains_key(&key) {
            return Err(operation(format!("Duplicate dpkg control field: {key}")));
        }
        record.insert(key.clone(), value.trim_start().into());
        previous = key;
    }
    if !record.is_empty() {
        result.push(record);
    }
    Ok(result)
}
fn optional(fields: &BTreeMap<String, String>, key: &str) -> Option<String> {
    fields.get(key).filter(|s| !s.is_empty()).cloned()
}
fn clauses(s: &str) -> Vec<String> {
    s.replace('\n', " ")
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}
/// Package alternatives/qualifiers are preserved verbatim for display. This name
/// extraction is deliberately only for reverse candidates, never solver decisions.
fn names(clause: &str) -> BTreeSet<String> {
    clause
        .split('|')
        .filter_map(|alt| {
            let token = alt
                .trim()
                .split(|c: char| c.is_whitespace() || c == '(' || c == '[' || c == '<')
                .next()?;
            let base = token.split(':').next()?;
            valid_name(base).then(|| base.into())
        })
        .collect()
}
pub(super) fn parse_status(input: &str, scope: &str) -> Result<Vec<PackageRecord>> {
    let mut result = vec![];
    let mut seen = BTreeSet::new();
    for f in fields(input)? {
        let status = f
            .get("status")
            .ok_or_else(|| operation("dpkg status record has no Status field"))?;
        let parts: Vec<_> = status.split_whitespace().collect();
        if parts.len() != 3 {
            return Err(operation("Invalid dpkg package status"));
        }
        let state = parts[2];
        if matches!(state, "not-installed" | "config-files") {
            continue;
        }
        if !matches!(
            state,
            "installed"
                | "unpacked"
                | "half-configured"
                | "half-installed"
                | "triggers-awaited"
                | "triggers-pending"
        ) {
            return Err(operation(format!("Unknown dpkg state: {state}")));
        }
        let name = optional(&f, "package")
            .ok_or_else(|| operation("Present dpkg package is missing its name"))?;
        let arch = optional(&f, "architecture")
            .ok_or_else(|| operation("Present dpkg package is missing its architecture"))?;
        let version = optional(&f, "version")
            .ok_or_else(|| operation("Present dpkg package is missing its version"))?;
        if !valid_name(&name) || !arch.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-') {
            return Err(operation("Invalid dpkg package identity"));
        }
        let id = PackageId {
            provider: "dpkg".into(),
            scope: scope.into(),
            native: format!("{name}:{arch}={version}"),
        };
        if !seen.insert((name.clone(), arch.clone())) {
            return Err(operation("Duplicate installed dpkg identity"));
        }
        let installed_bytes = match optional(&f, "installed-size") {
            Some(s) => Some(
                s.parse::<u64>()
                    .ok()
                    .and_then(|v| v.checked_mul(1024))
                    .ok_or_else(|| operation("Invalid or overflowing Installed-Size"))?,
            ),
            None => None,
        };
        let raw_description = f.get("description").cloned().unwrap_or_default();
        let title = raw_description
            .lines()
            .next()
            .filter(|s| !s.is_empty())
            .unwrap_or(&name)
            .to_string();
        let description = raw_description
            .lines()
            .map(|l| if l == "." { "" } else { l })
            .collect::<Vec<_>>()
            .join("\n");
        let mut requires = f.get("pre-depends").map(|s| clauses(s)).unwrap_or_default();
        requires.extend(f.get("depends").map(|s| clauses(s)).unwrap_or_default());
        let provides = f.get("provides").map(|s| clauses(s)).unwrap_or_default();
        let mut native_metadata = BTreeMap::new();
        for (key, value) in &f {
            if key != "description" {
                native_metadata.insert(format!("dpkg.{key}"), value.clone());
            }
        }
        result.push(PackageRecord {
            key: id.key(),
            id,
            name: name.clone(),
            title,
            description,
            version_display: version,
            arch,
            license: None,
            vendor: optional(&f, "origin"),
            url: optional(&f, "homepage"),
            source_package: Some(
                f.get("source")
                    .and_then(|s| s.split_whitespace().next())
                    .unwrap_or(&name)
                    .into(),
            ),
            repository: None,
            installed_bytes,
            install_time: None,
            requires,
            provides,
            provenance: "Installed dpkg database (read-only query)".into(),
            native_metadata,
        });
    }
    result.sort_by_key(PackageRecord::identity);
    Ok(result)
}
fn dependency_report(
    records: &[PackageRecord],
    id: &PackageId,
    include_reverse: bool,
) -> Result<DependencyReport> {
    let p = records.iter().find(|p| p.id == *id).ok_or_else(|| {
        ProviderError::new(
            ErrorKind::NotFound,
            "Package changed or is no longer present",
        )
    })?;
    let mut aliases = BTreeSet::from([p.name.clone()]);
    for provide in &p.provides {
        aliases.extend(names(provide));
    }
    let reverse = if include_reverse {
        records
            .iter()
            .filter(|candidate| {
                candidate.id != *id
                    && candidate
                        .requires
                        .iter()
                        .any(|clause| !names(clause).is_disjoint(&aliases))
            })
            .map(|p| p.id.clone())
            .collect()
    } else {
        vec![]
    };
    Ok(DependencyReport {
        package: id.clone(),
        requires: p.requires.clone(),
        provides: p.provides.clone(),
        reverse_candidates: reverse,
        reverse_queried: include_reverse,
        limitations: vec![
            "Required relationships include Depends and Pre-Depends. Recommends/Suggests remain native metadata.".into(),
            "Reverse candidates match installed dependency names and virtual Provides. Version constraints, alternatives and architecture qualifiers are not solver-resolved; results are not removal-safety evidence.".into(),
        ],
    })
}
impl PackageProvider for DpkgProvider {
    fn id(&self) -> &str {
        "dpkg"
    }
    fn label(&self) -> &str {
        "dpkg"
    }
    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            inventory: true,
            metadata: true,
            dependencies: true,
            reverse_candidates: true,
            removal_preview: false,
            removal_execution: false,
            reasons: BTreeMap::from([(
                "removal".into(),
                "APT/dpkg inspection is read-only in Milestone 2; removal is not implemented."
                    .into(),
            )]),
        }
    }
    fn inventory(&self) -> Result<Inventory> {
        let records = self.records()?;
        let incomplete = records
            .iter()
            .filter(|p| {
                p.native_metadata
                    .get("dpkg.status")
                    .is_some_and(|s| !s.ends_with("ok installed"))
            })
            .count();
        let mut entries: Vec<_> = records
            .into_iter()
            .map(|p| Entry {
                category: category(&p, false),
                capabilities: self.capabilities(),
                package: p,
                applications: vec![],
                services: vec![],
            })
            .collect();
        let mut warnings = self.associations(&mut entries);
        if incomplete > 0 {
            warnings.push(format!("{incomplete} present package(s) are not in a fully configured, error-free installed state; their dpkg status is preserved."));
        }
        Ok(Inventory {
            entries,
            warnings,
            source: if self.scope == "system" {
                "Live installed dpkg database + native ownership query"
            } else {
                "Offline root dpkg database + native ownership query"
            }
            .into(),
            provider_label: self.label().into(),
            capabilities: self.capabilities(),
        })
    }
    fn metadata(&self, id: &PackageId) -> Result<PackageRecord> {
        self.checked(id)?;
        self.records()?
            .into_iter()
            .find(|p| p.id == *id)
            .ok_or_else(|| {
                ProviderError::new(
                    ErrorKind::NotFound,
                    "Package changed or is no longer present; refresh the inventory",
                )
            })
    }
    fn dependencies(&self, id: &PackageId, include_reverse: bool) -> Result<DependencyReport> {
        self.checked(id)?;
        dependency_report(&self.records()?, id, include_reverse)
    }

    // prepare_removal deliberately uses the unsupported default; there is no execution path.
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> &'static str {
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/dpkg/status"
        ))
    }
    #[test]
    fn status_preserves_debian_metadata_multilib_and_partial_state() {
        let p = parse_status(fixture(), "system").unwrap();
        assert_eq!(p.len(), 5);
        let lib = p
            .iter()
            .find(|p| p.name == "sample-lib" && p.arch == "amd64")
            .unwrap();
        assert_eq!(lib.version_display, "1:2.0~rc1-3+b1");
        assert_eq!(lib.source_package.as_deref(), Some("sample-source"));
        assert_eq!(lib.installed_bytes, Some(123 * 1024));
        assert!(lib.description.contains("\n\nSecond paragraph."));
        assert!(lib.description.contains("<plain text> &"));
        assert!(lib.repository.is_none());
        assert!(lib.license.is_none());
        assert!(lib.install_time.is_none());
        assert_eq!(
            lib.native_metadata["dpkg.maintainer"],
            "Example Maintainer <example@example.invalid>"
        );
        let lib32 = p
            .iter()
            .find(|p| p.name == "sample-lib" && p.arch == "i386")
            .unwrap();
        assert_ne!(lib.key, lib32.key);
        assert!(!p.iter().any(|p| p.name == "removed-tool"));
        assert!(p.iter().any(|p| p.name == "damaged-tool"));
        let dev = p.iter().find(|p| p.name == "sample-dev").unwrap();
        assert_eq!(dev.installed_bytes, Some(0));
        assert_eq!(dev.requires[0], "base-runtime (>= 1.0)");
        assert!(dev.requires[2].contains(" | "));
        assert_eq!(category(dev, false), "Development");
    }
    #[test]
    fn reverse_candidates_include_alternatives_and_virtual_provides_but_are_not_a_solver() {
        let p = parse_status(fixture(), "system").unwrap();
        let lib = p
            .iter()
            .find(|p| p.name == "sample-lib" && p.arch == "amd64")
            .unwrap();
        let r = dependency_report(&p, &lib.id, true).unwrap();
        assert!(r.reverse_queried);
        assert!(r
            .reverse_candidates
            .iter()
            .any(|id| id.native.starts_with("sample-dev:")));
        assert!(r
            .reverse_candidates
            .iter()
            .any(|id| id.native.starts_with("sample-app:")));
        assert!(r
            .limitations
            .iter()
            .any(|s| s.contains("not solver-resolved")));
        let no_reverse = dependency_report(&p, &lib.id, false).unwrap();
        assert!(!no_reverse.reverse_queried);
        assert!(no_reverse.reverse_candidates.is_empty());
    }
    #[test]
    fn malformed_present_records_fail_instead_of_becoming_a_zero_inventory() {
        for text in [
            "Package: sample\nStatus: install ok installed\n",
            "Package: sample\nStatus: surprising\n",
            " continuation without field",
            "Package: sample\nPackage: duplicate\n",
        ] {
            assert!(parse_status(text, "system").is_err());
        }
        let bad = fixture().replace(
            "Installed-Size: 123",
            "Installed-Size: 18446744073709551615",
        );
        assert!(parse_status(&bad, "system").is_err());
        let duplicate = format!("{}\n{}", fixture(), fixture());
        assert!(parse_status(&duplicate, "system").is_err());
    }
    #[test]
    fn scope_and_source_mismatch_and_mutations_fail_before_queries() {
        let provider = DpkgProvider {
            root: PathBuf::from("/not-a-root"),
            scope: "system".into(),
        };
        let foreign = PackageId {
            provider: "rpm".into(),
            scope: "system".into(),
            native: "sample".into(),
        };
        assert_eq!(
            provider.metadata(&foreign).unwrap_err().kind,
            ErrorKind::WrongProvider
        );
        let other_scope = PackageId {
            provider: "dpkg".into(),
            scope: "root:/another".into(),
            native: "sample".into(),
        };
        assert_eq!(
            provider.metadata(&other_scope).unwrap_err().kind,
            ErrorKind::WrongProvider
        );
        assert!(!provider.capabilities().removal_execution);
        assert!(provider
            .inventory()
            .unwrap_err()
            .message
            .contains("status database"));
        assert!(matches!(
            provider.prepare_removal(&other_scope),
            Err(ProviderError {
                kind: ErrorKind::Unsupported,
                ..
            })
        ));
    }
    #[test]
    fn relationship_names_ignore_constraints_only_for_candidate_lookup() {
        assert_eq!(
            names("sample-lib:any (>= 1) | alt-package [amd64]"),
            BTreeSet::from(["sample-lib".into(), "alt-package".into()])
        );
        assert!(names("$(unsafe)").is_empty());
    }
}
