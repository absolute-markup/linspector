# Verification

## Milestone 4 — broader software integrations, 27 October 2026

**Adapters and read-only reviews implemented; native runtime/management gaps remain. Milestone 4 is not fully complete.**

- 99 distinct Rust tests passed across regular and opt-in runs: 74 unit, seven RPM/live, six provider-contract, three dpkg integration, two Flatpak integration, two unprivileged helper-process, four new provider fixture and one compile-fail test. The dpkg real-root case was repeated for Ubuntu 24.04 and Debian 12/13.
- Live Flatpak 1.18.4: all 91 installed refs (30 apps and 61 runtimes/extensions) matched native kinds/scopes/versions, full commits and exact reported byte sizes. Same-scope runtime candidates matched native app-runtime queries. Installed reference/active-commit listings were unchanged.
- Isolated native Flatpak fixture: 12 refs across stable/beta branches and user/default-system/named-system scopes. Native info size/ref routing and unique identities passed. System fixtures are copied synthetic deployments, not privileged installs; applications were never launched. Cache/export-trigger sandbox warnings were retained without disabling sandbox checks.
- Synthetic ALPM filesystem: installed metadata, bytes/epoch versions, ownership, virtual dependencies, lock rejection and out-of-root DB link rejection passed. No real Arch runtime/native pacman comparison was available.
- Synthetic snapd API/framing: active/inactive/local revisions, installed-only components, base/component candidates, unknown metadata, malformed/oversized/error framing and unsupported actions checked. The real socket and private mock Unix-socket binding are denied in this session; in-memory protocol tests are not a live daemon test.
- Source-derived Zypper XML review: exact RPM matching and rejection of installs/other changes, duplicates, counts, missing selections, errors and protected packages passed. Zypper itself is absent here. Authenticated Zypper execution is not implemented; the dry-run cannot be safely treated as an automatically accepted second transaction. Legacy DNF preview is explicitly denied for Zypper hosts even when both tools are present.
- Native Qt/Rust build, live Overview/Flatpak/Snap-unavailable views and demo smoke passed. The live combined view preserves 3,022 RPM + 91 Flatpak entries while Snap is explicitly unavailable with an unknown count. Switching source tabs clears an incompatible selection, and unavailable views do not display a false zero or an unrelated RPM removal control.
- Existing Fedora/DNF prepare-and-discard and dpkg database regressions passed. No working host package/service changes or native authentication/privileged changes were performed. Test fixture writes were restricted to temporary roots.

See [integration semantics and reproduction commands](milestone4-integrations.md), [machine-readable evidence](verification/milestone4.json) and [milestone checkpoints](milestones.md). Native Arch, reachable Snap, openSUSE runtime/execution and earlier Milestone 2/3 desktop/management gaps remain open. Milestone 5 has not started.

---


## Milestone 3 — helper and lifecycle implementation, 26 October 2026

**Automated/read-only checks passed; real privileged VM tests remain pending. This milestone is not complete.**

- 70 distinct Rust tests passed across ordinary and opt-in runs: 52 unit, seven RPM/live, five provider-contract, three dpkg integration, two actual unprivileged helper-process rejection and one compile-fail test. The real-root dpkg case was exercised on three databases. New fault-injection tests do not establish real authentication or mutation success.
- Exact reviewed snapshot/removal checks, protected dependencies, foreign IDs, oversized/unknown protocol fields, failure/partial outcome and unsuccessful postcondition cases passed. Native failure and interrupted outcomes are injected into an in-memory host; none remove real packages.
- Cancelled/late/selection-mismatched review results are dropped, confirmation consumes a plan once, and actual private-directory cleanup is checked. Controlled child processes verify 126/127 exit-code transport, bounded output and deadlines including descendants holding pipes. Those exit codes are simulated, not obtained from real polkit dialogs.
- The native helper binary rejects arbitrary arguments and unauthenticated ordinary execution. The missing-helper policy preserves previews and disables execution/startup changes; there is no direct elevated DNF/systemctl fallback.
- Live Fedora regressions passed with all 3,022 RPM identities, metadata/dependencies, application/storage/system data and DNF prepare/discard only. Ubuntu/Debian native database regressions also passed; full Ubuntu/Debian desktop startup remains open.
- The native Qt/Rust desktop and non-setuid helper compiled. The removal-preview UI rendered the real six-package DNF plan with execution disabled and a reason. Overview/demo smoke checks pass. The existing Qt/GCC header warning remains non-fatal.
- Four unsigned, disposable fixture RPMs built and were queried without installation. The deliberate failure fixture's pre-uninstall script exits 42. The polkit XML validates against the installed local policy DTD. Neither the helper nor policy was installed on this working host.
- No package or service mutation was executed. Libvirt daemon access is denied and `/dev/kvm` is absent in this session. Real polkit/SELinux behavior, authenticated root replay, successful service changes, native lock handling and power-loss/interrupted recovery remain **not run**. See [the VM procedure](fedora-vm-testing.md).

