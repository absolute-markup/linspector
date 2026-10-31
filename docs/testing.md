# Help test Linspector

Designed and developed by **Absolute Markup**.

Thank you for helping test this early native Qt Quick/QML + Rust application. We need successful results as well as bugs from **Fedora, Ubuntu, Debian, Arch Linux and openSUSE**. Testing one system or one feature is useful; you do not need to complete the whole matrix.

## Get the current source

The old `v0.1.0-starter` release predates the current providers. Use the repository's current `main` branch:

```sh
git clone https://github.com/absolute-markup/linspector.git
cd linspector
git rev-parse HEAD
```

Record that full commit in your report. Alternatively, download the [current source ZIP](https://github.com/absolute-markup/linspector/archive/refs/heads/main.zip) and record its download date plus the commit shown on GitHub. Source builds are currently required; there is no current installable release.

## Choose your test

| Environment | Start here | Scope |
| --- | --- | --- |
| Fedora 44 desktop | [Fedora build guide](development.md#build-and-run-on-fedora) | Normal startup, RPM inventory, System/Storage and read-only removal previews |
| Ubuntu 24.04 / Debian 12 or 13 desktop | [Ubuntu/Debian guide](ubuntu-debian-testing.md) | Build/startup, dpkg inventory, ownership and disabled management actions |
| Arch desktop | [Integration guide](milestone4-integrations.md) | Build/startup, local ALPM inventory and native pacman comparison |
| Mutable openSUSE desktop | [Integration guide](milestone4-integrations.md) | RPM inventory and native Zypper dry-run reviews; execution remains unavailable |
| Desktop with Flatpak or a working Snap daemon | [Integration guide](milestone4-integrations.md) | Source status, installed refs/revisions, metadata and relationships |
| Disposable Fedora 44 desktop VM | [Management test procedure](fedora-vm-testing.md) | Authentication, test-package removal, service startup, staleness and failure recovery |

A booted desktop VM with a graphical login is suitable. Containers and offline package databases can help test data, but do not establish desktop, authentication or service compatibility. Immutable/transactional systems are future dedicated work; record unsupported behavior without bypassing capability checks.

## Build and launch

Use Rust/Cargo **1.88 or newer**, a C++17 compiler, pkg-config and Qt 6 development tools/libraries for Qt Quick, QML and Quick Controls. QML runtime modules may be separate distribution packages. Use the distribution-specific prerequisites linked above. Complete Arch/openSUSE setup recipes are not yet validated; report missing prerequisites or build failures rather than treating those systems as proven compatible.

Record these versions:

```sh
rustc --version
cargo --version
qmake6 -query QT_VERSION
```

After installing prerequisites on your test system:

```sh
export QMAKE=/usr/bin/qmake6
cargo test --locked -p linspector-inventory
cargo build --locked -p linspector
./target/debug/linspector
```

If `qmake6` lives elsewhere, set `QMAKE` to its actual path. First builds download dependencies and require network access. Installing build prerequisites changes the test system separately from Linspector inspection.

Run the application as your ordinary account, **without sudo and without `--demo`**. Demo mode is synthetic and cannot validate real package discovery. Do not install the management helper on a working machine for this testing call; its setup and package-changing cases belong in the disposable-VM guide.

## Everyday inspection checklist

1. **Detection and startup:** confirm the correct distribution/native provider, no QML startup errors and a responsive window.
2. **Inventory:** inspect an installed application, library and development package. Compare native names, versions, architectures and metadata with your package manager. Record each source separately; combined counts include distinct installations/revisions, not unique products.
3. **Relationships:** inspect requirements and reverse candidates. Keep the displayed limitations in mind; candidates are not a removal-safety decision.
4. **Sources:** check Flatpak/Snap if installed. A failed source should show unavailable/unknown, preserve other inventory and avoid reporting a false zero. Source switches should clear incompatible selections.
5. **System and Storage:** check distribution/account details, memory, mounted filesystems and space. Package-size estimates need not equal allocated disk usage.
6. **Usability:** try searching, resizing, keyboard navigation and your normal display scaling/theme. Record unreadable, clipped or inaccessible controls.
7. **Refresh:** rescan and confirm the interface remains usable and shows current results. Avoid concurrent package transactions when comparing snapshots.
8. **Capabilities:** dpkg/pacman/Flatpak/Snap management must remain unavailable. openSUSE execution must remain unavailable. Fedora without the trusted helper must permit no actual removal or system service change.

For native comparisons, use your package manager: RPM queries for Fedora/openSUSE, `dpkg-query` for Ubuntu/Debian, `pacman -Q` / `pacman -Qi` for Arch, `flatpak list --all` and `snap list --all`. Ordinary native listings can omit extensions or count revisions differently; report identity differences rather than assuming every count difference is a bug. The detailed guides explain the provider semantics.

## Optional screenshot check

After testing a normal interactive launch:

```sh
./target/debug/linspector --smoke --capture /tmp/linspector-test.png
```

This checks scan/render startup and exits. An offscreen capture is useful diagnostics but does not replace a real desktop launch. Review images before sharing; the System page includes account and network information.

## Report your result

Use the [testing-report form](https://github.com/absolute-markup/linspector/issues/new?template=testing-report.yml). It accepts passing, failing, partial and blocked tests. Include:

- Commit, distribution/version, architecture and desktop/session/display scaling.
- Rust/Cargo/Qt and relevant package-manager versions.
- Build/startup result and tests actually performed; mark others **not run**.
- Native versus Linspector identities/counts, separated by source, with collection time if relevant.
- Expected and observed behavior, reproduction steps and relevant error excerpts.

Redact usernames, home paths, hostnames, IP/MAC addresses, private repository URLs and other personal details. Never post credentials or an unreviewed full System/inventory dump. A short relevant error excerpt is usually enough.

Management reports must identify the disposable guest, snapshot baseline and exact reviewed/observed changes. Report unexpected changes or ambiguous outcomes as failures; do not retry automatically or claim rollback. Follow the VM guide for recovery cases.

We review reports, implement fixes and request retests before closing a milestone. A report on one version or machine does not establish compatibility with every distribution or release.
