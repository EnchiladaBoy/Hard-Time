#!/usr/bin/env python3
"""Create a separate portable Windows install; preserve all existing save data."""
from __future__ import annotations

import argparse
from contextlib import contextmanager
import hashlib
import json
import os
from pathlib import Path
import shutil
import struct
import sys

import windows

ROOT = Path(__file__).resolve().parents[1]
ASSETS = ('Characters', 'Graphics', 'Sound', 'Weapons', 'World')
DEPENDENCY_NOTICES = {
    'SDL.txt': 'sdl/tree/LICENSE.txt',
    'FreeType.txt': 'freetype2/tree/docs/FTL.TXT',
    'FreeImage.txt': 'freeimage/src/license-fi.txt',
    'JPEG.txt': 'freeimage/src/Source/LibJPEG/README',
    'libpng.txt': 'libpng/tree/LICENSE',
    'zlib.txt': 'zlib/tree/README',
    'Ogg.txt': 'ogg/tree/COPYING',
    'Vorbis.txt': 'vorbis/tree/COPYING',
    'minimp3.txt': 'minimp3/tree/LICENSE',
    'ENet.txt': 'enet/tree/LICENSE',
    'CrossGuid.txt': 'crossguid/src/LICENSE',
    'utf8.txt': 'utf8.h/tree/LICENSE',
    'StackWalker.txt': 'stackwalker/tree/LICENSE',
}


def private_directory(path):
    if path.is_symlink() or (hasattr(path, 'is_junction') and path.is_junction()):
        raise ValueError(f'Private writable directory required: {path}')
    if path.exists() and not path.is_dir():
        raise ValueError(f'A directory is required: {path}')


def verify_pe64(binary):
    data = binary.read_bytes()
    if len(data) < 64 or data[:2] != b'MZ':
        raise ValueError(f'Not a Windows executable: {binary}')
    offset = struct.unpack_from('<I', data, 0x3c)[0]
    if (offset + 26 > len(data) or data[offset:offset+4] != b'PE\0\0' or
        struct.unpack_from('<H', data, offset+4)[0] != 0x8664 or
        struct.unpack_from('<H', data, offset+24)[0] != 0x20b):
        raise ValueError(f'Expected a native x64 executable: {binary}')


@contextmanager
def install_lock(destination):
    import msvcrt
    with (destination / '.launch.lock').open('a+b') as lock:
        if lock.tell() == 0:
            lock.write(b'\0')
            lock.flush()
        lock.seek(0)
        try:
            msvcrt.locking(lock.fileno(), msvcrt.LK_NBLCK, 1)
        except OSError as error:
            raise ValueError('Close this Hard Time installation before updating it.') from error
        try:
            yield
        finally:
            lock.seek(0)
            msvcrt.locking(lock.fileno(), msvcrt.LK_UNLCK, 1)


def install(binary, assets, destination, runtime_files, software=False):
    binary, assets, destination = (Path(p).resolve() for p in (binary, assets, destination))
    verify_pe64(binary)
    if assets == destination or assets in destination.parents or destination in assets.parents:
        raise ValueError('The destination must be separate from the original game.')
    for name in (*ASSETS, 'Data'):
        if not (assets / name).is_dir():
            raise ValueError(f'Missing original game directory: {assets / name}')
    marker = destination / '.hardtime-native-install.json'
    if destination.exists() and any(destination.iterdir()) and not marker.is_file():
        raise ValueError(f'Refusing to replace an unrelated directory: {destination}')
    if marker.is_file():
        previous = json.loads(marker.read_text(encoding='utf-8'))
        if previous.get('software_renderer') != software:
            raise ValueError('Choose a new destination when changing renderer mode; existing saves are preserved.')
    for name in (*ASSETS, 'Data', 'Photo Album'):
        private_directory(destination / name)
    destination.mkdir(parents=True, exist_ok=True)
    with install_lock(destination):
        for name in (*ASSETS, 'Data'):
            if not (destination / name).exists():
                shutil.copytree(assets / name, destination / name)
        (destination / 'Photo Album').mkdir(exist_ok=True)
        temporary = destination / '.Hard Time.new.exe'
        shutil.copy2(binary, temporary)
        temporary.replace(destination / 'Hard Time.exe')
        for dll, filename in runtime_files:
            shutil.copy2(dll, destination / filename)
        shutil.copy2(ROOT / 'scripts/play-windows.ps1', destination / 'play.ps1')
        (destination / 'Play Hard Time.cmd').write_text(
            '@echo off\npowershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0play.ps1"\n'
            'if errorlevel 1 pause\n', encoding='ascii')
        shutil.copy2(ROOT / 'NOTICE.txt', destination / 'NOTICE.txt')
        shutil.copy2(ROOT / 'docs/PLAY-WINDOWS.txt', destination / 'Read Me.txt')
        for name in ('Instruction Manual.txt', 'Survival Guide.txt'):
            if (assets / name).is_file() and not (destination / name).exists():
                shutil.copy2(assets / name, destination / name)
        notices = destination / 'licenses'
        notices.mkdir(exist_ok=True)
        shutil.copy2(ROOT / 'docs/BLITZ3D-LICENSE.txt', notices / 'BLITZ3D.txt')
        for filename, relative in DEPENDENCY_NOTICES.items():
            source = windows.WORK / 'engine/deps' / relative
            if source.is_file():
                shutil.copy2(source, notices / filename)
        openal_notice = windows.CACHE / 'toolchain/openal/openal-soft-1.25.2-bin/COPYING'
        if openal_notice.is_file():
            shutil.copy2(openal_notice, notices / 'OpenAL-Soft.txt')
        if software:
            shutil.copy2(ROOT / 'docs/MESA-NOTICE.txt', notices / 'Mesa.txt')
            (destination / 'software-renderer.txt').write_text(
                'Mesa llvmpipe is enabled for this installation.\n'
                'This uses the CPU and does not change the system graphics driver.\n')
        marker.write_text(json.dumps({'platform': 'windows-x64',
                                     'executable_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(),
                                     'software_renderer': software}, indent=2) + '\n')
    return destination


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    source = parser.add_mutually_exclusive_group(required=True)
    source.add_argument('--assets', type=Path, help='Directory containing the extracted original game')
    source.add_argument('--installer', type=Path, help='Original hardtime_setup.exe; extract without executing')
    parser.add_argument('--binary', type=Path, default=windows.WORK / 'game/Hard Time.exe')
    parser.add_argument('--destination', type=Path, default=ROOT / 'dist/Hard-Time-Windows-x64')
    parser.add_argument('--software-renderer', action='store_true')
    args = parser.parse_args()
    assets = args.assets
    if args.installer:
        with args.installer.open('rb') as source_file:
            digest = hashlib.file_digest(source_file, 'sha256').hexdigest()
        assets = windows.CACHE / ('original-' + digest[:16])
        if not assets.exists():
            sevenzip = windows.find_tool('7z', Path(os.environ['ProgramFiles']) / '7-Zip/7z.exe')
            windows.run(sevenzip, 'x', args.installer.resolve(), '-o' + str(assets), '-y', '-bso0', '-bsp0')
    destination = install(args.binary, assets, args.destination,
                          windows.runtime_files(args.software_renderer), args.software_renderer)
    print('Launch:', destination / 'Play Hard Time.cmd')
    print('Existing saves and photos are preserved. The launcher backs up Data before each run.')


if __name__ == '__main__':
    try:
        main()
    except (OSError, ValueError, RuntimeError) as error:
        print(f'Installation failed: {error}', file=sys.stderr)
        sys.exit(1)
