# Linspector developer guide

Designed and developed by Absolute Markup. The desktop application uses native Qt Quick/QML, Rust and CXX-Qt 0.8.1. There is no browser runtime.

## Build and run on Fedora

Install Rust/Cargo **1.88 or newer**, Qt 6 development tools and a C++17 compiler. Check `rustc --version` after installation; a distribution-provided version can be too old for the lockfile. Fedora prerequisites:

```sh
sudo dnf install gcc-c++ cmake ninja-build pkgconf-pkg-config qt6-qtbase-devel qt6-qtdeclarative-devel qt6-qttools-devel clang lld cargo rust
export QMAKE=/usr/bin/qmake6
cargo build --locked -p linspector
cargo run --locked -p linspector
```

Run without `--demo` for live discovery. JSON is an in-process serialization format at the CXX-Qt boundary, not the inventory source. The default scan selects RPM, dpkg or pacman based on the detected distribution, adds independently available Flatpak/Snap sources, reads native package records and ownership, and collects mounted filesystems independently. See [Ubuntu/Debian build and test instructions](ubuntu-debian-testing.md). The explicit `--demo` flag loads a synthetic fixture and disables actions.

## Edit the design

- `crates/desktop/qml/Main.qml`: dashboard, category navigation, package list, inspector, storage and action review dialogs.
- `SystemPage.qml`: searchable system overview, summary cards, expandable sections and selectable values.
- `Fonts.qml`: bundled Hanken Grotesk, common type sizes and fallback.
- `Icons.qml`: bundled Remix Icon 4.6.0 and named glyphs from its official CSS map.
- `Icon.qml`: reusable Remix glyph component.
- `Theme.qml`: colours, surfaces and corner radii.
- `Surface.qml`, `ActionButton.qml`, `NavItem.qml`: shared components.

