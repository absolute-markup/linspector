//! Shared interpretation of desktop entries. Values are data, never launch commands.
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};
pub(super) fn desktop_name(text: &str) -> Option<String> {
    let mut in_entry = false;
    let mut fields = BTreeMap::new();
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
            continue;
        }
        if in_entry {
            if let Some((k, v)) = line.split_once('=') {
                fields.insert(k, v);
            }
        }
    }
    if fields.get("Type") != Some(&"Application")
        || fields.get("Hidden") == Some(&"true")
        || fields.get("NoDisplay") == Some(&"true")
    {
        return None;
    }
    fields
        .get("Name")
        .filter(|n| !n.is_empty())
        .map(|n| n.to_string())
}
/// For offline roots, reject paths that escape through '..' or host-facing symlinks.
pub(super) fn rooted_file(root: &Path, native: &str) -> Option<PathBuf> {
    if !native.starts_with('/')
        || native.contains('\0')
        || Path::new(native)
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return None;
    }
    let path = root.join(native.trim_start_matches('/'));
    let actual = path.canonicalize().ok()?;
    if !actual.starts_with(root) || !actual.is_file() {
        return None;
    }
    Some(path)
}
pub(super) fn application_name(root: &Path, native: &str) -> Option<String> {
    if !native.starts_with("/usr/share/applications/") || !native.ends_with(".desktop") {
        return None;
    }
    let path = rooted_file(root, native)?;
    desktop_name(&fs::read_to_string(path).ok()?)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn desktop_visibility_and_groups() {
        assert_eq!(
            desktop_name(
                "[Desktop Entry]\nType=Application\nName=Studio\n[Desktop Action X]\nName=Other"
            ),
            Some("Studio".into())
        );
        assert_eq!(
            desktop_name("[Desktop Entry]\nType=Application\nName=X\nNoDisplay=true"),
            None
        );
        assert_eq!(desktop_name("[Desktop Entry]\nType=Link\nName=X"), None);
    }
}
