# Ubuntu/Debian native testing — Milestone 2

Designed and developed by Absolute Markup.

This is the remaining live desktop validation for the read-only dpkg provider. Use a normal, booted Ubuntu/Debian desktop, including a VM with a graphical login. A container does not establish desktop compatibility; automatic host inventory is deliberately unavailable in detected sandboxes/containers.

The source is native Qt Quick/QML + Rust + CXX-Qt. No WebView or Tauri is used. Fedora-built binaries are not a portable Ubuntu/Debian release: build from the current source ZIP or checkout on the test machine. The historical GitHub starter release does not contain Milestone 2.

## Prepare the build

Use Rust/Cargo **1.88 or newer** for the current lockfile (the locked cxx dependency requires it). An older distribution-provided Rust may be insufficient. Record `rustc --version`, `cargo --version` and `qmake6 -query QT_VERSION` with the test result. Dependency downloads require network access on the first build.

On Ubuntu 24.04 or Debian 12/13, the following are the expected build/runtime prerequisites. Installing these changes the test machine's packages; it is optional setup, separate from Linspector's read-only scan. This setup/build recipe itself still needs verification on those desktops.

```sh
sudo apt install build-essential cmake ninja-build pkg-config clang lld \
  qt6-base-dev qt6-declarative-dev qt6-tools-dev qt6-tools-dev-tools \
  qml6-module-qtquick qml6-module-qtquick-controls \
  qml6-module-qtquick-layouts qml6-module-qtquick-window \
  qml6-module-qtquick-templates qml6-module-qtqml-workerscript
```

Development libraries and QML runtime modules are separate packages. See the official [Debian Qt declarative development package](https://packages.debian.org/bookworm/qt6-declarative-dev), [Quick Controls QML module](https://packages.debian.org/bookworm/qml6-module-qtquick-controls) and [Workerscript QML module](https://packages.debian.org/bookworm/qml6-module-qtqml-workerscript). Ubuntu's [Noble Qt package catalogue](https://packages.ubuntu.com/search?keywords=qt6&suite=noble) lists its Qt development packages. Package availability does not prove Linspector build compatibility.

From the extracted `linspector` directory:

```sh
export QMAKE=/usr/bin/qmake6
cargo test --locked -p linspector-inventory
cargo test --locked -p linspector-inventory --test dpkg_native native_query_preserves -- --ignored
cargo build --locked -p linspector
./target/debug/linspector
```

Run as your ordinary account, without sudo and without `--demo`. If a build or QML module load fails, keep the error text so it can be fixed; a failed startup is a test finding, not evidence of compatibility.

## Check the live app

1. The sidebar identifies the real distribution and APT/dpkg. The source reports the **live installed dpkg database**, and package-changing actions are unavailable.
2. Search for `dpkg`, `libc6` and an installed `-dev` package. Check the native version, architecture, description, estimated size, source package, maintainer and package state. An absent `-dev` package is not a failure; use one already installed.
3. Compare an entry to `dpkg-query --status libc6`. Missing repository, license or installation date should say unknown/not recorded, rather than inventing a value.
4. Open dependencies and reverse candidates. Required alternatives and qualifiers should remain readable. The explanation must say reverse candidates do not resolve version/architecture constraints and do not establish removal safety.
5. If a graphical application is installed through dpkg, find it under Applications. Its launcher must be package-owned; Flatpak/Snap-only applications are outside this milestone.
6. Open System and Storage. Check the username/account and distribution details, available memory and mounted filesystem values. Scan again and confirm the app remains usable.
7. Review removal and service startup changes must remain unavailable on APT/dpkg. This milestone never asks for administrator authentication.

For a read-only package count reference:

```sh
dpkg-query --show --showformat='${db:Status-Status}\n' | awk '$0 ~ /^(installed|unpacked|half-installed|half-configured|triggers-awaited|triggers-pending)$/ { n++ } END { print n+0 }'
```

Linspector includes those present states, including held packages. Removed/config-files-only entries are excluded. Compare counts without an APT/dpkg transaction running concurrently. Partial states also produce a warning in the app.

Optional native startup/capture check, after the normal interactive launch:

```sh
./target/debug/linspector --smoke --capture /tmp/linspector-ubuntu-debian.png
```

This exits after the scan/render check. Offscreen mode may help diagnose graphics failures, but does not substitute for a normal desktop launch. System-page captures contain local account details and are unnecessary for the initial report.

## Report the result

Send the distribution/version, architecture, desktop/session (GNOME/KDE, Wayland/X11), Qt and Rust versions, whether build/startup passed, native count versus app count, and any errors or missing entries. This closes the live validation gap when the observed results support it. It does not authorize Milestone 3; that remains a separate approval checkpoint.
