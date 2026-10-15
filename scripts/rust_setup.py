"""Maintainer-only Rust setup build and dependency notice helpers."""
from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import shutil
import struct
import subprocess

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / 'setup/rust/Cargo.toml'
NOTICE_PREFIXES = ('license', 'licence', 'copying', 'copyright', 'notice')
MAGIC = b'HARDTIME-RUST-01'


def tool(name):
    found = shutil.which(name)
    if found:
        return found
    candidate = Path(os.environ.get('CARGO_HOME', Path.home() / '.cargo')) / 'bin' / (name + '.exe')
    if not candidate.is_file():
        raise RuntimeError(f'{name} was not found. Maintainers need the pinned Rust toolchain.')
    return str(candidate)


def cargo(*args):
    return [tool('cargo'), *args, '--manifest-path', str(MANIFEST), '--locked']


def metadata():
    return json.loads(subprocess.check_output(cargo('metadata', '--offline', '--format-version', '1',
                                                   '--filter-platform', 'x86_64-pc-windows-msvc'),
                                              cwd=ROOT, text=True, encoding='utf-8'))


def build():
    subprocess.run(cargo('build', '--release'), cwd=ROOT, check=True)
    target = Path(metadata()['target_directory']) / 'release'
    return target / 'hardtime-setup.exe', target / 'hardtime-launcher.exe'


def collect_notices(destination):
    """Flat notice paths remain compatible with the original setup journal."""
    destination.mkdir(parents=True, exist_ok=True)
    index = []
    data = metadata()
    resolved = {node['id'] for node in data['resolve']['nodes']}
    for package in data['packages']:
        if package['id'] not in resolved or not package['source']:
            continue
        source = Path(package['manifest_path']).parent
        files = sorted(p for p in source.rglob('*') if p.is_file()
                       and p.name.lower().startswith(NOTICE_PREFIXES))
        if package['license_file']:
            explicit = (source / package['license_file']).resolve(strict=True)
            if not explicit.is_relative_to(source.resolve()):
                raise RuntimeError(f'License escapes crate: {package["name"]}')
            if explicit not in files:
                files.append(explicit)
        if not files:
            raise RuntimeError(f'Missing license text: {package["name"]}; review before packaging.')
        filename = f'Rust-{package["name"]}-{package["version"]}.txt'
        sections = [f'{package["name"]} {package["version"]}\n'
                    f'Declared license: {package["license"]}\nRepository: {package["repository"]}\n']
        for path in files:
            sections.append(f'\n--- {path.relative_to(source).as_posix()} ---\n\n'
                            + path.read_text(encoding='utf-8'))
        # Include manifest attribution when crates put author/copyright metadata
        # there instead of in their shared upstream license text.
        sections.append('\n--- Cargo.toml ---\n\n' + (source / 'Cargo.toml').read_text(encoding='utf-8'))
        (destination / filename).write_text('\n'.join(sections), encoding='utf-8')
        index.append({'name': package['name'], 'version': package['version'],
                      'license': package['license'], 'repository': package['repository'], 'notice': filename})
    rustc = Path(subprocess.check_output([tool('rustup'), 'which', 'rustc'], cwd=ROOT, text=True).strip())
    docs = rustc.parent.parent / 'share/doc/rust'
    shutil.copy2(docs / 'COPYRIGHT-library.html', destination / 'Rust-standard-library.html')
    for path in sorted((docs / 'licenses').glob('*')):
        if path.is_file():
            shutil.copy2(path, destination / ('Rust-library-' + path.name))
    (destination / 'Rust-dependencies.json').write_text(json.dumps(index, indent=2) + '\n', encoding='utf-8')
    return len(index)


def append_payload(program, archive, output):
    """PE overlay: raw ZIP followed by its offset, size and SHA256 checksum."""
    data = archive.read_bytes()
    with output.open('xb') as destination, program.open('rb') as source:
        shutil.copyfileobj(source, destination)
        offset = destination.tell()
        destination.write(data)
        destination.write(MAGIC + struct.pack('<QQ', offset, len(data)) + hashlib.sha256(data).digest())
        destination.flush()
        os.fsync(destination.fileno())
