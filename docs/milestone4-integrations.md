# Milestone 4 — software integrations and validation

Designed and developed by Absolute Markup.

The user approved all Milestone 4 integrations in one pass. Flatpak, Snap and pacman inspection and openSUSE/Zypper removal reviews are implemented. **This is an implementation checkpoint, not full cross-distribution/management validation.** Arch and openSUSE runtime tests, a reachable real Snap daemon and authenticated Zypper execution remain open. Milestone 2/3 validation gaps remain recorded separately.

| Integration | Implemented | Evidence here | Limit |
|---|---|---|---|
| Flatpak | User/default-system/named-system refs, apps/runtimes/extensions, branches, commits, sizes, runtime candidates | Native Flatpak 1.18.4 on Fedora; 91 installed refs; isolated 12-ref native fixture | Read-only; older CLI fallback parsing tested, full older-version startup not tested |
| Snap | Local snapd GET installed inventory, active/inactive/local revisions, bases and installed component attachments | Synthetic API/schema and HTTP framing tests | Daemon socket access is denied here; real daemon/IPC integration not validated; read-only |
| Arch/pacman | Local ALPM installed metadata, ownership, dependencies and reverse candidates | Synthetic ALPM v9 filesystem, locks, links, metadata and ownership tests | No booted Arch or native pacman comparison here; read-only; default DB path only |
| openSUSE | Existing RPM inspection plus a native Zypper removal dry-run adapter and strict XML review validation | Source-derived XML format and negative/contract tests | Zypper binary/native runtime absent here; authenticated execution is not implemented |

## Independent sources and routing

A scan retains per-provider status and an optional count. A failed source is unavailable with an unknown count; a successfully queried empty source is zero. Another source's entries are preserved. A partial Flatpak/Snap scan does not silently revoke or confer native RPM capabilities. System and Storage remain independent.

The bridge dispatches metadata/dependency/removal requests by the selected provider and checks per-entry capabilities. Flatpak, Snap and pacman have no mutation implementation. Flatpak/Snap entries never fall back to RPM/DNF. The privileged helper protocol remains limited to the previously implemented Fedora/system-RPM actions.

The native Qt interface includes Flatpak and Snap views, source status, scope/ref/branch/state details, source-aware search and source-labelled list rows. Application counts refer to entries, not deduplicated products; the same title in several sources/scopes remains several distinct entries. Demo mode remains synthetic and does not collect the real inventory.

## Flatpak semantics

The provider uses read-only `flatpak list --all --app` and `--runtime` queries with explicit columns, preferring JSON. It falls back to full-column TSV only when the JSON option is unsupported; permission/malformed-data failures are errors. `--all` preserves locale/debug/extension refs that ordinary listings can hide.

Native refs retain `app/` versus `runtime/`, ID, architecture and branch. User scope includes the real UID. Named installations printed as `system (NAME)` are routed with the matching `--installation=NAME`; default system and user use their own selectors. The invoking user's native HOME/XDG/Flatpak installation configuration is retained. Other users' private installations are not enumerated.

Human-rounded size columns are retained as display metadata only. Exact byte estimates and full commits come from `info --show-commit --show-size` using the exact scope/ref. Four bounded workers and a detail-scan budget prevent an unbounded queue. Missing detail results stay unknown with warnings; they are not zero. Unknown publisher/license/source-package/install-time fields are not invented. A ref identifies a branch; commit metadata identifies its current deployment.

Dependencies show declared app runtime refs and same-installation candidates. SDK/build relations, extension attachment and cross-installation runtime fallback are not solver-resolved. No uninstall/override/run command occurs in the provider.

The isolated fixture builds a tiny local repository, installs only into an explicit temporary user root and clones those synthetic deployments to temporary default/named system roots. Those clones test native read routing, not privileged installation. It never launches the fixture commands. Native cache/export-trigger sandbox warnings were recorded; sandbox checks were not disabled.

## Snap semantics

Only `GET /v2/snaps?select=all` is sent to `/run/snapd.socket`, without credentials or store queries. Bounded HTTP/JSON handling supports content-length and chunked bodies; daemon/permission/schema errors are unavailable results, never empty successes.

Snap names/revisions remain distinct, including inactive installed revisions and local `xN` revisions. Components with revisions are listed as installed attachments; available-only components are excluded. Attachments include the parent revision in their identity. Snap sizes exclude app data and may describe different storage accounting from RPM/Flatpak sizes. Missing architecture/date conversions remain unknown or raw native metadata.