See [the boundary/lifecycle review](management-validation.md), [machine-readable evidence](verification/milestone3.json) and [milestone checkpoints](milestones.md). No production/security certification, package release or Milestone 4 work is implied.

---


## Milestone 2 — read-only dpkg inspection, 26 October 2026

- 44 distinct tests passed across regular and opt-in runs: 28 unit, seven RPM/live, five provider-contract, three dpkg integration and one compile-fail test. The real-root test was repeated for Ubuntu 24.04 and Debian 12/13. Environment-dependent tests remain opt-in in the ordinary suite.
- Native synthetic dpkg-query fixture: multilib/opaque versions, held/partial state, Depends/Pre-Depends/alternatives/virtual Provides, maintainer, unknown metadata, hidden/escaping desktop paths, service associations and ownership warnings. Inspection leaves the database bytes unchanged; mutations are unsupported.
- Real clean base-image filesystems were inspected read-only under Fedora. Provider identity/version/size maps matched each image's own dpkg-query executable and dpkg database hashes stayed unchanged. These are **not booted Ubuntu/Debian systems** and do not establish desktop compatibility.

| Filesystem | Present packages | Image dpkg-query | Match | Database unchanged |
|---|---:|---|---|---|
| Ubuntu 24.04.4 | 92 | 1.22.6 | Yes | Yes |
| Debian 12 | 88 | 1.21.22 | Yes | Yes |
| Debian 13 | 78 | 1.22.22 | Yes | Yes |

Machine-readable source-layer identifiers and database hashes are in [the database verification record](verification/dpkg-base-images.json). The snapshots contain package metadata only, not the developer's account/system report.

- All existing Fedora RPM tests passed, including 3,022 identities, metadata, reverse candidates, application/storage and account reports, plus DNF plan preparation/discard without executing a transaction.
- Native Qt/Rust desktop build passed. Fedora overview capture reported 3,022 packages, 50 app packages and 1,354 library-classified packages. Demo startup passed. The existing Qt/GCC header warning remains non-fatal.
- Full build/startup on Ubuntu/Debian desktops, real graphical application ownership, display-session behaviour and ARM remain unverified. The user has offered to help with [native desktop testing](ubuntu-debian-testing.md).
- No package installation/removal, service changes, or privileged authentication flow was exercised. These Milestone 2 checks preceded the subsequently approved Milestone 3 work above.

---


## Milestone 1 — shared provider architecture

- All 36 tests passed: 23 unit, seven live, five provider-contract and one compile-fail ownership test.
- Selected-provider inventory preserved all 3,022 native RPM identities. Fresh metadata, dependencies and reverse candidates were checked through the interface.
- A provider-backed DNF removal review was prepared and discarded; installed package identities were unchanged. No removal or service mutation was executed.
- Native live overview/removal-review captures and demo startup passed after the QML bridge moved to provider-neutral IDs, versions and metadata. The native build still has the existing non-fatal Qt/GCC header warning.
- At this historical checkpoint Fedora/RPM was the only implemented provider. Milestone 2 below records the subsequently approved dpkg work. See [the milestone record](milestones.md).

