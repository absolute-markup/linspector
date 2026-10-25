# Verification — 25 October 2026

This is a recreated starter, not a recovered original ZIP. The referenced chat provided no recoverable source archive. The uploaded text attachment was available through the earlier conversation attachment path; `/mnt/data/Pasted text.txt` itself was not mounted.

## Executed successfully

- Native desktop `cargo build --locked -p linspector`, with Qt 6.11.2, RPM 6.0.2 and CXX-Qt 0.8.1. The compiler emitted a Qt/GCC header warning about QChar/SFINAE; no build errors.
- Four unit tests: metadata/XML escaping and dependency version preservation; invalid/incomplete record rejection; public-key records without architecture; explicit synthetic fixture and distinct identities.
- Uploaded Fedora list test: 3,012 rows, all nine requested development tools present, epoch versions, unknown repository display and multilib preserved.
- Read-only live integration test: inventory count matches `rpm -qa`, metadata and requirements populated, reverse query returns installed package identities.
- CLI live export: 3,022 distinct installed identities; ten additional RPM public-key records lack architecture and are displayed with `unknown`, not discarded. `qt6-qtbase-devel` has its RPM summary and 85 required capability expressions.
- Native QML offscreen startup: demo reports 3 packages; live reports 3,022 packages. Both complete in five seconds with exit status 0. The smoke check fails if inventory is empty or still busy. QML component creation failure exits with an error.
- Source ZIP: CRC integrity, extraction and byte-for-byte comparison of every archived source file. Tests also run from an extracted copy before delivery. SHA-256 is supplied alongside the ZIP.

## Limits

These checks establish a compiled native starter and live RPM reading on this machine. They do not establish production readiness, a polished visual design, manual interaction/accessibility QA, successful distribution packaging, complete dependency resolution or cross-version compatibility. No installable binary/RPM is included in the source ZIP. A first build requires Rust dependency downloads and installed Qt development packages. CXX-Qt is pinned to a verified version, not the latest upstream release.

Reverse results deliberately use unversioned capability names as candidates. They may over-report across versions/architectures and miss implicit file dependencies or rich expressions. No removal decision may be inferred from them. Repository origin and available update versions are unknown in the live provider. Installation time is shown as Unix seconds. Query timeouts/cancellation and a native paginated list model are follow-up work documented in the README.
