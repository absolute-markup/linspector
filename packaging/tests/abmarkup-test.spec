# Disposable VM fixtures only. Designed and developed by Absolute Markup.
Name: abmarkup-test-leaf
Version: 1.0
Release: 1
Summary: Disposable Linspector removal and startup test fixture
License: CC0-1.0
BuildArch: noarch

%description
Harmless fixture data and a sleep-only service for a disposable Fedora VM.
Never install these test fixtures on a working system.

%package -n abmarkup-test-consumer
Summary: Disposable dependency consumer for Linspector
Requires: abmarkup-test-leaf = %{version}-%{release}
%description -n abmarkup-test-consumer
A fixture that depends on the leaf, to exercise a multi-package removal review.

%package -n abmarkup-test-unrelated
Summary: Disposable unrelated package for stale-review testing
%description -n abmarkup-test-unrelated
Install after a removal preview to make the reviewed installed state stale.

%package -n abmarkup-test-failure
Summary: Disposable intentional uninstall failure for Linspector
%description -n abmarkup-test-failure
A pre-uninstall script deliberately exits 42. Restore the VM snapshot afterward.

%install
mkdir -p %{buildroot}/usr/share/abmarkup-test %{buildroot}/usr/lib/systemd/system
printf 'Disposable leaf fixture\n' > %{buildroot}/usr/share/abmarkup-test/leaf.txt
printf 'Disposable consumer fixture\n' > %{buildroot}/usr/share/abmarkup-test/consumer.txt
printf 'Disposable unrelated fixture\n' > %{buildroot}/usr/share/abmarkup-test/unrelated.txt
printf 'Intentional uninstall failure fixture\n' > %{buildroot}/usr/share/abmarkup-test/failure.txt
cat > %{buildroot}/usr/lib/systemd/system/abmarkup-test.service <<'UNIT'
[Unit]
Description=Absolute Markup disposable startup test
[Service]
Type=simple
ExecStart=/usr/bin/sleep infinity
[Install]
WantedBy=multi-user.target
UNIT

%preun -n abmarkup-test-failure
# Deliberately exercise an unsuccessful native uninstall in a disposable VM.
exit 42

%files
/usr/share/abmarkup-test/leaf.txt
/usr/lib/systemd/system/abmarkup-test.service
%files -n abmarkup-test-consumer
/usr/share/abmarkup-test/consumer.txt
%files -n abmarkup-test-unrelated
/usr/share/abmarkup-test/unrelated.txt
%files -n abmarkup-test-failure
/usr/share/abmarkup-test/failure.txt

%changelog
* Mon Oct 26 2026 Absolute Markup - 1.0-1
- Add disposable removal, dependency, stale-state, and failure fixtures.
