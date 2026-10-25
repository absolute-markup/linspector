//! Read-only RPM inventory. Package identity is NEVRA, never just name.
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, process::Command};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Package {
    pub name: String,
    pub title: String,
    pub description: String,
    pub epoch: String,
    pub version: String,
    pub release: String,
    pub arch: String,
    pub license: String,
    pub vendor: String,
    pub url: String,
    pub source_rpm: String,
    pub installed_bytes: String,
    pub install_time: String,
    pub requires: Vec<String>,
    pub provides: Vec<String>,
    pub repository: String,
    pub provenance: String,
}
impl Package {
    pub fn identity(&self) -> String {
        format!(
            "{}-{}:{}-{}.{}",
            self.name, self.epoch, self.version, self.release, self.arch
        )
    }
    pub fn matches(&self, query: &str) -> bool {
        let hay = format!(
            "{} {} {} {} {}",
            self.name, self.title, self.description, self.arch, self.source_rpm
        )
        .to_lowercase();
        query
            .split_whitespace()
            .all(|s| hay.contains(&s.to_lowercase()))
    }
}
#[derive(Default, Deserialize)]
struct Field {
    #[serde(rename = "string", default)]
    strings: Vec<String>,
    #[serde(rename = "integer", default)]
    integers: Vec<String>,
}
impl Field {
    fn value(&self) -> String {
        self.strings
            .first()
            .or(self.integers.first())
            .filter(|s| s.as_str() != "(none)")
            .cloned()
            .unwrap_or_default()
    }
}
#[derive(Default, Deserialize)]
struct RawPackage {
    #[serde(default)]
    name: Field,
    #[serde(default)]
    summary: Field,
    #[serde(default)]
    description: Field,
    #[serde(default)]
    epoch: Field,
    #[serde(default)]
    version: Field,
    #[serde(default)]
    release: Field,
    #[serde(default)]
    arch: Field,
    #[serde(default)]
    license: Field,
    #[serde(default)]
    vendor: Field,
    #[serde(default)]
    url: Field,
    #[serde(default)]
    sourcerpm: Field,
    #[serde(default)]
    size: Field,
    #[serde(default)]
    installtime: Field,
    #[serde(default)]
    requires: Field,
    #[serde(default)]
    provides: Field,
}
#[derive(Deserialize)]
struct Document {
    #[serde(rename = "package", default)]
    packages: Vec<RawPackage>,
}