Only reported base-name and component-parent relationships are exposed. Content interface connections, automatic connections and revision-aware resolution are not claimed. Enable/disable, removal, permissions and service changes are unavailable. Fixture exports have `fixture:system` scope and explicit synthetic provenance.

## Pacman semantics

`providers/pacman.rs` reads `/var/lib/pacman/local`, with supported layout version 9, and parses percent-delimited desc/files records as data. It does not invoke pacman transactions or execute install/mtree/desktop/service scripts. Missing/unsupported DBs, locks, malformed/duplicate identities and escaping DB links fail explicitly. Ownership failures preserve known package records with warnings. Optional sizes remain unknown and `%SIZE%` is bytes.

Epoch/pkgrel versions, package base, licenses, packager, install time/reason, provisions and dependencies remain available. Optional dependencies and validation/XDATA remain native metadata. Desktop/service associations use rooted owned paths. Reverse candidates ignore version constraints and are not a libalpm solver result. Custom pacman DBPath configuration and full Arch desktop runtime still need validation.

## Zypper review boundary

On an identified mutable SUSE-family host with RPM/Zypper present, the adapter attempts an unprivileged XML removal dry-run with repository refresh/loading disabled, no automatic cleanup, package type and exact version/architecture selection. Native errors remain errors; no elevation is used to rescue a failed preview.

The parser accepts one complete removal summary, resolves every row to one exact installed RPM and rejects installs/upgrades/other changes, ambiguous identities, duplicates, wrong counts, protected packages, errors and missing selected packages. `PreparedRemoval.execute()` returns Unsupported. There is no DNF fallback.

**Why execution stays disabled:** a dry-run in one process followed by an automatically accepted remove in another could resolve different dependencies after state changes. A production executor must bind the reviewed set to the native solver lock/confirmation and be tested on real openSUSE. The existing Fedora helper is not broadened to pretend that this is solved. Transactional/immutable SUSE systems also need the separate Milestone 5 workflow.

## Repeat the checks

```sh
cargo test --locked -p linspector-inventory
cargo test --locked -p linspector-inventory --test live_rpm -- --include-ignored
cargo test --locked -p linspector-inventory --test flatpak_native live_inventory -- --ignored --nocapture
python3 scripts/create_flatpak_fixture.py /tmp/linspector-flatpak-fixture-new
LINSPECTOR_FLATPAK_FIXTURE_ROOT=/tmp/linspector-flatpak-fixture-new cargo test --locked -p linspector-inventory --test flatpak_native isolated_native -- --ignored
cargo run --locked -p linspector-inventory -- --flatpak
cargo run --locked -p linspector-inventory -- --snap
cargo run --locked -p linspector-inventory -- --snap-fixture fixtures/snap/installed.json
```

The fixture path must not exist. Its build/export/install commands are confined by explicit HOME/XDG/Flatpak paths to that new directory. `--snap` is expected to fail explicitly in this restricted session. `--snap-fixture` reads synthetic data only. To inspect an explicitly mounted/copied Arch filesystem read-only:

```sh
cargo run --locked -p linspector-inventory -- --pacman-root /path/to/arch-root
```

On real test systems, compare Arch entries/versions/byte sizes against `pacman -Q/-Qi`, Snap revisions against `snap list --all`, and Zypper dry-run summaries against the exact native command. Record release, architecture, versions and failures. Do not turn fixture checks into claims of full native compatibility or run package-changing tests on a working machine. Native Qt capture flags `--smoke --smoke-view Flatpak` and `Snap` exercise the actual UI, not a web mock.

## Primary references

- [Flatpak command reference](https://docs.flatpak.org/en/latest/flatpak-command-reference.html) and [1.18.4 native info implementation](https://github.com/flatpak/flatpak/blob/1.18.4/app/flatpak-builtins-info.c).
- [ALPM local desc format](https://man.archlinux.org/man/alpm-db-desc.5.en).
- [Canonical snapd REST API](https://forum.snapcraft.io/t/snapd-rest-api/17954), installed endpoint versus store queries and installed components.
- [Zypper manual](https://github.com/openSUSE/zypper/blob/master/doc/zypper.8.txt) and [native summary XML implementation](https://github.com/openSUSE/zypper/blob/master/src/Summary.cc).
