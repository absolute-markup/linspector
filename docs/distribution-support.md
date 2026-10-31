# Cross-distribution support plan

Designed and developed by Absolute Markup.

## Product direction and current status

Linspector targets broad Linux desktop distribution support with a consistent native Qt interface. This is a development requirement, not a current compatibility claim. The current implementation is verified on Fedora 44 with RPM/DNF5 and systemd. Startup identification and conservative capability gating are now implemented. The read-only dpkg provider is also implemented; native-query results match Ubuntu 24.04 and Debian 12/13 base-image databases. Full application startup on those desktops remains pending. Read-only pacman/ALPM and Snap adapters and Zypper removal dry-run reviews are now implemented with fixtures; Flatpak has live Fedora validation. Full native validation for Arch/Snap/openSUSE and Zypper authenticated execution remain open.

Support is recorded per distribution release, architecture, software source and capability. Being able to launch, inspect packages and change packages are separate milestones. Unknown environments must never be treated as safe management targets.

## Architecture

### Independent system discovery

Operating-system, hardware, account, memory, network and mounted-storage discovery must run independently of package discovery. `system::scan()` now collects System and Storage independently and preserves those results if RPM discovery fails or no package provider is available. Missing commands, restricted firmware or unsupported package managers should not prevent available System/Storage details from loading.

Use portable Linux interfaces where possible, optional probes where necessary, explicit unavailable values and timestamps. Detect process namespaces/containers when relevant so the app does not label a container view as the complete host. Preserve demo mode isolation from real account/hardware discovery.

### Software-source providers

Introduce provider interfaces for discovery, native package identity, metadata, dependency inspection, supported operations, transaction preview and transaction execution. A common data model must preserve native IDs and source scope. Package versions stay opaque to the UI; comparisons and dependency resolution belong to the native provider.

Initial provider families:

| Family/source | Discovery direction | Management direction |
|---|---|---|
| Fedora and related mutable RPM systems | RPM database | Explicit DNF4/DNF5 capabilities and tested native solver behaviour |
| Debian, Ubuntu and derivatives | dpkg/APT metadata | APT-native transaction planning and execution |
| Arch and derivatives | pacman/libalpm metadata | Native transaction handling with distribution-specific validation |
| openSUSE | RPM metadata | Zypper/libzypp; never assume DNF because RPM is present |
| Flatpak | User and system installations, application/runtime refs | Flatpak operations with explicit scope and runtime/dependant review |
| Snap | snapd metadata, where available | snapd operations and its own transaction/status model |
| Alpine | apk database | apk-native semantics; separate libc/desktop builds and OpenRC handling |
| Nix/NixOS | Profiles, generations and declared ownership | Separate user-profile operations from declarative system changes |
| Gentoo/Void and other families | Portage/XBPS or another dedicated provider | Added only with native transaction semantics and tests |
| AppImage | Explicit discovery locations or user registration | Manage only tracked files/integration; no claim to discover every portable binary |

An application and a package are related objects, not interchangeable identities. One application may involve many packages; one package may expose multiple launchers or services. Category tags are discoverability aids and never determine removal safety. Multiple software sources can coexist on the same machine; keep origin and user/system scope visible rather than deduplicating by display name.

### Capability-driven UI

Each provider reports which operations it supports and why others are unavailable. Render a consistent interface while respecting native semantics:

- Inspection can be available before package changes are supported.
- Unsupported actions are disabled with a useful explanation; never fall back to deleting files.
- Removal previews use the provider's solver and expose all planned changes, including dependencies and reboot/deployment consequences.
- Confirmation is bound to the reviewed plan; stale or changed state requires new review.
- Authentication, cancellation, progress and failures have explicit states.
- No universal “disable package” operation exists. Application autostart, service startup and package removal are distinct capabilities.

### Service and privilege providers

Service discovery/control must be separate from software-package discovery. Detect systemd, OpenRC, runit or an unknown service environment. Unknown environments retain inspection where possible and expose no guessed service-changing commands. Treat user services and system services distinctly.

Design and audit a narrow privileged-operation boundary. Do not carry the prototype's broad command execution unchanged into every backend. Tests must cover argument validation, native identity resolution, authentication denial, state changes after preview, process failures and interrupted operations.

