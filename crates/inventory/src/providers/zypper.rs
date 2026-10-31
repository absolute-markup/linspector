//! openSUSE native removal dry-run review. Execution is deliberately unsupported:
//! a separate re-solve followed by noninteractive remove cannot bind the reviewed
//! transaction atomically. No DNF fallback or authenticated mutation is provided.
use super::*;
use crate::Package;
use quick_xml::{events::Event, Reader};
use std::collections::{BTreeMap, BTreeSet};
fn error(message: impl Into<String>) -> ProviderError {
    ProviderError::new(ErrorKind::OperationFailed, message)
}
fn protected(name: &str) -> bool {
    super::dnf::protected(name)
        || [
            "zypper",
            "libzypp",
            "libsolv",
            "filesystem",
            "aaa_base",
            "permissions",
            "pam",
        ]
        .iter()
        .any(|p| name == *p || name.starts_with(&format!("{p}-")))
}
fn attrs(event: &quick_xml::events::BytesStart<'_>) -> Result<BTreeMap<String, String>> {
    event
        .attributes()
        .map(|item| {
            let item = item.map_err(|e| error(e.to_string()))?;
            Ok((
                String::from_utf8(item.key.as_ref().to_vec()).map_err(|e| error(e.to_string()))?,
                item.unescape_value()
                    .map_err(|e| error(e.to_string()))?
                    .into_owned(),
            ))
        })
        .collect()
}
fn resolve(attributes: &BTreeMap<String, String>, installed: &[Package]) -> Result<Package> {
    if attributes.get("type").map(String::as_str) != Some("package") {
        return Err(error("Zypper review contains a non-package change"));
    }
    let matches: Vec<_> = installed
        .iter()
        .filter(|p| {
            let edition = format!("{}-{}", p.version, p.release);
            let full = format!("{}:{edition}", p.epoch);
            attributes.get("name") == Some(&p.name)
                && attributes.get("arch") == Some(&p.arch)
                && attributes
                    .get("edition")
                    .is_some_and(|v| v == &full || (p.epoch == "0" && v == &edition))
        })
        .collect();
    if matches.len() != 1 {
        return Err(error(
            "Zypper change does not resolve to one exact installed RPM",
        ));
    }
    let package = matches[0];
    if package.arch == "unknown" || package.name == "gpg-pubkey" {
        return Err(error(
            "RPM key records cannot be reviewed as removable packages",
        ));
    }
    if protected(&package.name) {
        return Err(error(format!(
            "Zypper removal includes protected system/runtime package {}",
            package.name
        )));
    }
    Ok(package.clone())
}
pub(super) fn parse_review(
    xml: &str,
    installed: &[Package],
    requested: &Package,
) -> Result<RemovalReview> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut path: Vec<String> = vec![];
    let mut summaries = 0;
    let mut expected_count = None;
    let mut changes = vec![];
    let mut seen = BTreeSet::new();
    loop {
        match reader
            .read_event()
            .map_err(|e| error(format!("Invalid Zypper XML: {e}")))?
        {
            event @ (Event::Start(_) | Event::Empty(_)) => {
                let empty = matches!(&event, Event::Empty(_));
                let e = match event {
                    Event::Start(e) | Event::Empty(e) => e,
                    _ => unreachable!(),
                };
                let tag = String::from_utf8(e.name().as_ref().to_vec())
                    .map_err(|e| error(e.to_string()))?;
                if tag == "install-summary" {
                    if path != ["stream"] || summaries != 0 {
                        return Err(error("Unexpected or repeated Zypper summary"));
                    }
                    summaries += 1;
                    expected_count = Some(
                        attrs(&e)?
                            .get("packages-to-change")
                            .ok_or_else(|| error("Zypper summary has no package count"))?
                            .parse::<usize>()
                            .map_err(|_| error("Invalid Zypper change count"))?,
                    );
                }
                if path.iter().any(|p| p == "install-summary")
                    && tag.starts_with("to-")
                    && tag != "to-remove"
                {
                    return Err(error("Zypper summary contains changes other than removal"));
                }
                if tag == "error"
                    || (tag == "message" && attrs(&e)?.get("type").is_some_and(|v| v == "error"))
                {
                    return Err(error("Zypper reported a solver/transaction error"));
                }
                if tag == "solvable" {
                    if path != ["stream", "install-summary", "to-remove"] {
                        return Err(error("Unexpected solvable outside the removal section"));
                    }
                    let package = resolve(&attrs(&e)?, installed)?;
                    if !seen.insert(package.identity()) {
                        return Err(error("Duplicated Zypper removal"));
                    }
                    changes.push(PlannedChange {
                        native_id: package.identity(),
                        action: ChangeAction::Remove,
                        reason: if package.identity() == requested.identity() {
                            "User"
                        } else {
                            "Dependency"
                        }
                        .into(),
                    });
                }
                if !empty {
                    path.push(tag);
                }
            }
            Event::End(e) => {
                let tag = String::from_utf8(e.name().as_ref().to_vec())
                    .map_err(|e| error(e.to_string()))?;
                if path.pop().as_deref() != Some(tag.as_str()) {
                    return Err(error("Unbalanced Zypper XML"));
                }
            }
            Event::DocType(_) => {
                return Err(error("Zypper review must not contain a document type"))
            }
            Event::Eof => break,
            _ => (),
        }
    }
    if !path.is_empty()
        || summaries != 1
        || changes.is_empty()
        || expected_count != Some(changes.len())
        || !seen.contains(&requested.identity())
    {
        return Err(error(
            "Incomplete Zypper removal summary or selected package missing",
        ));
    }
    Ok(RemovalReview{executor_label:"Zypper (dry-run)".into(),requested:PackageId{provider:"rpm".into(),scope:"system".into(),native:requested.identity()},changes,notes:vec!["Read-only native Zypper removal review. Unused dependencies are kept.".into(),"Authenticated execution is not implemented: a reviewed transaction must remain bound to the native solver lock before changes are allowed.".into()]})
}
struct Plan {
    review: RemovalReview,
}
impl PreparedRemoval for Plan {
    fn review(&self) -> &RemovalReview {
        &self.review
    }
    fn execute(self: Box<Self>) -> Result<String> {
        Err(ProviderError::new(
            ErrorKind::Unsupported,
            "Zypper execution is unavailable; this is a dry-run review only",
        ))
    }
}
pub(super) fn preview(requested: &Package) -> Result<Box<dyn PreparedRemoval>> {
    let platform = crate::platform::detect();
    if platform.family != "zypper-rpm"
        || platform.management_model != "Mutable"
        || !platform.capabilities.removal_preview
    {
        return Err(ProviderError::new(
            ErrorKind::Unsupported,
            "Native Zypper preview requires a supported mutable openSUSE/SUSE host",
        ));
    }
    if requested.provenance.starts_with("Synthetic")
        || protected(&requested.name)
        || requested.arch == "unknown"
        || requested.name == "gpg-pubkey"
    {
        return Err(error("Selected package cannot be reviewed for removal"));
    }
    let installed = crate::installed().map_err(error)?;
    if !installed
        .iter()
        .any(|p| p.identity() == requested.identity())
    {
        return Err(error("Package changed; scan again"));
    }
    let edition = if requested.epoch == "0" {
        format!("{}-{}", requested.version, requested.release)
    } else {
        format!(
            "{}:{}-{}",
            requested.epoch, requested.version, requested.release
        )
    };
    let spec = format!("{}.{}={edition}", requested.name, requested.arch);
    let text = crate::process::large_query(crate::process::command("/usr/bin/zypper").args([
        "--xmlout",
        "--non-interactive",
        "--no-refresh",
        "--disable-repositories",
        "remove",
        "--dry-run",
        "--no-clean-deps",
        "--type",
        "package",
        "--",
        &spec,
    ]))
    .map_err(error)?
    .query_text()
    .map_err(error)?;
    Ok(Box::new(Plan {
        review: parse_review(&text, &installed, requested)?,
    }))
}
#[cfg(test)]
mod tests {
    use super::*;
    fn packages() -> Vec<Package> {
        serde_json::from_str(include_str!("../../../../fixtures/sample-inventory.json")).unwrap()
    }
    fn row(p: &Package) -> String {
        format!(
            "<solvable type=\"package\" name=\"{}\" edition=\"{}:{}-{}\" arch=\"{}\"/>",
            p.name, p.epoch, p.version, p.release, p.arch
        )
    }
    fn xml(rows: &str, count: usize) -> String {
        format!("<stream><install-summary packages-to-change=\"{count}\"><to-remove>{rows}</to-remove></install-summary></stream>")
    }
    #[test]
    fn exact_removal_xml_produces_a_source_bound_readonly_review() {
        let p = packages();
        let review =
            parse_review(&xml(&format!("{}{}", row(&p[0]), row(&p[1])), 2), &p, &p[0]).unwrap();
        assert_eq!(review.changes.len(), 2);
        assert_eq!(review.requested.native, p[0].identity());
        assert!(Box::new(Plan { review }).execute().is_err());
    }
    #[test]
    fn installs_wrong_versions_duplicate_and_protected_changes_are_rejected() {
        let p = packages();
        let normal = xml(&row(&p[0]), 1);
        assert!(parse_review(&normal.replace("to-remove", "to-install"), &p, &p[0]).is_err());
        assert!(parse_review(&normal.replace("0.demo", "wrong-version"), &p, &p[0]).is_err());
        assert!(
            parse_review(&xml(&format!("{}{}", row(&p[0]), row(&p[0])), 2), &p, &p[0]).is_err()
        );
        let mut core = p[0].clone();
        core.name = "libzypp".into();
        assert!(parse_review(&xml(&row(&core), 1), &[core.clone()], &core).is_err());
    }
    #[test]
    fn missing_counts_errors_unknown_objects_and_truncated_streams_fail() {
        let p = packages();
        let normal = xml(&row(&p[0]), 1);
        for text in [
            normal.replace("packages-to-change=\"1\"", ""),
            normal.replace("</stream>", ""),
            normal.replace("type=\"package\"", "type=\"pattern\""),
            normal.replace(
                "</stream>",
                "<message type=\"error\">failure</message></stream>",
            ),
        ] {
            assert!(parse_review(&text, &p, &p[0]).is_err());
        }
    }
}
