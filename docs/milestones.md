# Implementation milestones and approval checkpoints

Designed and developed by Absolute Markup.

Working agreement: explain the milestone, implement its scope, verify it and report the evidence. Wait for the user's explicit approval before starting the next milestone. Completing a checkpoint does not authorize the next one.

| Milestone | Scope | Status |
|---|---|---|
| 1. Shared package-provider architecture | Provider-neutral records and interfaces; move existing RPM/Fedora operations behind them; preserve live behaviour | Implemented and verified; approved by user |
| 2. Ubuntu/Debian inspection | Read-only APT/dpkg inventory, native metadata/identities, relationships and ownership; clean Ubuntu/Debian validation | Implementation and database verification passed; live desktop validation pending |
| 3. Management validation | Disposable-VM tests for authentication, removal, service changes, cancellation, stale previews and recovery; review the privilege boundary | Approved by user; helper/lifecycle implementation and automated verification; real VM cases pending |
| 4. Broader software integrations | Arch/pacman, openSUSE transactions, Flatpak/Snap with explicit source/scope | All integrations authorized; inspection adapters and Zypper dry-run review implemented; runtime/management gaps remain |
| 5. Specialised systems | Dedicated immutable/transactional/declarative workflows and additional distribution families | Not started; separate approval required |
| 6. Release quality | Accessibility, scaling, performance, compatibility matrix, clean installs and packaging | Not started; separate approval required |

## Milestone 1 — completion record

### What it enables

The desktop and scan coordinator use a common `PackageProvider` interface rather than direct RPM queries or DNF calls. A new provider can supply inventory, metadata, dependencies, capability information and supported removal previews without imposing RPM's epoch/release layout on every package.

### Implemented

- `model.rs`: provider/scope/native identity, collision-safe opaque UI keys, provider-neutral metadata, explicit optional values and native display versions. Native metadata remains namespaced for lossless RPM details.
- `providers/mod.rs`: object-safe package-provider contract, factory selection, typed errors, capability reports and an opaque prepared-removal interface.
- `providers/rpm.rs`: the existing RPM/Fedora adapter. Inventory, metadata lookup, dependency candidates and DNF plan preparation route through this adapter.
- `providers/rpm_query.rs`, `rpm_catalog.rs`, `dnf.rs`: isolated native RPM parsing, ownership/launcher discovery and DNF plan implementation.
- `services.rs`: service operations remain separate; package/service ownership is verified through the selected package provider before changes.
- The QML bridge carries provider-neutral records and executor labels. It no longer reconstructs NEVRA IDs or requires RPM epoch/release/source-RPM fields.
- Prepared plans are owned server-side in Rust, cleaned up when discarded and consumed on execution. QML receives review data, never plan file paths or executable commands.
- The original RPM JSON CLI API remains available through documented compatibility exports. It is not the interface for new providers.

### Verification

- 36 tests passed: 23 unit tests, seven live checks, five provider-contract checks and one compile-fail ownership check.
- A live provider inventory was compared against the native RPM inventory: the same 3,022 identities were retained. Metadata, size and required capabilities were also compared.
- Live reverse-dependency candidates remained scoped RPM identities and matched installed packages.
- A real DNF removal plan was prepared through the provider, reviewed and discarded. The selected identity matched and the installed inventory was unchanged before and after. No transaction was executed.
- Contract tests use an in-memory, non-RPM-shaped fixture. They test source/scope separation, opaque versions, missing metadata and read-only operation defaults; this fixture is not an APT backend.
- Wrong-provider IDs, foreign scopes and unsupported removals are rejected. The factory does not grant DNF management to openSUSE or select RPM on Ubuntu/Arch/unknown systems.
- The prepared-plan type cannot be executed twice; a Rust compile-fail test verifies consumed ownership.
- Native desktop build, live overview/removal-review captures and demo startup succeeded. No packages or services were changed.

### Remaining limits

Only the existing RPM adapter is implemented. No APT/dpkg code, Ubuntu/Debian runtime claim, new native package format or marketplace publication is part of this milestone. Native mutation/authentication and failure recovery still require Milestone 3. The existing privilege design and JSON list transport are preserved rather than certified production-ready.

### Checkpoint

User approved Milestone 2. The Milestone 1 completion record above describes that historical checkpoint; its RPM-only scope does not describe the current provider set.


## Milestone 2 — implementation and verification checkpoint

### What it enables

On an identified Debian/Ubuntu-family host with dpkg-query available, the app selects the read-only dpkg provider automatically. This includes ordinary installed packages, libraries, development packages and partially installed packages; it does not filter inventory to graphical applications.

### Implemented

- Native dpkg-query status and file-ownership queries; no APT or dpkg mutation path.
- Provider/scope/native identities, multilib preservation and opaque Debian versions, including epochs, tildes and binNMU revisions.
- Summary, full description, architecture, estimated installed size, source package, homepage, maintainer and native state. Unrecorded repository, license and installation time remain unavailable.
- Depends, Pre-Depends and Provides, retaining alternatives and version/architecture qualifiers. Reverse results are explicitly candidates, not native solver results or removal-safety evidence.
- Visible package-owned desktop entries and eligible systemd file associations. Hidden launchers and paths escaping an offline root are excluded. Incomplete ownership produces warnings without discarding valid unrelated entries.
- The inspector displays dpkg maintainer/state; dependency explanations come from the selected provider.
- Bounded read-only queries, explicit database/pager options and separate offline-root inspection for verification. Missing/malformed databases fail explicitly. Fedora DNF actions are never inherited by the dpkg provider.

