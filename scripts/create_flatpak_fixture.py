#!/usr/bin/env python3
"""Create synthetic Flatpak deployments under a NEW temporary root, without launch.

Only explicit temporary installation/config/cache paths are used. No changes are
made to the working Flatpak installations. Named/default system fixtures are copies
of the temporary user deployment, queried by native Flatpak as separate scopes;
this is not a privileged installation test.
"""
from pathlib import Path
import argparse
import json
import os
import shutil
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('root', type=Path, help='New, nonexistent directory, normally under /tmp')
args = parser.parse_args()
root = args.root.absolute()
root.mkdir(mode=0o700)
env = dict(os.environ)
for key, child in {
    'HOME': 'home', 'XDG_DATA_HOME': 'data', 'XDG_CONFIG_HOME': 'user-config',
    'XDG_CACHE_HOME': 'cache', 'XDG_RUNTIME_DIR': 'runtime', 'FLATPAK_USER_DIR': 'user',
    'FLATPAK_SYSTEM_DIR': 'system', 'FLATPAK_CONFIG_DIR': 'config',
    'FLATPAK_SYSTEM_CACHE_DIR': 'system-cache',
}.items():
    env[key] = str(root / child)
(root / 'home').mkdir()
(root / 'runtime').mkdir(mode=0o700)
env['DBUS_SESSION_BUS_ADDRESS'] = 'unix:path=' + str(root / 'runtime/no-session-bus')
(root / 'config/installations.d').mkdir(parents=True)
notes = []

def run(*arguments):
    result = subprocess.run(['/usr/bin/flatpak', *map(str, arguments)], env=env,
                            capture_output=True, text=True, timeout=60)
    if result.returncode:
        raise SystemExit(f'Fixture command failed: {arguments}\n{result.stdout}\n{result.stderr}')
    if result.stderr.strip():
        notes.append(result.stderr.strip())
    return result.stdout

arch = run('--default-arch').strip()
for branch in ['stable', 'beta']:
    runtime = root / f'runtime-{branch}'
    (runtime / 'usr').mkdir(parents=True)
    (runtime / 'files').mkdir()
    (runtime / 'usr/fixture.txt').write_text('Synthetic runtime data.\n')
    (runtime / 'metadata').write_text('[Runtime]\nname=org.example.LinspectorPlatform\n')
    run('build-export', '--runtime', root / 'repo', runtime, branch)
    app = root / f'app-{branch}'
    (app / 'files/bin').mkdir(parents=True)
    command = app / 'files/bin/never-execute-this-fixture'
    command.write_text('#!/bin/sh\nexit 99\n')
    command.chmod(0o755)
    (app / 'metadata').write_text(
        '[Application]\nname=org.example.LinspectorPaint\n'
        f'runtime=org.example.LinspectorPlatform/{arch}/{branch}\n'
        'command=never-execute-this-fixture\n')
    run('build-finish', '--command=never-execute-this-fixture', app)
    run('build-export', root / 'repo', app, branch)
run('remote-add', '--user', '--no-gpg-verify', 'fixture-local', root / 'repo')
refs = [f'{kind}/org.example.{name}/{arch}/{branch}'
        for branch in ['stable', 'beta']
        for kind, name in [('runtime', 'LinspectorPlatform'), ('app', 'LinspectorPaint')]]
run('install', '--user', '--noninteractive', '--no-related', '--no-deps', 'fixture-local', *refs)
# Clone synthetic deployments, not host data, to exercise native scope routing.
shutil.copytree(root / 'user', root / 'system', symlinks=True)
shutil.copytree(root / 'user', root / 'extra', symlinks=True)
(root / 'config/installations.d/fixture-extra.conf').write_text(
    '[Installation "fixture-extra"]\n' + f'Path={root / "extra"}\n'
    'DisplayName=Linspector synthetic named scope\nStorageType=harddisk\n')
listed = json.loads(run('list', '--all', '--json', '--columns=ref,installation'))
assert len(listed) == 12, listed
assert {row['installation'] for row in listed} == {'user', 'system', 'system (fixture-extra)'}
(root / 'fixture-report.json').write_text(json.dumps({
    'synthetic': True, 'native_refs': len(listed), 'scopes': ['user', 'system', 'fixture-extra'],
    'working_installation_changed': False, 'apps_launched': False,
    'system_scopes': 'Copied temporary deployments; no privileged install',
    'native_setup_warnings': notes,
}, indent=2) + '\n')
print(f'Created {len(listed)} native synthetic refs under {root}')
if notes:
    print('Native setup warnings were recorded in fixture-report.json; no sandbox checks were disabled.')
