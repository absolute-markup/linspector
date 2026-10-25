# Linspector

A native Qt 6 Quick / QML desktop starter with a Rust inventory engine and CXX-Qt 0.8.1 QObject bridge. Edit the interface in `crates/desktop/qml/Main.qml` using VS Code and the Qt QML extension; Rust Analyzer supports the Rust crates.

## Build and run on Fedora

Prerequisites: Rust stable with Cargo, a C++17 compiler and Qt 6 development tools. The uploaded Fedora 44 list already includes the named C++/Qt toolchain; Rust/Cargo must also be available.

```sh
sudo dnf install gcc-c++ cmake ninja-build pkgconf-pkg-config qt6-qtbase-devel qt6-qtdeclarative-devel qt6-qttools-devel clang lld cargo rust
export QMAKE=/usr/bin/qmake6
cargo build --locked -p linspector
cargo run --locked -p linspector
```

The installation command is a manual prerequisite only; the application never invokes DNF or sudo. Cargo downloads dependencies on the first build. Cargo is the build entry point; CXX-Qt compiles and links the Qt/C++ bridge internally, so no top-level CMake project is necessary.

```sh
# Independent core tests; Qt is not compiled for this package
cargo test --locked -p linspector-inventory
# Optional read-only integration test on an RPM host
cargo test --locked -p linspector-inventory --test live_rpm -- --ignored
# Export complete live inventory as JSON
cargo run --locked -p linspector-inventory > inventory.json
# Clearly marked synthetic fixture in the GUI
cargo run --locked -p linspector -- --demo
# Headless startup check (exits after five seconds)
QT_QPA_PLATFORM=offscreen cargo run --locked -p linspector -- --demo --smoke
```

## Implemented

- Every installed RPM database entry: command-line tools, libraries, development packages, services and applications. No AppStream/desktop-file filter. Distinct architectures, epochs and parallel installed versions remain distinct.
- RPM summary as the package title, exact package name, multiline description, installed epoch/version/release, architecture, license, vendor, URL, source RPM, installed byte size and installation timestamp.
- Full required and provided capability expressions from RPM, including version constraints. Do not mistake capability strings for resolved package-name edges.
- On-demand RPM `--whatrequires` reverse dependency candidates for the selected package's name and explicit capabilities. Capability names are queried without version constraints, so these are labelled as candidates rather than solver-validated edges; file requirements and rich expressions may be incomplete. This is not a DNF transaction solver or exhaustive dependency graph.
- Background inventory and reverse queries, busy/error states, multiword search, selectable plain text details and refresh. Metadata is never evaluated as HTML, QML, a command or a shell argument string.
- Synthetic inventory for UI work; sanitized 3,012-row DNF attachment fixture and its review in `docs/upload-review.md`.

## Evidence and safe treatment

All queries use `/usr/bin/rpm` with argument arrays, without a shell or elevated privileges. There are no installation, removal, repair or update operations. No removal safety score is inferred from names, dependency counts or a missing reverse result. A successful scan is a snapshot; package changes during scanning are possible. A failed refresh retains the previous inventory and reports the error.

RPM source package, vendor and repository are different concepts. The live provider reports the source RPM and vendor; repository remains **Unknown** because this query cannot establish the original install repository. The uploaded DNF repository display is preserved separately as evidence, not silently joined onto live packages. Available update versions, installation reason, protected-package policy, signature/trust evaluation, resolved dependency graphs and transaction simulation are not implemented. Required capability expressions are shown exactly as RPM reports them. No links from untrusted metadata are automatically opened.

## Next implementation steps

1. Add a versioned DNF5/libdnf5 read-only provider for repository evidence, available versions and installation reasons, retaining explicit unknown values.
2. Add solver-backed dependency edges, file provides and rich dependency evaluation with integration fixtures for multilib and parallel kernels.
3. Replace the starter's JSON-to-QML list bridge with a paginated native QAbstractListModel for very large inventories, preserving the core API.
4. Add cancellation/timeouts for package-manager queries, selection retention, sort/filter controls and accessibility/keyboard testing.
5. Package Fedora RPMs and add CI against supported Qt/Rust/Fedora versions. Any future transactions need a separately reviewed design with policy checks, a preview and explicit confirmation.

See `docs/verification.md` for this archive's actual verification results and limits. Source copyright belongs to Absolute Markup; no open-source license grant is implied. Dependencies retain their respective licenses. Check Qt licensing for your eventual distribution model.

Integration reference: [CXX-Qt's Cargo build example, pinned to v0.8.1](https://github.com/KDAB/cxx-qt/tree/v0.8.1/examples/cargo_without_cmake). The version is deliberately pinned for reproducible starter builds, not asserted to be the latest release.