### Verification performed

- 44 distinct Rust tests passed across the ordinary suite and opt-in runs: 28 unit, seven RPM/live, five provider-contract, three dpkg integration and one compile-fail ownership test. The real-root integration test was exercised against three separate databases.
- Native fixture queries cover multilib, held and partially installed states, alternatives, virtual Provides, hidden/escaping launchers and missing ownership data. Database bytes remained unchanged during inspection.
- Ubuntu 24.04.4 (92 packages), Debian 12 (88) and Debian 13 (78) base filesystems: provider identities, versions and installed sizes matched each image's own dpkg-query binary. Database hashes were unchanged, with no ownership warnings.
- These are extracted, clean base-image filesystems queried under Fedora, **not booted distributions or desktop compatibility tests**. They have no desktop applications; graphical ownership behaviour is covered by the native synthetic fixture.
- Native Qt/Rust build, Fedora live capture (3,022 packages) and demo startup passed. All existing Fedora read-only and prepare/discard DNF-preview regressions passed. No packages or services were changed.

### Remaining validation and checkpoint

The user has offered to test on Ubuntu/Debian. Follow [the native testing guide](ubuntu-debian-testing.md) to check build/startup, real desktop package ownership, System/Storage and disabled actions. This live desktop validation remains open; data verification does not substitute for it. Derivatives, older dpkg versions, other architectures and full desktop/display combinations are not claimed as validated.

The user subsequently approved Milestone 3. Its record below preserves the distinction between implemented checks and missing VM evidence. APT management remains future provider work; Milestone 2 does not promise a native transaction implementation.


## Milestone 3 — implementation and verification checkpoint

The user approved this scope and offered their Fedora laptop for testing. Ordinary startup, read-only scanning and previews can be checked there. A disposable desktop VM is still required for the prepared mutation/failure tests; no test packages, helper or polkit policy were installed on the working host.

### Implemented

- Replaced direct elevated DNF/systemctl invocation with a narrow, versioned stdin protocol and a root-owned non-setuid helper/polkit policy. Unsupported or untrusted installations retain previews with execution disabled.
- The authenticated helper checks the reviewed installed snapshot, generates a fresh private root-owned native DNF plan, compares the exact removal set, then replays with native mismatch checks intact. User-owned transaction paths never enter the privileged request.
- Exact service-package ownership, loaded fragment/drop-in restrictions, eligible startup states and before/after checks. Multi-unit install effects and core/runtime services are refused.
- Generation-bound review ownership rejects cancelled/late/selection-mismatched plans and consumes confirmation once. UI rescans after every action outcome; normal closing is blocked during an action.
- Bounded output and deadline/reaping for read-only management probes; no forced timeout/kill for active mutations. Failure/unknown outcomes require observation and new review, without automatic retry or rollback.
- Four disposable fixture RPMs and a repeatable desktop-VM test matrix for authentication, removal, startup, staleness and recovery.

### Evidence and missing gate

See [management validation](management-validation.md) and [verification notes](verification.md) for executed checks. Fault-injection and real unprivileged helper rejection tests cover application behavior, not actual polkit authorization or privileged DNF/systemd execution. Test RPMs were built and inspected only. Fedora read-only inventory/preview regressions and native UI capture pass.

Libvirt daemon access is denied in this session and `/dev/kvm` is absent. The [VM procedure](fedora-vm-testing.md) is prepared but its authenticated/mutating cases have **not been run**. Root replay, real cancellation/denial, successful service changes, SELinux and interrupted-transaction recovery remain open. This milestone is **not complete** until that evidence is collected and failures are resolved.

The user subsequently approved all Milestone 4 integrations in one pass. The Milestone 3 gaps above remain open.


## Milestone 4 — software integrations checkpoint

The user approved Flatpak, Snap, Arch/pacman and openSUSE work together. Implemented multi-source discovery with per-source status, per-entry capabilities and provider-bound routing; Flatpak and Snap native UI views; installed Flatpak refs/commits/sizes, Snap revisions/components and local ALPM metadata/ownership; and strict Zypper removal dry-run reviews. The existing native Qt Quick/QML + Rust/CXX-Qt architecture is preserved.

Flatpak passed live Fedora and isolated native scope/branch checks. ALPM, Snap API/framing and Zypper XML checks use synthetic/source-derived fixtures; real Arch, reachable snapd and native openSUSE execution were not available here. Snap socket access is denied. No working RPM/Flatpak/Snap installation was changed and no fixture application was launched.

Zypper authenticated execution is not implemented: a safe executor needs to keep the reviewed transaction bound to the native solver lock and be verified on openSUSE. This is not hidden behind a noninteractive remove command or a DNF fallback. Flatpak, Snap and pacman remain read-only. **Milestone 4 is not fully complete** while these native runtime/management gaps remain.

See [integration semantics and test procedure](milestone4-integrations.md) and [verification](verification.md). Milestone 2/3 gaps remain recorded. Milestone 5 has not started and requires separate explicit user approval.
