#!/usr/bin/env python3
"""Compare a trusted extracted dpkg root with native query output, read-only."""
from pathlib import Path
import argparse
import hashlib
import json
import os
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('root', type=Path)
parser.add_argument('--exporter', type=Path, default=Path('target/debug/examples/inspect_dpkg_root'))
parser.add_argument('--image-amd64-query', action='store_true',
                    help="Execute the trusted image's amd64 dpkg-query with its own loader/libraries")
args = parser.parse_args()
root = args.root.resolve(strict=True)
exporter = args.exporter.resolve(strict=True)

def digest():
    result = hashlib.sha256()
    for path in sorted((root / 'var/lib/dpkg').rglob('*')):
        if path.is_file() and not path.is_symlink():
            result.update(str(path.relative_to(root)).encode() + b'\0')
            result.update(path.read_bytes())
    return result.hexdigest()

env = {**os.environ, 'LC_ALL': 'C.UTF-8', 'DPKG_COLORS': 'never'}
for key in ('DPKG_ROOT', 'DPKG_ADMINDIR', 'DPKG_PAGER', 'PAGER'):
    env.pop(key, None)

def run(command):
    return subprocess.run(command, capture_output=True, text=True, check=True,
                          env=env, timeout=60).stdout

before = digest()
inventory = json.loads(run([str(exporter), str(root)]))
query = ['/usr/bin/dpkg-query']
if args.image_amd64_query:
    libs = root / 'usr/lib/x86_64-linux-gnu'
    query = [str(libs / 'ld-linux-x86-64.so.2'), '--library-path', str(libs),
             str(root / 'usr/bin/dpkg-query')]
version = run(query + ['--version']).splitlines()[0]
output = run(query + ['--admindir=' + str(root / 'var/lib/dpkg'), '--no-pager',
                     '--show', '--showformat=${Package}\t${Architecture}\t${Version}\t${db:Status-Status}\t${Installed-Size}\n'])
present = {'installed', 'unpacked', 'half-installed', 'half-configured',
           'triggers-awaited', 'triggers-pending'}
expected = {}
for line in output.splitlines():
    name, arch, version_value, state, size = line.split('\t')
    if state in present:
        expected[f'{name}:{arch}={version_value}'] = (version_value, int(size) * 1024 if size else None)
actual = {entry['id']['native']: (entry['version_display'], entry['installed_bytes'])
          for entry in inventory['entries']}
if actual != expected:
    raise SystemExit('Provider identity/version/size map differs from native query')
if inventory['capabilities']['removal_preview'] or inventory['capabilities']['removal_execution']:
    raise SystemExit('Read-only provider unexpectedly exposes removal')
if before != digest():
    raise SystemExit('Database changed during verification (check for concurrent transactions)')
print(json.dumps({'packages': len(actual), 'native_query': version,
                  'identities_versions_and_sizes_match': True, 'database_unchanged': True,
                  'database_sha256': before, 'warnings': inventory['warnings'],
                  'validation_scope': 'Extracted filesystem; not a booted OS/desktop test'}, indent=2))