### Immutable and declarative systems

Detect the host's management model before enabling actions. Fedora Atomic/rpm-ostree, bootc and transactional systems need their own adapters. Base image content, layered packages, user applications and pending deployments are different objects; removing an item may require a new deployment or reboot.

NixOS system packages can come from declarative configuration, while user-profile packages have separate management semantics. Do not present a conventional mutable-package removal action for configuration-owned system software. Begin with inspection and explanations until a tested workflow exists.

## Rollout

1. **Milestone 1 implemented and approved:** startup detection, capability policy, independent System/Storage loading, provider-neutral records and the shared provider contract are implemented. Existing Fedora/RPM behaviour now runs through the RPM adapter. See [milestone checkpoints](milestones.md).
2. Establish a named test matrix for mutable Fedora, Ubuntu/Debian, Arch and openSUSE. Bring up inspection first, then validated operations per provider.
3. Add Flatpak/Snap sources and test mixed-source machines and derivatives individually. Similar package managers do not prove compatibility.
4. Add immutable/declarative adapters and additional package/service families, initially with clearly stated inspection-only support.
5. Expand architectures and distribution packages only with builds and clean-system tests. x86_64 and aarch64 require separate evidence; musl/glibc compatibility must be treated explicitly.

GNOME/KDE and Wayland/X11 coverage, high-DPI/text scaling, non-systemd environments and accessibility belong in the compatibility matrix. Packaging formats such as AppImage or Flatpak do not by themselves provide host package-management compatibility.

## Release quality gates

The 10/10 quality ambition is translated into measurable acceptance gates, not a claim of perfection:

- Named, tested distribution/version/architecture support rather than “works everywhere”.
- Complete, source-attributed inventory within the declared scope, including multilib and parallel versions.
- Native solver-backed previews and verified privileged changes in disposable machines; no destructive testing on the user's working system.
- Tested failure recovery, locks, authorization denial, stale plans and cancellation behaviour.
- Keyboard/screen-reader and display-scaling checks, readable typography and measured contrast.
- Responsive behaviour with large inventories and bounded background work.
- Reproducible builds, signed releases, dependency/license records and supported update paths.
- User-facing documentation that states supported capabilities and limitations accurately.

## Primary references

- [Fedora Atomic software and package layering](https://docs.fedoraproject.org/en-US/fedora-silverblue/getting-started/)
- [NixOS package management](https://nixos.org/manual/nixos/stable/#sec-package-management)
- [Alpine's musl, apk and OpenRC architecture](https://alpinelinux.org/about/)
- [Arch pacman](https://wiki.archlinux.org/title/Pacman)

## Startup detection milestone

Implemented in `platform.rs`: distribution ID/family/release from os-release, package-system identification, installed-tool checks, image/transactional/declarative markers, service-manager evidence and sandbox/container checks. `/etc/os-release` takes precedence over `/usr/lib/os-release`; values are parsed as data and never sourced in a shell. ID and ID_LIKE identify candidate providers; an installed DNF/RPM tool alone never selects that provider on Ubuntu, Arch or an unidentified system.

Removal preview remains available only for mutable Fedora 44 with the required native tools, a successful inventory and no detected sandbox/container. Execution and service changes additionally require the trusted installed management helper and matching policy. Management APIs recheck the platform before preview, replay and startup changes; UI controls are also gated. Unknown releases and derivatives are inspection-only until separately validated. Detection is not a security boundary against a malicious local system, and does not replace the implemented helper checks or the still-pending real management validation. See [the boundary review](management-validation.md).

RPM-family inspection can operate where RPM is available. APT/dpkg inspection is implemented, with package/service changes disabled. Its base-image database validation does not establish full desktop compatibility. Pacman/ALPM and Snap inspection, Flatpak inspection and Zypper dry-run removal review are implemented; broader native runtime validation and management remain pending. Apk, Nix, Portage and XBPS adapters are still pending. See [Milestone 4 evidence](milestone4-integrations.md). Recognition of those families in fixtures is not validation on those operating systems.

If package inventory cannot load, it is labelled unavailable rather than reported as zero installed packages. System/Storage remain available, package actions are disabled, and no stale inventory is used to authorize changes. Runtime app startup, graphics, available tools and libc compatibility still need tests on each target distribution.