Fonts are local Qt resources and require no runtime network access. Their licenses are in `crates/desktop/assets/licenses`. Hanken Grotesk uses SIL OFL 1.1; Remix Icon 4.6.0 uses Apache 2.0. Assets are unmodified. See [Remix Icon 4.6.0](https://github.com/Remix-Design/RemixIcon/tree/v4.6.0) and [Hanken Grotesk](https://github.com/marcologous/hanken-grotesk).

CXX-Qt singleton files must be registered as `QmlFile::from(path).singleton(true)` in `build.rs`. Font and license resources are in `assets.qrc`. Build entry is Cargo; Qt/C++ bridge compilation is handled by CXX-Qt.

## Data and classification

`crates/inventory/src/providers/rpm_query.rs` queries installed RPM metadata as escaped XML. `lib.rs` retains the original RPM CLI compatibility exports. Every installed NEVRA remains distinct, including multilib, parallel versions and architecture-less public-key records.

`providers/rpm_catalog.rs` adds RPM-owned, visible `Type=Application` desktop entries and eligible non-template systemd service files. Applications are package groups, not a count of all launchers. Hidden and NoDisplay entries are excluded. Development and library categories are discoverability heuristics (`-devel`/`-static`, shared-library capabilities); they are not security, trust or removal-safety judgements. The Applications view does not currently include Flatpak, Snap, local application overrides or manually installed software.

Storage comes from `df` in bytes. Root volume statistics and package installed-size totals have different meanings. Shared Btrfs mount capacity is never added together. RPM and dpkg footprints can double-count shared files and differ from allocated/compressed bytes. dpkg Installed-Size is an estimate recorded in KiB, converted to bytes; missing values stay unknown.

## Removal and startup changes

`providers/dnf.rs`, called by the RPM adapter, generates a stored DNF5 transaction using `remove --no-autoremove --store`. Repositories are disabled because this operation only removes installed packages. Preparing a plan never runs it. The application validates that every planned item is a known installed RPM, every action is `Remove`, the selected package is included, the plan is nonempty and no protected package is included.

The native modal displays every planned removal. Confirmation now sends exact reviewed identities through a bounded stdin protocol to `/usr/libexec/linspector-helper` using pkexec. The helper builds a fresh root-owned plan under `/run/linspector`, checks the reviewed snapshot/removal set and invokes native replay without mismatch-relaxation flags. User-owned transaction paths are never privileged inputs. The root-owned policy uses per-invocation administrator authentication without retained authorization. Execution is unavailable until the matching helper/policy are installed; previews remain read-only.

Service changes use exact RPM ownership and loaded fragment/state checks. Static, masked, overridden, template and multi-unit/alias install cases are refused. System startup changes pass through the same narrow helper; user changes remain unprivileged. Neither path uses `--now`, start, stop or mask. The state is queried afterward.

All action outcomes trigger a fresh scan, including authentication dismissal/denial and unsuccessful native operations. A failed/interrupted native operation may already have changed state; there is no automatic retry/rollback. See [the boundary review](management-validation.md) and [guest-only setup/test procedure](fedora-vm-testing.md). Privileged mutation and real authentication remain unverified in this session.

Processes receive argument arrays without a shell. Core system and Linspector runtime packages have a conservative built-in protection policy in addition to DNF's policy. Eligible Qt development headers are not confused with runtime packages. Queries and actions run on worker threads; selection changes are checked before accepting reverse results and before removal execution.

## Verify

```sh
cargo test --locked -p linspector-inventory
# Live read-only tests; includes a stored DNF preview, never replay
cargo test --locked -p linspector-inventory --test live_rpm -- --include-ignored
# Native synthetic dpkg fixture, if /usr/bin/dpkg-query is installed
cargo test --locked -p linspector-inventory --test dpkg_native native_query_preserves -- --ignored
QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software cargo run --locked -p linspector -- --smoke
QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software cargo run --locked -p linspector -- --smoke --capture /tmp/linspector.png
cargo run --locked -p linspector -- --demo
```

Smoke mode installs a Qt diagnostic handler and exits after checking the live catalogue. Native screenshots use QQuickWindow::grabWindow. The CLI `linspector-inventory` remains a read-only RPM JSON exporter.

Follow-up work: native validation and management for the new software sources, native QAbstractListModel instead of the JSON bridge for very large inventories, interactive query cancellation, richer source/repository evidence, complete dependency graphs, disposable-VM validation and release packaging of the new privileged helper, accessibility/keyboard QA, and Fedora RPM distribution builds. No installable release is produced by these source changes.

## System details

`crates/inventory/src/host.rs` collects ten sections: operating system; account/session; hardware/firmware; processor; memory/swap; graphics/displays; storage devices; network interfaces; power/battery; and reported security/platform state. Values are collected on the background scan and refreshed by Scan system. CPU topology and caches include hierarchical `lscpu` JSON; memory numbers use `/proc/meminfo` kB × 1024; available RAM is not confused with free RAM. Storage device sizes are distinct from filesystem statistics on Storage.

Account identity is selected by real UID (`id -ru`) from NSS (`getent passwd`), with `/etc/passwd` fallback. Username, GECOS display name, UID/GID, home and login shell come from that record. Effective UID and group names are also shown. Session values use an explicit environment-variable allowlist, not an environment dump. Password/shadow files, account credentials and hardware serial numbers are not collected. Account/network details are displayed locally; scanning has no network-upload feature.

Optional utility probes have a three-second child deadline and bounded output. Missing or restricted sources become Unavailable without failing package inventory. The collector does not elevate privileges. Demo mode supplies no real machine/account report. Display modes are labelled as advertised modes, not asserted to be active resolutions. Security fields report observed state rather than assigning a security rating.

Capture the System view for local inspection with `--smoke --smoke-view System --capture /tmp/linspector-system.png`. Avoid adding a real System-page screenshot to repository documentation because it contains account details.

## Platform detection and partial discovery

`platform.rs` separates distribution/tool evidence from implemented capabilities. The GUI receives platform details, package readiness, removal availability and service-control availability. It disables actions while a fresh scan runs. Rust management entry points independently re-detect the platform so a direct invocation cannot bypass the GUI gate.

Only mutable Fedora 44 supports DNF5 previews and the new helper action path; derivative IDs, new Fedora releases, image-based hosts, unknown systems and detected sandboxes/containers do not inherit it. RPM inspection and ownership discovery are isolated from generic system/storage loading. Unsupported provider or RPM failure yields a partial snapshot with explicit reasons; failed ownership discovery preserves package metadata but disables startup changes. dpkg inspection is selected on identified APT-family systems; package and service changes remain disabled. Ubuntu/Debian data has additionally been tested against real extracted base-image databases, but full native desktop startup there is still pending.

## Shared package-provider contract (Milestone 1)

`model.rs` holds provider-neutral package IDs and records. Identity includes provider, installation scope and the opaque native ID; QML uses a collision-safe serialized key and never assembles NEVRA strings. Version display is supplied by the provider. Optional metadata uses null/None, with RPM epoch/version/release retained under namespaced native metadata.

`providers::PackageProvider` supplies inventory, metadata, dependencies, capability reporting and optional removal planning. `providers::select` selects only implemented providers from detected platform capabilities; RPM, dpkg and local ALPM/pacman adapters are implemented; Flatpak and Snap coexist as extra sources. `PreparedRemoval` owns the native plan and exposes a serializable review; consuming execution prevents accidental reuse. Only its Rust implementation can access DNF transaction directories. The UI names the executor supplied by the review instead of hardcoding DNF.

The scan coordinator obtains package entries through the factory. Desktop metadata/dependency/removal calls also use the contract. `services.rs` uses provider ownership information but remains separate from package mutation. Native RPM and DNF details live in `providers/rpm_query.rs`, `rpm_catalog.rs`, `rpm.rs` and `dnf.rs`. `management.rs` is a compatibility facade for the earlier RPM API, not the extension point.

Provider-contract tests use an in-memory test source with an opaque non-RPM-shaped ID/version; the real dpkg adapter is additionally exercised by native fixtures and extracted-image database tests. Live tests on the designated Fedora 44 host compare native and provider inventories and prepare/discard removal plans without executing them. See [milestone approval checkpoints](milestones.md).


## Read-only dpkg provider (Milestone 2)

`providers/dpkg.rs` queries `/usr/bin/dpkg-query --admindir=... --no-pager --status` and `--search` without a shell. Queries have a 20-second deadline and bounded output. Root/database/pager environment overrides are removed. It never invokes apt, dpkg installation/removal, package scripts, desktop Exec values or service commands.

The Deb822 parser preserves Debian versions and native fields. Present partial states remain listed with warnings; removed config-file-only records are excluded. Maintainer is not mislabeled as vendor. Package-origin repository, license and installation date cannot be inferred reliably from the status database and remain unavailable. Source package name falls back to the binary package name when Source is absent.

`providers/catalog.rs` shares desktop-entry interpretation with RPM. dpkg ownership comes from native search results, not filename/name guesses. Offline paths must stay within the selected root. The current application scope is `/usr/share/applications`; local overrides, diversions and desktop-environment-specific visibility need further work. Service association does not grant permission to change services.

The offline helper is for explicit validation and never changes the live provider or pretends that System/Storage belong to that root:

```sh
cargo run --locked -p linspector-inventory --example inspect_dpkg_root -- /path/to/extracted-root > /tmp/dpkg-inventory.json
LINSPECTOR_DPKG_ROOT=/path/to/extracted-root cargo test --locked -p linspector-inventory --test dpkg_native real_root_matches -- --ignored --nocapture
```

These require host dpkg-query. The first command exports normalized inventory JSON; the integration test compares identity sets against native `--show`, queries fresh metadata/reverse candidates and verifies database files stayed unchanged. Use a trusted, extracted/mounted root. It does not chroot, start containers or run the root's package scripts.

Debian references: [dpkg-query](https://manpages.debian.org/trixie/dpkg/dpkg-query.1.en.html), [control fields and Installed-Size](https://www.debian.org/doc/debian-policy/ch-controlfields.html), [dependency relationships](https://www.debian.org/doc/debian-policy/ch-relationships.html). The installed query must support no-argument `--status` and `--no-pager` (dpkg 1.19.2 or newer). Older releases have not been validated.

For a repeatable identity/version/size comparison and database SHA-256 check:

```sh
cargo build --locked -p linspector-inventory --example inspect_dpkg_root
python3 scripts/verify_dpkg_root.py /path/to/extracted-root
```

For a trusted amd64 base image, `--image-amd64-query` additionally runs that image's query executable with its own loader/libraries under the current host. It does not boot the image and must never be described as a full distribution runtime test. This was the method used for the three recorded base-image comparisons.


## Milestone 4 sources

See [integration architecture, limits and commands](milestone4-integrations.md). `system::Snapshot.sources` distinguishes known-empty counts from unavailable sources. `Entry.capabilities` and `providers::for_package` bind operations to the selected source; `providers::current` remains the native-provider compatibility API. A secondary source failure marks the combined scan partial without deleting native entries or granting/denying unrelated native capabilities.

The CLI now supports `--flatpak`, `--snap`, `--snap-fixture FILE` and `--pacman-root ROOT`; its no-argument RPM export remains available. Flatpak fixture construction is a separate temporary-root step, not an application scan. Source-specific UI fields retain unavailable metadata rather than guessing it. Zypper reviews are read-only; `PreparedRemoval.execute` returns Unsupported for that adapter.
