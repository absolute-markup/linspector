//! Read-only snapd GET /v2/snaps?select=all, preserving installed revisions/components.
use super::*;
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{Read, Write},
    os::unix::net::UnixStream,
    time::{Duration, Instant},
};
const LIMIT: usize = 8 * 1024 * 1024;
pub(super) struct SnapProvider;
fn error(message: impl Into<String>) -> ProviderError {
    ProviderError::new(ErrorKind::OperationFailed, message)
}
fn query() -> Result<String> {
    let mut stream = UnixStream::connect("/run/snapd.socket")
        .map_err(|e| error(format!("snapd socket unavailable: {e}")))?;
    stream
        .set_write_timeout(Some(Duration::from_secs(3)))
        .map_err(|e| error(e.to_string()))?;
    stream
        .write_all(
            b"GET /v2/snaps?select=all HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
        )
        .map_err(|e| error(e.to_string()))?;
    let start = Instant::now();
    let mut bytes = vec![];
    let mut buffer = [0; 8192];
    loop {
        let remaining = Duration::from_secs(8)
            .checked_sub(start.elapsed())
            .ok_or_else(|| error("snapd read-only request timed out"))?;
        stream
            .set_read_timeout(Some(remaining.min(Duration::from_secs(3))))
            .map_err(|e| error(e.to_string()))?;
        let n = stream
            .read(&mut buffer)
            .map_err(|e| error(format!("snapd read failed: {e}")))?;
        if n == 0 {
            break;
        }
        bytes.extend_from_slice(&buffer[..n]);
        if bytes.len() > LIMIT + 65536 {
            return Err(error("snapd response exceeded its limit"));
        }
    }
    http_body(&bytes)
}
fn http_body(bytes: &[u8]) -> Result<String> {
    let split = bytes
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or_else(|| error("Malformed snapd HTTP response"))?;
    if split > 65536 {
        return Err(error("snapd HTTP headers exceeded limit"));
    }
    let headers =
        std::str::from_utf8(&bytes[..split]).map_err(|_| error("Invalid snapd HTTP headers"))?;
    let mut lines = headers.split("\r\n");
    let status_line = lines.next().unwrap_or("");
    if !status_line.starts_with("HTTP/1.1 ") && !status_line.starts_with("HTTP/1.0 ") {
        return Err(error("Invalid snapd HTTP status line"));
    }
    let status = status_line.split_whitespace().nth(1).unwrap_or("");
    if status != "200" {
        return Err(error(format!("snapd returned HTTP {status}")));
    }
    let mut length = None;
    let mut chunked = false;
    let mut transfer_seen = false;
    for line in lines {
        let (k, v) = line
            .split_once(':')
            .ok_or_else(|| error("Malformed snapd HTTP header"))?;
        if k.eq_ignore_ascii_case("content-length") {
            if length.is_some() {
                return Err(error("Duplicate HTTP length"));
            }
            length = Some(
                v.trim()
                    .parse::<usize>()
                    .map_err(|_| error("Invalid HTTP length"))?,
            );
        }
        if k.eq_ignore_ascii_case("transfer-encoding") {
            if transfer_seen {
                return Err(error("Duplicate HTTP transfer encoding"));
            }
            transfer_seen = true;
            if v.trim().eq_ignore_ascii_case("chunked") {
                chunked = true;
            } else {
                return Err(error("Unsupported snapd transfer encoding"));
            }
        }
    }
    if length.is_some() && chunked {
        return Err(error("Ambiguous snapd HTTP framing"));
    }
    let body = &bytes[split + 4..];
    let decoded = if chunked {
        let mut result = vec![];
        let mut cursor = 0;
        loop {
            let end = body[cursor..]
                .windows(2)
                .position(|w| w == b"\r\n")
                .map(|i| cursor + i)
                .ok_or_else(|| error("Truncated HTTP chunk"))?;
            let size = std::str::from_utf8(&body[cursor..end])
                .map_err(|_| error("Invalid HTTP chunk"))?
                .split(';')
                .next()
                .unwrap_or("");
            let size = usize::from_str_radix(size.trim(), 16)
                .map_err(|_| error("Invalid HTTP chunk size"))?;
            cursor = end + 2;
            if size == 0 {
                let trailers = &body[cursor..];
                if trailers != b"\r\n" && !trailers.ends_with(b"\r\n\r\n") {
                    return Err(error("Truncated HTTP chunk terminator"));
                }
                break;
            }
            if size > LIMIT.saturating_sub(result.len())
                || size > body.len().saturating_sub(cursor + 2)
            {
                return Err(error("Oversized/truncated snapd HTTP chunk"));
            }
            result.extend_from_slice(&body[cursor..cursor + size]);
            cursor += size;
            if body.get(cursor..cursor + 2) != Some(b"\r\n") {
                return Err(error("Invalid HTTP chunk ending"));
            }
            cursor += 2;
        }
        result
    } else {
        if length.is_some_and(|n| n != body.len()) {
            return Err(error("Truncated or oversized snapd body"));
        }
        body.to_vec()
    };
    if decoded.len() > LIMIT {
        return Err(error("snapd JSON exceeded its limit"));
    }
    String::from_utf8(decoded).map_err(|_| error("snapd JSON is not UTF-8"))
}
fn text(value: &Value, key: &str) -> String {
    value.get(key).and_then(Value::as_str).unwrap_or("").into()
}
fn revision(value: &Value) -> Result<String> {
    let s = match value {
        Value::String(v) => v.clone(),
        Value::Number(n) => n.to_string(),
        _ => return Err(error("Snap revision is missing")),
    };
    if s.is_empty() || s.len() > 128 || !s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
        return Err(error("Invalid Snap revision"));
    }
    Ok(s)
}
fn name(value: &Value) -> Result<String> {
    let n = text(value, "name");
    if n.is_empty()
        || !n
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"-_".contains(&b))
    {
        return Err(error("Invalid Snap package/component name"));
    }
    Ok(n)
}
fn size(value: &Value) -> Result<Option<u64>> {
    match value.get("installed-size") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(n)) => n
            .as_u64()
            .map(Some)
            .ok_or_else(|| error("Invalid Snap size")),
        Some(Value::String(s)) => s.parse().map(Some).map_err(|_| error("Invalid Snap size")),
        _ => Err(error("Invalid Snap size")),
    }
}
pub(super) fn parse(text_value: &str, fixture: bool) -> Result<Inventory> {
    if text_value.len() > LIMIT {
        return Err(error("Snap JSON exceeds limit"));
    }
    let envelope: Value = serde_json::from_str(text_value).map_err(|e| error(e.to_string()))?;
    if envelope["type"] != "sync" || envelope["status-code"] != 200 {
        return Err(error(format!(
            "snapd did not return a successful installed inventory: {}",
            envelope["result"]["message"]
        )));
    }
    let rows = envelope["result"]
        .as_array()
        .ok_or_else(|| error("snapd inventory is not an array"))?;
    let mut entries = vec![];
    let mut seen = BTreeSet::new();
    let caps = SnapProvider.capabilities();
    for row in rows {
        let n = name(row)?;
        let rev = revision(&row["revision"])?;
        let state = text(row, "status");
        if !matches!(state.as_str(), "active" | "installed") {
            return Err(error("Unexpected state in installed Snap inventory"));
        }
        let kind = text(row, "type");
        let mut metadata = BTreeMap::new();
        for key in [
            "status",
            "type",
            "confinement",
            "tracking-channel",
            "channel",
            "install-date",
            "base",
            "broken",
            "instance-key",
        ] {
            if let Some(v) = row.get(key) {
                metadata.insert(
                    format!("snap.{key}"),
                    v.as_str()
                        .map(str::to_owned)
                        .unwrap_or_else(|| v.to_string()),
                );
            }
        }
        metadata.insert("snap.revision".into(), rev.clone());
        let id = PackageId {
            provider: "snap".into(),
            scope: if fixture { "fixture:system" } else { "system" }.into(),
            native: format!("{n}@{rev}"),
        };
        let mut requires = vec![];
        let base = text(row, "base");
        if !base.is_empty() && base != "none" {
            requires.push(format!("snap:{base}"));
        }
        let title = text(row, "title");
        let summary = text(row, "summary");
        let version = text(row, "version");
        let package = PackageRecord {
            key: id.key(),
            id,
            name: n.clone(),
            title: if !title.is_empty() {
                title
            } else if !summary.is_empty() {
                summary
            } else {
                n.clone()
            },
            description: text(row, "description"),
            version_display: if version.is_empty() {
                rev.clone()
            } else {
                version
            },
            arch: if text(row, "architecture").is_empty() {
                "unknown".into()
            } else {
                text(row, "architecture")
            },
            license: match text(row, "license").as_str() {
                "" | "unset" => None,
                s => Some(s.into()),
            },
            vendor: row["publisher"]["display-name"]
                .as_str()
                .or_else(|| row["publisher"]["username"].as_str())
                .or_else(|| row["developer"].as_str())
                .map(str::to_owned),
            url: row
                .get("website")
                .and_then(Value::as_str)
                .map(str::to_owned),
            source_package: None,
            repository: None,
            installed_bytes: size(row)?,
            install_time: None,
            requires,
            provides: vec![format!("snap:{n}"), format!("snap:{n}@{rev}")],
            provenance: if fixture {
                "Synthetic Snap API fixture (not the live daemon)"
            } else {
                "Installed snapd revisions (read-only local API)"
            }
            .into(),
            native_metadata: metadata,
        };
        if !seen.insert(package.key.clone()) {
            return Err(error("Duplicate installed Snap revision"));
        }
        entries.push(Entry {
            category: if kind == "app" {
                "Applications"
            } else if matches!(kind.as_str(), "base" | "core" | "os") {
                "Libraries"
            } else {
                "Packages"
            }
            .into(),
            package: package.clone(),
            applications: vec![],
            services: vec![],
            capabilities: caps.clone(),
        });
        for component in row
            .get("components")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if component.get("revision").is_none() {
                continue;
            } // Available components are not installed.
            let component_name = name(component)?;
            let component_rev = revision(&component["revision"])?;
            let mut p = package.clone();
            p.id.native = format!("{n}@{rev}/component/{component_name}@{component_rev}");
            p.key = p.id.key();
            p.name = format!("{n}+{component_name}");
            p.title = component
                .get("summary")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .unwrap_or(&p.name)
                .into();
            p.description = text(component, "description");
            p.version_display = component
                .get("version")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .unwrap_or(&component_rev)
                .into();
            p.installed_bytes = size(component)?;
            p.license = None;
            p.url = None;
            p.native_metadata
                .insert("snap.type".into(), "component".into());
            p.native_metadata.remove("snap.base");
            p.native_metadata.remove("snap.install-date");
            if let Some(date) = component.get("install-date").and_then(Value::as_str) {
                p.native_metadata
                    .insert("snap.install-date".into(), date.into());
            }
            p.requires = vec![format!("snap:{n}@{rev}")];
            p.provides = vec![format!("snap-component:{n}+{component_name}")];
            p.native_metadata
                .insert("snap.component-revision".into(), component_rev);
            p.native_metadata
                .insert("snap.component-type".into(), text(component, "type"));
            if !seen.insert(p.key.clone()) {
                return Err(error("Duplicate Snap component attachment"));
            }
            entries.push(Entry {
                package: p,
                category: "Libraries".into(),
                applications: vec![],
                services: vec![],
                capabilities: caps.clone(),
            });
        }
    }
    entries.sort_by_key(|e| e.package.identity());
    Ok(Inventory {
        entries,
        warnings: vec![],
        source: if fixture {
            "Synthetic installed Snap API fixture"
        } else {
            "Live snapd installed revisions/components (read-only)"
        }
        .into(),
        provider_label: "Snap".into(),
        capabilities: caps,
    })
}
fn dependency_report(entries: &[Entry], id: &PackageId, reverse: bool) -> Result<DependencyReport> {
    let package = &entries
        .iter()
        .find(|e| e.package.id == *id)
        .ok_or_else(|| {
            ProviderError::new(ErrorKind::NotFound, "Snap revision no longer installed")
        })?
        .package;
    let candidates = if reverse {
        entries
            .iter()
            .filter(|e| {
                e.package.id != *id
                    && e.package.id.scope == id.scope
                    && e.package
                        .requires
                        .iter()
                        .any(|r| package.provides.contains(r))
            })
            .map(|e| e.package.id.clone())
            .collect()
    } else {
        vec![]
    };
    Ok(DependencyReport{package:id.clone(),requires:package.requires.clone(),provides:package.provides.clone(),reverse_candidates:candidates,reverse_queried:reverse,limitations:vec!["Snap relationships include only reported base names and installed component attachments. Content interfaces, revision constraints and automatic connection semantics are not resolved; this is not a removal plan.".into()]})
}
impl PackageProvider for SnapProvider {
    fn id(&self) -> &str {
        "snap"
    }
    fn label(&self) -> &str {
        "Snap"
    }
    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities{inventory:true,metadata:true,dependencies:true,reverse_candidates:true,reasons:BTreeMap::from([("removal".into(),"Snap inspection is read-only; removal, enable/disable and permission changes are not implemented.".into())]),..Default::default()}
    }
    fn inventory(&self) -> Result<Inventory> {
        parse(&query()?, false)
    }
    fn metadata(&self, id: &PackageId) -> Result<PackageRecord> {
        if id.provider != "snap" || id.scope != "system" {
            return Err(ProviderError::new(
                ErrorKind::WrongProvider,
                "Snap source/scope mismatch",
            ));
        }
        self.inventory()?
            .entries
            .into_iter()
            .find(|e| e.package.id == *id)
            .map(|e| e.package)
            .ok_or_else(|| {
                ProviderError::new(ErrorKind::NotFound, "Snap revision no longer installed")
            })
    }
    fn dependencies(&self, id: &PackageId, reverse: bool) -> Result<DependencyReport> {
        if id.provider != "snap" || id.scope != "system" {
            return Err(ProviderError::new(
                ErrorKind::WrongProvider,
                "Snap source/scope mismatch",
            ));
        }
        dependency_report(&self.inventory()?.entries, id, reverse)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn http_length_and_chunking_are_bounded_and_unambiguous() {
        assert_eq!(
            http_body(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n[]").unwrap(),
            "[]"
        );
        assert_eq!(
            http_body(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n2\r\n[]\r\n0\r\n\r\n")
                .unwrap(),
            "[]"
        );
        for bytes in [
            b"HTTP/1.1 403 Denied\r\n\r\n".as_slice(),
            b"HTTP/1.1 200 OK\r\nContent-Length: 20\r\n\r\n[]",
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nContent-Length: 0\r\n\r\n",
        ] {
            assert!(http_body(bytes).is_err());
        }
    }
    #[test]
    fn daemon_errors_never_become_an_empty_success() {
        assert!(parse(&" ".repeat(LIMIT + 1), false).is_err());
        assert!(parse(
            r#"{"type":"error","status-code":403,"result":{"message":"denied"}}"#,
            false
        )
        .is_err());
        assert!(parse(r#"{"type":"sync","status-code":200,"result":{}}"#, false).is_err());
        assert!(
            parse(r#"{"type":"sync","status-code":200,"result":[]}"#, false)
                .unwrap()
                .entries
                .is_empty()
        );
    }
    #[test]
    fn foreign_scope_and_mutations_fail_before_socket_access() {
        let id = PackageId {
            provider: "snap".into(),
            scope: "user:1000".into(),
            native: "example@1".into(),
        };
        assert_eq!(
            SnapProvider.metadata(&id).unwrap_err().kind,
            ErrorKind::WrongProvider
        );
        assert!(matches!(
            SnapProvider.prepare_removal(&id),
            Err(ProviderError {
                kind: ErrorKind::Unsupported,
                ..
            })
        ));
    }
}

#[cfg(test)]
mod relationship_tests {
    use super::*;
    #[test]
    fn base_and_component_candidates_use_one_revision_snapshot() {
        let inventory = parse(
            include_str!("../../../../fixtures/snap/installed.json"),
            true,
        )
        .unwrap();
        let base = &inventory
            .entries
            .iter()
            .find(|e| e.package.name == "core22")
            .unwrap()
            .package
            .id;
        assert_eq!(
            dependency_report(&inventory.entries, base, true)
                .unwrap()
                .reverse_candidates
                .len(),
            2
        );
        let app = &inventory
            .entries
            .iter()
            .find(|e| e.package.id.native == "example@12")
            .unwrap()
            .package
            .id;
        let report = dependency_report(&inventory.entries, app, true).unwrap();
        assert_eq!(report.reverse_candidates.len(), 1);
        assert!(report.reverse_candidates[0].native.contains("/component/"));
    }
}
