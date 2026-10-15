#!/usr/bin/env python3
"""Build the native x64 Windows game from pinned sources and local game assets."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
WORK = Path(os.environ.get('HARDTIME_WORK_DIR', ROOT / '.work/windows')).resolve()
CACHE = ROOT / '.work'
LOCK = json.loads((ROOT / 'scripts/windows-toolchain.json').read_text(encoding='utf-8'))


def run(*args, cwd=None):
    print('+', ' '.join(map(str, args)), flush=True)
    subprocess.run([str(a) for a in args], cwd=cwd, check=True)


def find_tool(name, *candidates):
    found = shutil.which(name)
    if found:
        return Path(found)
    for candidate in candidates:
        if Path(candidate).is_file():
            return Path(candidate)
    raise RuntimeError(f'{name} is required. See scripts/README-windows.txt.')


def developer_environment():
    if os.name != 'nt' or platform.machine().lower() not in ('amd64', 'x86_64'):
        raise RuntimeError('This workflow requires native x64 Windows.')
    pf = Path(os.environ.get('ProgramFiles', r'C:\Program Files'))
    pf86 = Path(os.environ.get('ProgramFiles(x86)', r'C:\Program Files (x86)'))
    local = Path(os.environ['LOCALAPPDATA'])
    git = find_tool('git', pf / 'Git/cmd/git.exe', local / 'Programs/Git/cmd/git.exe')
    cmake = find_tool('cmake', pf / 'CMake/bin/cmake.exe')
    vswhere = find_tool('vswhere', pf86 / 'Microsoft Visual Studio/Installer/vswhere.exe')
    installation = subprocess.check_output([
        str(vswhere), '-latest', '-products', '*', '-requires',
        'Microsoft.VisualStudio.Component.VC.Tools.x86.x64', '-property',
        'installationPath'], text=True).strip()
    if not installation:
        raise RuntimeError('Install Visual Studio 2022 Build Tools with Desktop development with C++.')
    vs = Path(installation)
    devcmd = vs / 'Common7/Tools/VsDevCmd.bat'
    # cmd is needed only to import the Visual Studio environment, not for file operations.
    command = f'cmd.exe /d /s /c ""{devcmd}" -no_logo -arch=x64 -host_arch=x64 >nul && set"'
    output = subprocess.check_output(command, text=True, errors='replace')
    for line in output.splitlines():
        key, sep, value = line.partition('=')
        if sep and key:
            os.environ[key] = value
    ninja = find_tool('ninja', vs / 'Common7/IDE/CommonExtensions/Microsoft/CMake/Ninja/ninja.exe')
    os.environ['PATH'] = os.pathsep.join([str(git.parent), str(cmake.parent), str(ninja.parent), os.environ['PATH']])
    return cmake


def dependency(name):
    entry = LOCK[name]
    downloads = CACHE / 'downloads'
    downloads.mkdir(parents=True, exist_ok=True)
    archive = downloads / entry['url'].rsplit('/', 1)[1]
    if not archive.exists():
        temporary = archive.with_suffix(archive.suffix + '.partial')
        print('Downloading', entry['url'], flush=True)
        with urllib.request.urlopen(entry['url'], timeout=60) as response, temporary.open('wb') as output:
            shutil.copyfileobj(response, output)
        temporary.replace(archive)
    with archive.open('rb') as source:
        digest = hashlib.file_digest(source, 'sha256').hexdigest()
    if digest != entry['sha256']:
        raise RuntimeError(f'Dependency checksum mismatch: {archive}')
    destination = CACHE / 'toolchain' / name
    marker = destination / '.verified-sha256'
    if not marker.exists() or marker.read_text().strip() != digest:
        sevenzip = find_tool('7z', Path(os.environ.get('ProgramFiles', r'C:\Program Files')) / '7-Zip/7z.exe')
        run(sevenzip, 'x', archive, '-o' + str(destination), '-y', '-bso0', '-bsp0')
        marker.write_text(digest + '\n')
    return destination


def runtime_files(software=False):
    openal = dependency('openal') / 'openal-soft-1.25.2-bin'
    files = [(openal / 'bin/Win64/soft_oal.dll', 'OpenAL32.dll')]
    if software:
        mesa = dependency('mesa') / 'x64'
        files += [(mesa / name, name) for name in ('opengl32.dll', 'libgallium_wgl.dll')]
    return files


def configure(cmake, jobs, configure_only=False):
    llvm = dependency('llvm') / 'llvm'
    openal = dependency('openal') / 'openal-soft-1.25.2-bin'
    os.environ['HARDTIME_PLATFORM'] = 'windows-x64'
    os.environ['HARDTIME_WORK_DIR'] = str(WORK)
    run(sys.executable, ROOT / 'scripts/prepare.py')
    run(cmake, '-S', WORK / 'engine', '-B', WORK / 'build', '-G', 'Ninja',
        '-DBB_PLATFORM=win64', '-DARCH=x86_64', '-DBB_ENV=release',
        '-DOUTPUT_PATH=' + (WORK / 'release').as_posix(),
        '-DLLVM_DIR=' + (llvm / 'lib/cmake/llvm').as_posix(),
        '-DLLD_DIR=' + (llvm / 'lib/cmake/lld').as_posix(),
        '-DOPENAL_INCLUDE_DIR=' + (openal / 'include').as_posix(),
        '-DOPENAL_LIBRARY=' + (openal / 'libs/Win64/OpenAL32.lib').as_posix(),
        '-DCMAKE_POLICY_VERSION_MINIMUM=3.5', '-DBB_HARDTIME_CLI=ON')
    if configure_only:
        return
    run(cmake, '--build', WORK / 'build', '--target', 'blitzcc',
        'runtime.opengl.shared', 'runtime.opengl.static', '-j', jobs)
    runtime_dll = openal / 'bin/Win64/soft_oal.dll'
    shutil.copy2(runtime_dll, WORK / 'release/bin/OpenAL32.dll')
    os.environ['blitzpath'] = str(WORK / 'release')
    # Legacy narrow Windows filesystem calls must round-trip non-ASCII user
    # directories. This is per process and does not change the system locale.
    shutil.copy2(ROOT / 'scripts/windows-game.manifest', WORK / 'release/bin/native.manifest')
    run(WORK / 'release/bin/blitzcc64.exe', '-r', 'opengl', '-o',
        'Hard Time.exe', 'Gameplay.bb', cwd=WORK / 'game')
    shutil.copy2(runtime_dll, WORK / 'game/OpenAL32.dll')
    print('Built:', WORK / 'game/Hard Time.exe')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--configure-only', action='store_true')
    parser.add_argument('--jobs', type=int, default=2)
    args = parser.parse_args()
    if args.jobs < 1:
        parser.error('--jobs must be positive')
    cmake = developer_environment()
    configure(cmake, args.jobs, args.configure_only)


if __name__ == '__main__':
    try:
        main()
    except (OSError, RuntimeError, subprocess.CalledProcessError) as error:
        print(f'Windows build failed: {error}', file=sys.stderr)
        sys.exit(1)
