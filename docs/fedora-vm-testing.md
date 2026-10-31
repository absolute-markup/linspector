# Fedora 44 disposable VM tests — Milestone 3

Designed and developed by Absolute Markup.

Use a **disposable mutable Fedora 44 desktop VM**, with a normal graphical login, a polkit authentication agent and a restorable snapshot. Your Fedora laptop can host the VM. On the laptop itself, use ordinary startup, scanning and prepare/discard previews; this procedure's helper installation, fixture installation, removals and service changes belong inside the disposable guest.

These commands are a prepared test procedure, **not completed validation**. This session cannot access the libvirt daemon, has no `/dev/kvm`, and has not run these authenticated/mutating cases. Native host checks do not substitute for the guest results. Do not use a detected container as a replacement for full desktop/polkit/SELinux testing.

## Build and install inside the guest

Copy/extract the current source ZIP into the guest. Follow the Fedora build prerequisites in [development.md](development.md), with Rust/Cargo 1.88 or newer. The fixture build also requires `rpm-build`. Build as the ordinary guest user:

```sh
export QMAKE=/usr/bin/qmake6
cargo test --locked -p linspector-inventory
cargo build --locked -p linspector
cargo build --locked -p linspector-inventory --bin linspector-helper
```

The following are **guest-only** installation commands:

```sh
sudo install -D -o root -g root -m 0755 target/debug/linspector-helper /usr/libexec/linspector-helper
sudo install -D -o root -g root -m 0644 packaging/polkit/com.absolutemarkup.linspector.policy /usr/share/polkit-1/actions/com.absolutemarkup.linspector.policy
sudo restorecon /usr/libexec/linspector-helper /usr/share/polkit-1/actions/com.absolutemarkup.linspector.policy
pkaction --action-id com.absolutemarkup.linspector.manage --verbose
```

No setuid bit or permissive polkit rule is needed. Keep SELinux enforcing; record AVC/permission failures instead of disabling protection. Start the app as the normal user, without sudo. Check that removal execution and startup controls become available after scanning. If not, record the reason and file/policy ownership; do not bypass checks.

## Build disposable fixtures

```sh
rpmbuild -bb --define "_topdir /tmp/linspector-vm-fixtures" --define "_tmppath /tmp" packaging/tests/abmarkup-test.spec
```

The spec creates four packages: `abmarkup-test-leaf`, a dependent `abmarkup-test-consumer`, an unrelated package, and `abmarkup-test-failure`. The leaf owns a sleep-only service. The failure fixture deliberately returns 42 from its pre-uninstall script. Restore a VM snapshot after that test instead of forcing package removal.

Guest-only initial installation:

```sh
sudo dnf5 --disable-repo='*' install /tmp/linspector-vm-fixtures/RPMS/noarch/abmarkup-test-leaf-1.0-1.noarch.rpm /tmp/linspector-vm-fixtures/RPMS/noarch/abmarkup-test-consumer-1.0-1.noarch.rpm
```

Take a snapshot here. Record the installed identity set before each case:

```sh
rpm -qa --qf '%{NEVRA}\n' | sort > /tmp/linspector-before.txt
```

## Cases to perform and record

Restore the baseline snapshot between independent mutation cases. Record expected/observed behavior, package identities, startup state, authentication outcome, app message and relevant DNF history/errors.

| Case | Procedure | Required result |
|---|---|---|
| Cancel preview | Review leaf removal, then Cancel | Both leaf and consumer are listed; cancelling leaves the RPM set unchanged |
| Cancel authentication | Confirm that same review, then dismiss polkit | Cancellation reported; both packages remain; UI rescans and requires a new review |
| Denied authorization | Deny authentication or use a guest account without admin authorization | Denial/agent failure reported; no replay; UI refreshes |
| Successful removal | Confirm and authenticate the leaf review | Exactly leaf + consumer removed, unrelated packages preserved, successful post-check and refreshed inventory |
| Stale review | Open leaf review, install the unrelated fixture in another guest terminal, then confirm/authenticate | Helper refuses the old installed snapshot; leaf + consumer remain; no automatic retry |
| State changes during auth | Start authentication, install unrelated fixture before completing it, then authenticate | Same stale-state rejection after authentication |
| Failure/partial outcome | Install failure fixture in guest, review its removal and authenticate | Native failure/post-check mismatch is reported, actual current inventory is refreshed, no automatic retry/rollback claim |
| Native lock/error | While another guest DNF transaction holds its native lock, attempt a review/change | Native behavior is reported without relaxing locks/mismatch checks; app remains able to rescan afterward |
| Window close | Try normal window closing while waiting for authentication/operation | Closing is refused; dismiss authentication or let the operation complete |
| Interrupted guest | After taking a snapshot, power off the disposable guest during a fixture transaction; reboot | Observe RPM/DNF state/history, record recovery needed; app rescans and uses a fresh review; no success/rollback claim without evidence |

Install the unrelated/failure fixture in the guest only when needed:

```sh
sudo dnf5 --disable-repo='*' install /tmp/linspector-vm-fixtures/RPMS/noarch/abmarkup-test-unrelated-1.0-1.noarch.rpm
sudo dnf5 --disable-repo='*' install /tmp/linspector-vm-fixtures/RPMS/noarch/abmarkup-test-failure-1.0-1.noarch.rpm
```

Compare state afterward, including cancelled/failed cases:

```sh
rpm -qa --qf '%{NEVRA}\n' | sort > /tmp/linspector-after.txt
diff -u /tmp/linspector-before.txt /tmp/linspector-after.txt
```

## Service startup cases

Restore the baseline with the leaf installed. In the guest:

```sh
sudo systemctl daemon-reload
sudo systemctl start abmarkup-test.service
systemctl show abmarkup-test.service --property=MainPID --property=ActiveState
systemctl is-enabled abmarkup-test.service
```

In Linspector, select the leaf, manage startup and enable it. Authenticate, verify `enabled`, and compare MainPID/ActiveState before/after. Disable startup through Linspector, verify `disabled`, and check the same running process remains. Test cancelling authentication as well.

Also test a guest-local override/drop-in, a static/masked unit, a template and a unit with `Also=`/`Alias=` install effects. The app/helper must refuse unsupported startup changes. Use only disposable test units; do not choose production/core services. Record a failure if ownership, expected state or loaded fragment checks can be bypassed.

## Report

Record Fedora release/architecture, DNF5/Qt/Rust versions, GNOME/KDE and Wayland/X11, polkit agent availability, SELinux mode, case results and logs. Leave unperformed cases marked **not run**. Full desktop tests on Ubuntu/Debian remain a separate Milestone 2 gap. We will review results and fixes before declaring Milestone 3 complete; the next milestone still needs explicit approval.
