//! Read-only local ALPM database inspection. No pacman transaction is invoked.
use super::*;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Read,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
pub(super) struct PacmanProvider {
    root: PathBuf,
    scope: String,
}
fn error(message: impl Into<String>) -> ProviderError {
    ProviderError::new(ErrorKind::OperationFailed, message)
}
impl PacmanProvider {
    pub(super) fn system() -> Self {
        Self {
            root: "/".into(),
            scope: "system".into(),
        }
    }
    pub(super) fn offline(root: &Path) -> Result<Self> {
        let root = root.canonicalize().map_err(|e| error(e.to_string()))?;
        Ok(Self {
            scope: format!("root:{}", root.display()),
            root,
        })
    }
    fn read(&self, path: &Path, limit: u64) -> Result<String> {
        let actual = path
            .canonicalize()
            .map_err(|e| error(format!("Cannot read ALPM database file: {e}")))?;
        if !actual.starts_with(&self.root) || !actual.is_file() {
            return Err(error("ALPM database file escapes the selected root"));
        }
        let mut bytes = vec![];
        fs::File::open(&actual)
            .map_err(|e| error(e.to_string()))?
            .take(limit + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| error(e.to_string()))?;
        if bytes.len() as u64 > limit {
            return Err(error("ALPM database file exceeds inspection limit"));
        }
        String::from_utf8(bytes).map_err(|_| error("ALPM metadata is not UTF-8"))
    }
    fn checked(&self, id: &PackageId) -> Result<()> {
        if id.provider != "pacman" || id.scope != self.scope {
            return Err(ProviderError::new(
                ErrorKind::WrongProvider,
                "Pacman source/root does not match this provider",
            ));
        }
        Ok(())
    }
    fn entries(&self) -> Result<(Vec<Entry>, Vec<String>)> {
        let db = self.root.join("var/lib/pacman/local");
        let lock = self.root.join("var/lib/pacman/db.lck");
        if lock.exists() {
            return Err(error(
                "Pacman database is locked; scan after the native transaction finishes",
            ));
        }
        if self.read(&db.join("ALPM_DB_VERSION"), 32)?.trim() != "9" {
            return Err(error("Unsupported ALPM database layout version"));
        }
        let start = Instant::now();
        let mut entries = vec![];
        let mut seen = BTreeSet::new();
        let mut warnings = vec![];
        for item in fs::read_dir(&db).map_err(|e| error(e.to_string()))? {
            if start.elapsed() > Duration::from_secs(30) || entries.len() > 100_000 {
                return Err(error("ALPM inventory exceeded its scan limit"));
            }
            let path = item.map_err(|e| error(e.to_string()))?.path();
            if path.file_name().is_some_and(|n| n == "ALPM_DB_VERSION") {
                continue;
            }
            if !fs::symlink_metadata(&path)
                .map_err(|e| error(e.to_string()))?
                .file_type()
                .is_dir()
            {
                return Err(error("Unexpected or linked item in ALPM package database"));
            }
            let fields = parse_fields(&self.read(&path.join("desc"), 1024 * 1024)?)?;
            let package = record(&fields, &self.scope)?;
            if !seen.insert(package.name.clone()) {
                return Err(error("Duplicate installed ALPM package name"));
            }
            let mut entry = Entry {
                category: if package.name.ends_with("-headers")
                    || package.name.ends_with("-debug")
                    || fields
                        .get("GROUPS")
                        .is_some_and(|g| g.iter().any(|v| v == "base-devel"))
                {
                    "Development"
                } else if package.provides.iter().any(|p| p.contains(".so")) {
                    "Libraries"
                } else {
                    "Packages"
                }
                .into(),
                package,
                applications: vec![],
                services: vec![],
                capabilities: self.capabilities(),
            };
            match self
                .read(&path.join("files"), 64 * 1024 * 1024)
                .and_then(|text| parse_fields(&text))
            {
                Ok(files) => {
                    for file in files.get("FILES").into_iter().flatten() {
                        if file.starts_with('/') || file.split('/').any(|s| s == "..") {
                            warnings.push("ALPM ownership contains unsafe paths; those associations were omitted".into());
                            continue;
                        }
                        let native = format!("/{file}");
                        if let Some(name) = super::catalog::application_name(&self.root, &native) {
                            entry.applications.push(Application {
                                name,
                                desktop_file: native.clone(),
                            });
                        }
                        for scope in ["system", "user"] {
                            let prefix = format!("usr/lib/systemd/{scope}/");
                            if let Some(unit) = file.strip_prefix(&prefix) {
                                if crate::services::valid_unit(unit).is_ok()
                                    && super::catalog::rooted_file(&self.root, &native).is_some()
                                {
                                    entry.services.push(Service {
                                        unit: unit.into(),
                                        scope: scope.into(),
                                    });
                                }
                            }
                        }
                    }
                }
                Err(e) => warnings.push(format!(
                    "ALPM ownership unavailable for {}: {e}",
                    entry.package.name
                )),
            }
            if !entry.applications.is_empty() {
                entry.category = "Applications".into();
            }
            entries.push(entry);
        }
        if lock.exists() {
            return Err(error(
                "Pacman transaction began during discovery; scan again",
            ));
        }
        entries.sort_by_key(|e| e.package.identity());
        warnings.sort();
        warnings.dedup();
        Ok((entries, warnings))
    }
}
fn parse_fields(text: &str) -> Result<BTreeMap<String, Vec<String>>> {
    let mut result = BTreeMap::new();
    let mut key = None;
    for line in text.lines() {
        let line = line.trim_end_matches('\r');
        if line.is_empty() {
            continue;
        }
        if line.starts_with('%') && line.ends_with('%') {
            if line.len() < 3 {
                return Err(error("Invalid ALPM field header"));
            }
            let name = &line[1..line.len() - 1];
            if name.is_empty()
                || !name.bytes().all(|b| b.is_ascii_uppercase() || b == b'_')
                || result.contains_key(name)
            {
                return Err(error("Invalid or duplicated ALPM field"));
            }
            result.insert(name.into(), vec![]);
            key = Some(name.to_string());
        } else {
            let name = key
                .as_ref()
                .ok_or_else(|| error("ALPM value without a field"))?;
            result.get_mut(name).unwrap().push(line.to_string());
        }
    }
    Ok(result)
}
fn record(f: &BTreeMap<String, Vec<String>>, scope: &str) -> Result<PackageRecord> {
    let one = |key: &str| -> Result<Option<String>> {
        match f.get(key) {
            None => Ok(None),
            Some(v) if v.is_empty() => Ok(None),
            Some(v) if v.len() == 1 => Ok(Some(v[0].clone())),
            _ => Err(error(format!("ALPM {key} must have one value"))),
        }
    };
    let name = one("NAME")?.ok_or_else(|| error("ALPM record has no NAME"))?;
    let version = one("VERSION")?.ok_or_else(|| error("ALPM record has no VERSION"))?;
    let arch = one("ARCH")?.ok_or_else(|| error("ALPM record has no ARCH"))?;
    if name.is_empty()
        || version.is_empty()
        || arch.is_empty()
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"@._+-".contains(&b))
        || name.starts_with('-')
        || !arch.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
    {
        return Err(error("Invalid ALPM native identity"));
    }
    let number = |key: &str| -> Result<Option<u64>> {
        one(key)?
            .map(|s| s.parse().map_err(|_| error(format!("Invalid ALPM {key}"))))
            .transpose()
    };
    if one("REASON")?.is_some_and(|v| v != "0" && v != "1") {
        return Err(error("Unknown ALPM installation reason"));
    }
    let description = one("DESC")?.unwrap_or_default();
    let id = PackageId {
        provider: "pacman".into(),
        scope: scope.into(),
        native: format!("{name}:{arch}={version}"),
    };
    Ok(PackageRecord {
        key: id.key(),
        id,
        name: name.clone(),
        title: if description.is_empty() {
            name
        } else {
            description.clone()
        },
        description,
        version_display: version,
        arch,
        license: f
            .get("LICENSE")
            .filter(|v| !v.is_empty())
            .map(|v| v.join(", ")),
        vendor: None,
        url: one("URL")?,
        source_package: one("BASE")?,
        repository: None,
        installed_bytes: number("SIZE")?,
        install_time: number("INSTALLDATE")?,
        requires: f.get("DEPENDS").cloned().unwrap_or_default(),
        provides: f.get("PROVIDES").cloned().unwrap_or_default(),
        provenance: if scope == "system" {
            "Installed local ALPM database (read-only)"
        } else {
            "Offline root ALPM database (read-only; not the live host)"
        }
        .into(),
        native_metadata: f
            .iter()
            .map(|(k, v)| (format!("pacman.{}", k.to_ascii_lowercase()), v.join("\n")))
            .collect(),
    })
}
fn relation_name(value: &str) -> &str {
    value.split(['<', '>', '=']).next().unwrap_or("")
}
impl PackageProvider for PacmanProvider {
    fn id(&self) -> &str {
        "pacman"
    }
    fn label(&self) -> &str {
        "pacman"
    }
    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            inventory: true,
            metadata: true,
            dependencies: true,
            reverse_candidates: true,
            reasons: BTreeMap::from([(
                "removal".into(),
                "Pacman inspection is read-only; native transaction management is not implemented."
                    .into(),
            )]),
            ..Default::default()
        }
    }
    fn inventory(&self) -> Result<Inventory> {
        let (entries, warnings) = self.entries()?;
        Ok(Inventory {
            entries,
            warnings,
            source: if self.scope == "system" {
                "Live installed local ALPM database"
            } else {
                "Offline root local ALPM database"
            }
            .into(),
            provider_label: self.label().into(),
            capabilities: self.capabilities(),
        })
    }
    fn metadata(&self, id: &PackageId) -> Result<PackageRecord> {
        self.checked(id)?;
        self.entries()?
            .0
            .into_iter()
            .find(|e| e.package.id == *id)
            .map(|e| e.package)
            .ok_or_else(|| {
                ProviderError::new(ErrorKind::NotFound, "Pacman package changed or was removed")
            })
    }
    fn dependencies(&self, id: &PackageId, reverse: bool) -> Result<DependencyReport> {
        self.checked(id)?;
        let (entries, _) = self.entries()?;
        let package = &entries
            .iter()
            .find(|e| e.package.id == *id)
            .ok_or_else(|| {
                ProviderError::new(ErrorKind::NotFound, "Pacman package no longer present")
            })?
            .package;
        let mut aliases = BTreeSet::from([package.name.as_str()]);
        aliases.extend(package.provides.iter().map(|v| relation_name(v)));
        let candidates = if reverse {
            entries
                .iter()
                .filter(|e| {
                    e.package.id != *id
                        && e.package
                            .requires
                            .iter()
                            .any(|r| aliases.contains(relation_name(r)))
                })
                .map(|e| e.package.id.clone())
                .collect()
        } else {
            vec![]
        };
        Ok(DependencyReport{package:id.clone(),requires:package.requires.clone(),provides:package.provides.clone(),reverse_candidates:candidates,reverse_queried:reverse,limitations:vec!["ALPM direct name/provision candidates do not resolve version constraints, conflicts or a native removal transaction. Optional dependencies remain native metadata.".into()]})
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    const RECORD:&str="%NAME%\nexample-lib\n%VERSION%\n2:1.0~rc1-3\n%ARCH%\nx86_64\n%DESC%\nLibrary <plain> & tools\n%SIZE%\n1234\n%INSTALLDATE%\n1700000000\n%LICENSE%\nMIT\nApache-2.0\n%DEPENDS%\nbase>=1\n%PROVIDES%\nlibexample.so=1-64\n%OPTDEPENDS%\nhelper: optional reason\n%BASE%\nexample\n%XDATA%\npkgtype=split\n";
    #[test]
    fn local_desc_preserves_units_versions_and_native_fields() {
        let p = record(&parse_fields(RECORD).unwrap(), "system").unwrap();
        assert_eq!(p.installed_bytes, Some(1234));
        assert_eq!(p.version_display, "2:1.0~rc1-3");
        assert!(p.title.contains("<plain>"));
        assert_eq!(p.requires, vec!["base>=1"]);
        assert!(p.repository.is_none());
        assert_eq!(
            p.native_metadata["pacman.optdepends"],
            "helper: optional reason"
        );
    }
    #[test]
    fn malformed_or_duplicate_fields_and_invalid_sizes_fail() {
        for text in ["orphan value", "%NAME%\na\n%NAME%\nb\n", "%bad%\nx\n"] {
            assert!(parse_fields(text).is_err());
        }
        assert!(record(
            &parse_fields(&RECORD.replace("1234", "overflow")).unwrap(),
            "system"
        )
        .is_err());
        assert!(record(&parse_fields("%NAME%\nexample\n").unwrap(), "system").is_err());
    }
    #[test]
    fn missing_numbers_remain_unknown_and_provision_names_ignore_only_constraints() {
        let p = record(
            &parse_fields("%NAME%\nmeta\n%VERSION%\n1-1\n%ARCH%\nany\n").unwrap(),
            "system",
        )
        .unwrap();
        assert!(p.installed_bytes.is_none());
        assert!(p.install_time.is_none());
        assert_eq!(relation_name("virtual>=2:1"), "virtual");
    }
    #[test]
    fn foreign_scopes_and_mutations_are_rejected_before_database_reads() {
        let p = PacmanProvider {
            root: "/no-database".into(),
            scope: "system".into(),
        };
        let id = PackageId {
            provider: "pacman".into(),
            scope: "root:other".into(),
            native: "example".into(),
        };
        assert_eq!(p.metadata(&id).unwrap_err().kind, ErrorKind::WrongProvider);
        assert!(matches!(
            p.prepare_removal(&id),
            Err(ProviderError {
                kind: ErrorKind::Unsupported,
                ..
            })
        ));
    }
}