/// XML escaping is performed by RPM, so descriptions may contain newlines and delimiters.
pub fn query_format() -> String {
    let mut s = "<package>".to_owned();
    for (field, tag) in [
        ("name", "NAME"),
        ("summary", "SUMMARY"),
        ("description", "DESCRIPTION"),
        ("epoch", "EPOCH"),
        ("version", "VERSION"),
        ("release", "RELEASE"),
        ("arch", "ARCH"),
        ("license", "LICENSE"),
        ("vendor", "VENDOR"),
        ("url", "URL"),
        ("sourcerpm", "SOURCERPM"),
        ("size", "SIZE"),
        ("installtime", "INSTALLTIME"),
    ] {
        s.push_str(&format!("<{field}>%{{{tag}:xml}}</{field}>"));
    }
    s.push_str("<requires>[%{REQUIRENEVRS:xml}]</requires><provides>[%{PROVIDENEVRS:xml}]</provides></package>");
    s
}
pub fn parse_rpm_xml(text: &str) -> Result<Vec<Package>, String> {
    let doc: Document = quick_xml::de::from_str(&format!("<inventory>{text}</inventory>"))
        .map_err(|e| e.to_string())?;
    let mut result = Vec::new();
    for r in doc.packages {
        let name = r.name.value();
        if name.is_empty()
            || r.version.value().is_empty()
            || (r.arch.value().is_empty() && name != "gpg-pubkey")
        {
            return Err("RPM record missing name, architecture or version".into());
        }
        let summary = r.summary.value();
        result.push(Package {
            title: if summary.is_empty() {
                name.clone()
            } else {
                summary
            },
            name,
            description: r.description.value(),
            epoch: {
                let e = r.epoch.value();
                if e.is_empty() {
                    "0".into()
                } else {
                    e
                }
            },
            version: r.version.value(),
            release: r.release.value(),
            arch: {
                let a = r.arch.value();
                if a.is_empty() {
                    "unknown".into()
                } else {
                    a
                }
            },
            license: r.license.value(),
            vendor: r.vendor.value(),
            url: r.url.value(),
            source_rpm: r.sourcerpm.value(),
            installed_bytes: r.size.value(),
            install_time: r.installtime.value(),
            requires: r.requires.strings,
            provides: r.provides.strings,
            repository: "Unknown — not recorded by this RPM query".into(),
            provenance: "Installed RPM database (live, read-only)".into(),
        });
    }
    result.sort_by_key(Package::identity);
    Ok(result)
}
fn rpm(args: &[&str]) -> Result<String, String> {
    // No shell, privilege escalation, transactions, or user-supplied query formats.
    let output = Command::new("/usr/bin/rpm")
        .args(args)
        .env("LC_ALL", "C")
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    String::from_utf8(output.stdout).map_err(|e| e.to_string())
}
pub fn installed() -> Result<Vec<Package>, String> {
    parse_rpm_xml(&rpm(&["-qa", "--qf", &query_format()])?)
}
/// RPM-reported direct reverse dependency candidates for explicit capabilities.
/// Not a transaction solver; implicit file dependencies and rich expressions may be incomplete.
pub fn reverse_candidates(p: &Package) -> Result<Vec<String>, String> {
    let capabilities: BTreeSet<_> = std::iter::once(p.name.clone())
        .chain(
            p.provides
                .iter()
                .filter_map(|s| s.split_whitespace().next().map(str::to_owned)),
        )
        .collect();
    let mut args = vec![
        "-q",
        "--whatrequires",
        "--qf",
        "%{NAME}-%{EPOCHNUM}:%{VERSION}-%{RELEASE}.%{ARCH}\\n",
        "--",
    ];
    args.extend(capabilities.iter().map(String::as_str));
    let out = Command::new("/usr/bin/rpm")
        .args(args)
        .env("LC_ALL", "C")
        .output()
        .map_err(|e| e.to_string())?;
    // RPM's exit status counts capability arguments without matches (not just 1).
    // Query diagnostics on stderr are errors; unmatched-capability notices on stdout
    // are filtered below. A terminating signal is always a failure.
    if out.status.code().is_none() || !out.stderr.is_empty() {
        return Err(format!(
            "RPM reverse query failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    let rows = String::from_utf8(out.stdout).map_err(|e| e.to_string())?;
    Ok(rows
        .lines()
        .filter(|line| !line.starts_with("no package requires "))
        .map(str::to_owned)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn xml_preserves_metadata_and_dependency_versions() {
        let xml = r#"<package><name><string>lib-example-devel</string></name><summary><string>Headers &amp; tools</string></summary><description><string>Line 1
Line 2 &lt;native&gt;</string></description><version><string>1.2</string></version><release><string>3.fc44</string></release><arch><string>i686</string></arch><requires><string>lib-example = 1.2-3.fc44</string><string>libc.so.6(GLIBC_2.34)</string></requires></package>"#;
        let p = parse_rpm_xml(xml).unwrap().remove(0);
        assert_eq!(p.title, "Headers & tools");
        assert!(p.description.contains("\nLine 2 <native>"));
        assert_eq!(p.requires.len(), 2);
        assert_eq!(p.epoch, "0");
        assert!(p.matches("DEVEL i686"));
        assert!(!p.matches("devel x86_64"));
    }
    #[test]
    fn rpm_public_key_records_without_arch_are_retained() {
        let p = parse_rpm_xml("<package><name><string>gpg-pubkey</string></name><version><string>abcdef</string></version><arch>(none)</arch></package>").unwrap();
        assert_eq!(p[0].arch, "unknown");
        assert_eq!(p[0].name, "gpg-pubkey");
    }
    #[test]
    fn malformed_or_incomplete_records_fail() {
        assert!(parse_rpm_xml("<package>").is_err());
        assert!(parse_rpm_xml("<package><name><string>x</string></name></package>").is_err());
    }
    #[test]
    fn fixture_is_explicitly_synthetic_and_includes_devel() {
        let p: Vec<Package> =
            serde_json::from_str(include_str!("../../../fixtures/sample-inventory.json")).unwrap();
        assert!(p.iter().any(|p| p.name.ends_with("-devel")));
        assert!(p.iter().all(|p| p.provenance.contains("Synthetic")));
        let ids: BTreeSet<_> = p.iter().map(Package::identity).collect();
        assert_eq!(ids.len(), p.len());
    }
}
