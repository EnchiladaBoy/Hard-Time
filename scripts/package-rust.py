#!/usr/bin/env python3
"""Package the Windows Rust preview and dependency notices; no game assets."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import struct
import subprocess
import tempfile
import zipfile

ROOT = Path(__file__).resolve().parents[1]
TARGET = 'x86_64-pc-windows-msvc'
NOTICE_PREFIXES = ('license', 'licence', 'copying', 'copyright', 'notice', 'ofl')


def rust_tool(name):
    found = shutil.which(name)
    if found:
        return found
    candidate = Path(os.environ.get('CARGO_HOME', Path.home() / '.cargo')) / 'bin' / (name + '.exe')
    if not candidate.is_file():
        raise RuntimeError(f'{name} was not found. Install the pinned Rust toolchain first.')
    return str(candidate)


def run(command):
    return subprocess.check_output(command, cwd=ROOT, text=True, encoding='utf-8')


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def copy(source, destination):
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source, destination)


def collect_notices(destination):
    metadata = json.loads(run([rust_tool('cargo'), 'metadata', '--locked', '--offline',
                               '--format-version', '1', '--filter-platform', TARGET]))
    resolved = {node['id'] for node in metadata['resolve']['nodes']}
    supplements = json.loads((ROOT / 'rust/licenses/upstream.json').read_text(encoding='utf-8'))
    supplemental = {name: entry for entry in supplements for name in entry['packages']}
    index = []
    for package in metadata['packages']:
        if package['id'] not in resolved or not package['source']:
            continue
        source = Path(package['manifest_path']).parent
        folder = destination / f"{package['name']}-{package['version']}"
        files = sorted(p for p in source.rglob('*') if p.is_file()
                       and p.name.lower().startswith(NOTICE_PREFIXES))
        if package['license_file']:
            explicit = (source / package['license_file']).resolve(strict=True)
            if not explicit.is_relative_to(source.resolve()):
                raise RuntimeError(f'License path escapes crate: {package["name"]}')
            if explicit not in files:
                files.append(explicit)
        for path in files:
            copy(path, folder / path.relative_to(source))
        if not files:
            entry = supplemental.get(package['name'])
            if not entry:
                raise RuntimeError(f'Missing license text for {package["name"]}; review before packaging.')
            vcs = json.loads((source / '.cargo_vcs_info.json').read_text())
            if entry['revision'] != vcs['git']['sha1']:
                raise RuntimeError(f'Stale supplemental notice for {package["name"]}')
            for file in entry['files']:
                path = (ROOT / file['path']).resolve(strict=True)
                if not path.is_relative_to((ROOT / 'rust/licenses').resolve()) or digest(path) != file['sha256']:
                    raise RuntimeError(f'Invalid supplemental notice: {file["path"]}')
                copy(path, folder / path.name)
        # Preserve declared licenses and author/source metadata, including crates
        # whose notices only exist at the root of their upstream monorepo.
        for name in ('Cargo.toml', 'Cargo.toml.orig', 'README.md', '.cargo_vcs_info.json'):
            if (source / name).is_file():
                copy(source / name, folder / name)
        index.append({'name': package['name'], 'version': package['version'],
                      'license': package['license'], 'repository': package['repository'],
                      'files': [p.relative_to(destination).as_posix()
                                for p in sorted(folder.rglob('*')) if p.is_file()]})
    rustc = Path(run([rust_tool('rustup'), 'which', 'rustc']).strip())
    rust_docs = rustc.parent.parent / 'share/doc/rust'
    copy(rust_docs / 'COPYRIGHT-library.html', destination / 'rust/COPYRIGHT-library.html')
    for path in (rust_docs / 'licenses').glob('*'):
        if path.is_file():
            copy(path, destination / 'rust/licenses' / path.name)
    copy(ROOT / 'rust/licenses/README.txt', destination / 'README.txt')
    copy(ROOT / 'rust/licenses/upstream.json', destination / 'upstream.json')
    (destination / 'index.json').write_text(json.dumps(index, indent=2) + '\n', encoding='utf-8')
    return len(index)


def package(binary, output):
    data = binary.read_bytes()
    if len(data) < 64 or data[:2] != b'MZ':
        raise RuntimeError('Expected a Windows executable; run cargo build --release first.')
    offset = struct.unpack_from('<I', data, 0x3c)[0]
    if data[offset:offset + 4] != b'PE\0\0' or data[offset + 4:offset + 6] != b'\x64\x86':
        raise RuntimeError('Expected a Windows x64 executable.')
    output.parent.mkdir(parents=True, exist_ok=True)
    # Cleanup is restricted to this newly created temporary directory, inside
    # the explicit package output directory. No source-game files are copied.
    with tempfile.TemporaryDirectory(prefix='rust-package-', dir=output.parent) as temporary:
        stage = Path(temporary).resolve()
        if not stage.is_relative_to(output.parent.resolve()):
            raise RuntimeError('Package staging directory escaped output directory')
        copy(binary, stage / 'Hard-Time-Rust.exe')
        copy(ROOT / 'rust/README.txt', stage / 'Read Me.txt')
        copy(ROOT / 'NOTICE.txt', stage / 'NOTICE.txt')
        count = collect_notices(stage / 'licenses')
        manifest = {
            'schema': 1,
            'description': 'Rust environment preview; not the complete game',
            'target': TARGET,
            'rustc': run([rust_tool('rustc'), '--version']).strip(),
            'cargo_lock_sha256': digest(ROOT / 'Cargo.lock'),
            'dependency_notice_count': count,
            'files': {p.relative_to(stage).as_posix(): digest(p)
                      for p in sorted(stage.rglob('*')) if p.is_file()},
        }
        (stage / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n', encoding='utf-8')
        payload = [p for p in sorted(stage.rglob('*')) if p.is_file()]
        built = stage / 'package.zip'
        with zipfile.ZipFile(built, 'w', zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
            for path in payload:
                # Cargo archives often use a 1970 timestamp, outside ZIP's range.
                # Stable timestamps also make repeated packaging deterministic.
                entry = zipfile.ZipInfo('Hard-Time-Rust-preview/' + path.relative_to(stage).as_posix())
                entry.compress_type = zipfile.ZIP_DEFLATED
                entry.external_attr = 0o644 << 16
                archive.writestr(entry, path.read_bytes(), compresslevel=9)
        os.replace(built, output)
    output.with_suffix('.sha256').write_text(f'{digest(output)}  {output.name}\n', encoding='ascii')
    print(f'Packaged {output.name}: {output.stat().st_size / 1024**2:.1f} MiB; {count} dependency notices')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, default=ROOT / 'target/release/hardtime-rust.exe')
    parser.add_argument('--output', type=Path, default=ROOT / 'dist/Hard-Time-Rust-preview.zip')
    args = parser.parse_args()
    package(args.binary.resolve(strict=True), args.output.resolve())


if __name__ == '__main__':
    main()
