#!/usr/bin/env python3
"""Create a source-only archive and verify every byte after extracting it."""
from pathlib import Path
import hashlib, tempfile, zipfile
root = Path(__file__).resolve().parents[1]
out = root.parent / 'linspector-native-starter.zip'
files = sorted(p for p in root.rglob('*') if p.is_file() and not any(x in {'.git', 'target', 'build'} for x in p.relative_to(root).parts) and p.name != '.qmlls.ini')
with zipfile.ZipFile(out, 'w', zipfile.ZIP_DEFLATED) as z:
    for p in files:
        z.write(p, Path(root.name) / p.relative_to(root))
with zipfile.ZipFile(out) as z:
    assert z.testzip() is None
    with tempfile.TemporaryDirectory(prefix='linspector-archive-') as temp:
        z.extractall(temp)
        for p in files:
            restored = Path(temp) / root.name / p.relative_to(root)
            assert restored.read_bytes() == p.read_bytes(), p
checksum = hashlib.sha256(out.read_bytes()).hexdigest()
out.with_suffix('.zip.sha256').write_text(f'{checksum}  {out.name}\n')
print(f'Verified: {len(files)} files, {out.stat().st_size} bytes\nSHA-256: {checksum}\n{out}')