## Startup detection and capability gates — 26 October 2026

- All 25 tests passed: 20 unit tests and five live checks, including family detection, derivatives, immutable/transactional/declarative systems, missing tools/metadata, sandbox blocking, non-systemd identification and partial-scan behaviour.
- Simulated Ubuntu discovery verifies that RPM is not invoked and System/Storage values survive unsupported package discovery. A simulated RPM failure verifies that management is disabled while system details remain intact.
- Existing live Fedora RPM inventory, account/hardware discovery and non-mutating DNF preview tests remain green. No packages or services were changed.
- Native desktop build and Fedora startup/render check succeeded. Other operating systems are represented by fixtures, not live compatibility tests.
- Detection is a first foundation: no APT/pacman/apk/Nix package backend is implemented by this milestone.

## System tab — 26 October 2026

- Added a native System sidebar page with ten read-only detail sections, live summary cards, search, expandable cards and selectable plain-text values.
- The account collector identifies the real UID and displays username, display name, UID/GID, group membership, home directory, login shell and available desktop/session values.
- All 17 tests passed (12 unit tests and five opt-in live tests), including os-release parsing without expansion, memory units/missing values, UID-based passwd selection, nested CPU JSON and a live account/hardware report check. Earlier inventory and non-mutating DNF preview checks remain green.
- Native desktop build and System-page capture succeeded. The rendered page was inspected, including the Account & session card. Restricted firmware/network sources display unavailable values; no elevated permissions are requested.
- Actual machine/account screenshots remain outside the repository. Demo mode does not collect the real report. This tab does not change account, network or system settings.

## Native dashboard and management pass — 26 October 2026

- Desktop compiled successfully with Qt 6.11.2 and CXX-Qt 0.8.1. The existing Qt/GCC QChar/SFINAE header warning remains non-fatal.
- All 12 core and integration tests passed: XML/metadata preservation, invalid-record handling, architecture-less key records, demo fixture, desktop-entry visibility, disk metrics including shared mounts, removal plan validation, protected packages/demo actions, uploaded Fedora fixture, live RPM inventory/reverse lookup, live application/storage discovery and DNF non-mutating preview.
- Live scan returned 3,022 RPM entries, 50 packages with visible desktop applications, 1,354 library-classified packages and 175 development-classified packages. Categories are discoverability heuristics; all RPM entries remain listed.
- The default source is the installed RPM database, RPM file ownership, desktop entries and mounted filesystems. Only explicit `--demo` uses the synthetic fixture. JSON remains a serialization detail at the Qt/Rust boundary.
- Native offscreen render and capture succeeded for Overview, Storage and the real DNF removal-review dialog. Captures use the application's QQuickWindow, not a browser mock. The test also exercises category filtering and search. The README screenshot is an actual live native render.
- A DNF preview for gcc-c++ included six removals: gcc-c++, clang, three Qt development packages and qt6-rpm-macros. The test verified the installed inventory was identical before and after preparing the plan. No transaction was replayed.
- **Actual package removal, polkit authentication and service startup mutation have not been executed in this development session.** They are implemented behind explicit confirmation and should next be exercised on a disposable Fedora VM. No packages or services on this machine were removed, enabled or disabled to test the new code.
- Qt font resources load bundled Hanken Grotesk and official Remix Icon 4.6.0 glyphs. Their original licenses are included.

Current limits: Fedora/RPM sources only; no Flatpak, Snap or manually installed application discovery. Startup controls are for eligible non-template systemd services, not a generic app-disable switch. Repository origins/available updates, query cancellation/timeouts, native list-model scalability, complete dependency graphs and broad accessibility/manual QA remain follow-ups. Package-installed size is distinct from real disk usage. No installable RPM or binary distribution is produced by this pass.

The current source ZIP is verified by integrity checks, extraction and source tests. The initial GitHub v0.1.0-starter release remains a historical archive; it does not automatically change when local source changes.

---

# Initial starter — 25 October 2026

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
