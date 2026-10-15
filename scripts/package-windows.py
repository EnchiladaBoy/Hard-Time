#!/usr/bin/env python3
"""Build the single-file Windows setup app. Build tools are for maintainers only."""
from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import struct
import subprocess
import tempfile
import zipfile

import windows

ROOT = Path(__file__).resolve().parents[1]
CSC = Path(os.environ.get('WINDIR', r'C:\Windows')) / 'Microsoft.NET/Framework64/v4.0.30319/csc.exe'
REFERENCES = ('System.dll', 'System.Core.dll', 'System.Drawing.dll', 'System.Windows.Forms.dll',
              'System.IO.Compression.dll', 'System.Web.Extensions.dll')


def compile_cs(output, sources, *, payload=None, console=False, icon=None):
    command = [str(CSC), '/nologo', '/optimize+', '/platform:x64',
               '/target:' + ('exe' if console else 'winexe'), '/out:' + str(output),
               '/win32manifest:' + str(ROOT / 'setup/app.manifest')]
    command += ['/reference:' + name for name in REFERENCES]
    if payload:
        command.append('/resource:' + str(payload) + ',payload.zip')
    if icon:
        command.append('/win32icon:' + str(icon))
    subprocess.run(command + [str(p) for p in sources], check=True)


def make_icon(path):
    """A small code-drawn prison-bar monogram; no original game art is bundled."""
    size = 48
    pixels = bytearray()
    for y in reversed(range(size)):
        for x in range(size):
            orange = (10 <= x < 16 or 32 <= x < 38 or 16 <= x < 32 and 21 <= y < 27) and 9 <= y < 39
            r, g, b = (237, 148, 85) if orange else (20, 27, 35)
            pixels.extend((b, g, r, 255))
    mask = bytes(((size + 31) // 32) * 4 * size)
    bitmap = struct.pack('<IiiHHIIiiII', 40, size, size * 2, 1, 32, 0, len(pixels), 0, 0, 0, 0) + pixels + mask
    path.write_bytes(struct.pack('<HHH', 0, 1, 1) + struct.pack('<BBBBHHII', size, size, 0, 0, 1, 32, len(bitmap), 22) + bitmap)


def package(binary, output):
    if not CSC.is_file():
        raise RuntimeError('The Windows .NET Framework C# compiler was not found.')
    spec = importlib.util.spec_from_file_location('legacy_install', ROOT / 'scripts/install-windows.py')
    legacy = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(legacy)
    legacy.verify_pe64(binary)
    if b'UTF-8</activeCodePage>' not in binary.read_bytes():
        raise RuntimeError('Rebuild with scripts/windows.py so the game supports non-ASCII installation paths.')
    runtime = windows.runtime_files(True)
    extractor = windows.dependency('sevenzip')
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='hardtime-package-', dir=windows.CACHE) as temporary:
        stage = Path(temporary)
        payload = stage / 'payload'
        for folder in ('runtime/licenses', 'software', 'tools'):
            (payload / folder).mkdir(parents=True)
        icon = stage / 'hardtime.ico'
        make_icon(icon)
        compile_cs(payload / 'runtime/Play Hard Time.exe',
                   [ROOT / 'setup/Common.cs', ROOT / 'setup/Launcher.cs'], icon=icon)
        shutil.copy2(binary, payload / 'runtime/Hard Time.exe')
        for source, name in runtime:
            shutil.copy2(source, payload / ('runtime' if name == 'OpenAL32.dll' else 'software') / name)
        (payload / 'software/software-renderer.txt').write_text(
            'Mesa llvmpipe is enabled for this installation.\n'
            'This uses the CPU and does not change the system graphics driver.\n', encoding='ascii')
        for source, target in ((ROOT / 'docs/PLAY-WINDOWS.txt', 'Read Me.txt'),
                               (ROOT / 'NOTICE.txt', 'NOTICE.txt'),
                               (ROOT / 'scripts/play-windows.ps1', 'play.ps1')):
            shutil.copy2(source, payload / 'runtime' / target)
        (payload / 'runtime/Play Hard Time.cmd').write_text(
            '@echo off\nstart "" "%~dp0Play Hard Time.exe"\n', encoding='ascii')
        notices = payload / 'runtime/licenses'
        shutil.copy2(ROOT / 'docs/BLITZ3D-LICENSE.txt', notices / 'BLITZ3D.txt')
        shutil.copy2(ROOT / 'docs/MESA-NOTICE.txt', notices / 'Mesa.txt')
        for name, relative in legacy.DEPENDENCY_NOTICES.items():
            shutil.copy2(windows.WORK / 'engine/deps' / relative, notices / name)
        # FreeImage includes additional image codecs with their own notices.
        freeimage = windows.WORK / 'engine/deps/freeimage/src/Source'
        for name, relative in {
            'FreeImage-WebP.txt': 'LibWebP/COPYING',
            'FreeImage-OpenJPEG.txt': 'LibOpenJPEG/LICENSE',
            'FreeImage-PNG.txt': 'LibPNG/LICENSE',
            'FreeImage-Raw-Copyright.txt': 'LibRawLite/COPYRIGHT',
            'FreeImage-Raw-LGPL.txt': 'LibRawLite/LICENSE.LGPL',
            'FreeImage-Raw-CDDL.txt': 'LibRawLite/LICENSE.CDDL',
            'FreeImage-OpenEXR.txt': 'OpenEXR/Copyrights/openexr/LICENSE',
            'FreeImage-IlmBase.txt': 'OpenEXR/Copyrights/ilmbase/LICENSE',
            'FreeImage-JXR.txt': 'LibJXR/LICENCE',
        }.items():
            shutil.copy2(freeimage / relative, notices / name)
        tiff = (freeimage / 'LibTIFF4/tiff.h').read_text(encoding='utf-8')
        (notices / 'FreeImage-TIFF.txt').write_text(tiff.split('*/', 1)[0] + '*/\n', encoding='utf-8')
        shutil.copy2(windows.CACHE / 'toolchain/openal/openal-soft-1.25.2-bin/COPYING', notices / 'OpenAL-Soft.txt')
        for name in ('7z.exe', '7z.dll'):
            shutil.copy2(extractor / name, payload / 'tools' / name)
        shutil.copy2(extractor / 'License.txt', notices / '7-Zip.txt')
        for file in (ROOT / 'setup/licenses').glob('*'):
            shutil.copy2(file, notices / file.name)
        (notices / '7-Zip-source.txt').write_text(
            'Setup uses unmodified 7-Zip 26.04 (7z.exe and 7z.dll), Copyright (c) Igor Pavlov.\n'
            'Licensed under GNU LGPL 2.1 or later, with additional notices in 7-Zip.txt.\n'
            'Source and downloads: https://www.7-zip.org/\n'
            'Version source: https://github.com/ip7z/7zip/tree/26.04\n', encoding='ascii')
        files = {file.relative_to(payload).as_posix(): hashlib.sha256(file.read_bytes()).hexdigest()
                 for file in sorted(payload.rglob('*')) if file.is_file()}
        manifest = {'schema': 1, 'game_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(), 'files': files}
        (payload / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n', encoding='utf-8')
        archive = stage / 'payload.zip'
        with zipfile.ZipFile(archive, 'w', zipfile.ZIP_DEFLATED, compresslevel=9) as zip_file:
            for file in sorted(payload.rglob('*')):
                if file.is_file():
                    zip_file.write(file, file.relative_to(payload).as_posix())
        built = stage / 'Hard-Time-Setup.exe'
        compile_cs(built, [ROOT / 'setup' / name for name in ('Common.cs', 'SetupCore.cs', 'GraphicsProbe.cs', 'Setup.cs')],
                   payload=archive, icon=icon)
        shutil.copy2(built, output)
        digest = hashlib.sha256(output.read_bytes()).hexdigest()
        output.with_suffix('.sha256').write_text(f'{digest}  {output.name}\n', encoding='ascii')
        output.with_suffix('.manifest.json').write_text(json.dumps(manifest, indent=2) + '\n', encoding='utf-8')
        print(f'Built {output} ({output.stat().st_size / 1024**2:.1f} MiB)\nSHA256 {digest}')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, default=windows.WORK / 'game/Hard Time.exe')
    parser.add_argument('--output', type=Path, default=ROOT / 'dist/Hard-Time-Setup.exe')
    args = parser.parse_args()
    package(args.binary.resolve(), args.output.resolve())


if __name__ == '__main__':
    main()
