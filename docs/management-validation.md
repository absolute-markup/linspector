# Milestone 3 — management boundary and validation

Designed and developed by Absolute Markup.

Status: implementation and automated/read-only verification passed; **real authentication, successful mutations, and failure recovery in a disposable Fedora desktop VM remain unverified**. Milestone 2's Ubuntu/Debian desktop startup also remains open. Approval to begin Milestone 3 did not turn those missing results into passes.

## Changes and reason

The prototype invoked pkexec directly for DNF replay from a user-writable transaction directory, and directly elevated systemctl. Comparing plan bytes in the GUI process did not prevent the directory from being changed before privileged DNF read it. Package/service policy checks also ran before authentication, leaving a window for the observed state to change. Failed actions did not automatically refresh inventory.

The desktop now sends a bounded, versioned JSON request on stdin to `/usr/libexec/linspector-helper` through pkexec. The request contains exact system RPM identities and either reviewed installed/removal identities or one service startup change with its expected state. It contains no transaction path, executable, command string, arbitrary argument list or environment override. Unknown fields, foreign scopes, duplicate identities and unsupported operations are rejected.

The helper is an ordinary root-owned executable, **not setuid**. The supplied policy requires administrator authentication for each active-session invocation (`auth_admin`, without retained authorization); inactive/other sessions are denied. The frontend requires the helper, its ancestor directories and the matching policy to be root-owned and not writable by group/others. If they are absent/untrusted, removal previews remain available but execution and startup changes stay unavailable. There is no fallback to elevating arbitrary DNF/systemctl arguments.

This is a focused implementation review, not an independent security audit or production certification. System binaries, polkit, root-managed native configuration, the RPM database and authorized package scriptlets remain trusted. The UI's distribution detection and root ownership checks do not defend against an already malicious administrator.

## Removal lifecycle

1. An unprivileged DNF5 `remove --no-autoremove --store` produces the displayed preview. Repositories are disabled. Every planned action must remove a known, unprotected installed package, including the selected identity; duplicate removals are rejected.
2. The pending review owns its plan. Discard/selection changes invalidate it; a late async result cannot reopen a cancelled review. Confirmation consumes the matching plan once.
3. After authentication, the helper checks the complete installed identity set against the reviewed snapshot. It resolves another native plan in a private root-owned directory under `/run/linspector`, and compares its installed identities and removal set to the reviewed sets.
4. Only that helper-created directory is replayed. DNF's default mismatch/extras checks remain enabled, without ignore/skip flags.
5. The helper checks the resulting installed identity set, including after an unsuccessful native operation when readable. The UI rescans after success, authentication cancellation/denial, native failure or a disconnected action worker. Selectable Operation details preserve the full captured message when the inline status is truncated. Partial/unknown outcomes are reported as such, never automatically retried or rolled back.

Concurrent native transactions remain DNF/RPM's responsibility. The before/fresh-plan/after checks are conservative observations, not a global cross-process package-manager lock. An unrelated installation makes an old review stale. The helper rechecks rather than silently widening a reviewed removal. Scriptlet failures, signals, power loss and root-managed configuration changes still need VM evidence.

## Service lifecycle

Only ordinary non-template service names are accepted. The selected exact RPM must own the unit under `/usr/lib/systemd/system` or `/usr/lib/systemd/user`, and that file and its directories must be trusted. The manager's loaded fragment must be that same file, without drop-ins. Units with `Also=` or `Alias=` install effects are refused until multiple-unit review is supported. Core/runtime package services are protected.

Only `enabled` and `disabled` states can change, with the expected opposite state checked again immediately before the operation. System changes are authenticated and rechecked by the helper; user changes run as the user. The result is queried afterward. No `--now`, start, stop or mask command is issued. External state changes can still occur concurrently; manual VM checks include the service's MainPID to distinguish observed running state from startup configuration.

## Cancellation, process handling and recovery

- Cancel/close a removal preview: discard its private directory without executing it. An in-flight read-only preview may finish, but its invalidated result is discarded.
- Dismiss the system authentication dialog: pkexec status 126 is reported as cancellation. Status 127 means authorization/agent/invocation failure. Controlled child-process tests verify status handling, **not real authentication**.
- Once a mutation starts, Linspector does not send a timeout/kill signal. Ordinary window closing is blocked while the action is pending. OS termination/power loss cannot be prevented, and no rollback guarantee is made.
- Read-only management probes have deadlines and isolated process groups; query timeout kills/reaps that group. Output capture drains pipes while retaining at most 1 MiB per stream, except the 128 MiB stdout allowance for bulk RPM inventory/ownership reads. Bulk RPM queries have a 30-second deadline and reject invalid UTF-8 rather than silently changing metadata. Authenticated mutations have bounded capture but no forced deadline.
- Ordinary completion/error drops working directories. A killed helper can leave a private directory under `/run/linspector`; it is never reused for replay. Reboot removes `/run` state. Inspect native history and current package/service state before another review.

## Evidence and remaining gate

Automated tests cover malformed/oversized requests, exact source/scope, protected dependencies, stale snapshots, changed native results, postcondition mismatch, native failure/partial outcome, authentication-code classification, absent helper, unit eligibility, cancelled/late reviews, consume-once plans, private-directory cleanup, bounded pipes, query descendants and actual unprivileged helper rejection.

Live Fedora checks exercise inventory, dependencies, system/storage and **prepare/discard only**, without root execution. Native Qt build and removal-preview capture check the unavailable-execution UI. Disposable test RPMs can be built without installing them. These checks do not prove real authentication, root replay, SELinux integration, service mutations or recovery after a killed transaction.

Complete the [Fedora VM procedure](fedora-vm-testing.md), record the results, fix failures and rerun affected checks before calling Milestone 3 complete. Milestone 4 requires separate user approval.

## Native references

- [DNF5 replay](https://dnf5.readthedocs.io/en/latest/commands/replay.8.html): exact replay and mismatch/extra checks; relaxation flags deliberately omitted.
- [pkexec](https://www.freedesktop.org/software/polkit/docs/latest/pkexec.1.html): exit status 126/127, minimal environment, PKEXEC_UID, executable-path policies and argument validation responsibility.
- [systemctl](https://www.freedesktop.org/software/systemd/man/latest/systemctl.html): is-enabled states and enable/disable semantics without `--now`.
