"""Compare two executables in disposable copies of the same crowded save.

Requires an existing native installation containing an occupied slot 2.
The supplied installation is read only; each run gets a separate directory.
AI remains active, so results are a scene sample, not a deterministic benchmark.
"""
import argparse
import csv
import ctypes
from ctypes import wintypes
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import struct
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
import windows
spec = importlib.util.spec_from_file_location('installer', ROOT / 'scripts/install-windows.py')
installer = importlib.util.module_from_spec(spec)
spec.loader.exec_module(installer)

user32 = ctypes.WinDLL('user32', use_last_error=True)
callback_type = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)
user32.EnumWindows.argtypes = [callback_type, wintypes.LPARAM]
user32.GetWindowThreadProcessId.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.DWORD)]
user32.GetClassNameW.argtypes = [wintypes.HWND, wintypes.LPWSTR, ctypes.c_int]
user32.PostMessageW.argtypes = [wintypes.HWND, wintypes.UINT, wintypes.WPARAM, wintypes.LPARAM]


def window(pid):
    found = []
    @callback_type
    def visit(handle, parameter):
        owner = wintypes.DWORD()
        user32.GetWindowThreadProcessId(handle, ctypes.byref(owner))
        name = ctypes.create_unicode_buffer(64)
        user32.GetClassNameW(handle, name, len(name))
        if owner.value == pid and name.value == 'SDL_app':
            found.append(handle)
        return True
    user32.EnumWindows(visit, 0)
    if len(found) != 1:
        raise RuntimeError(f'Expected one game window, found {len(found)}')
    return found[0]


def key(pid, code):
    handle = window(pid)
    scan = user32.MapVirtualKeyW(code, 0)
    flags = 1 | scan << 16 | (0x1000000 if 33 <= code <= 46 else 0)
    user32.PostMessageW(handle, 0x100, code, flags)
    time.sleep(0.07)
    user32.PostMessageW(handle, 0x101, code, flags | 0xc0000000)
    time.sleep(0.85)


def crowd(data, shadows):
    slot = data / 'Slot02'
    progress = bytearray((slot / 'Progress.dat').read_bytes())
    count, player, _, location = struct.unpack_from('<4i', progress)
    if not 1 <= player <= count:
        raise ValueError('Slot 2 must contain a playable save')
    struct.pack_into('<i', progress, 12, 9)
    struct.pack_into('<3i', progress, 24, 0, 0, 12)
    (slot / 'Progress.dat').write_bytes(progress)
    chosen = [n for n in range(4, count + 1) if n != player][:19]
    for n in range(1, count + 1):
        path = slot / f'Character{n:03}.dat'
        content = bytearray(path.read_bytes())
        start = 4 + struct.unpack_from('<i', content)[0]
        if n in chosen or n == player:
            struct.pack_into('<i', content, start + 92 * 4, 9)
            # Existing held weapons belong to their old rooms. Do not create
            # a cross-room carrier reference when relocating the characters.
            struct.pack_into('<i', content, start + 58 * 4, 0)
            if n == player:
                xyz = (0, 10, -130, 0)
            else:
                order = chosen.index(n)
                xyz = ((order % 5 - 2) * 35, 10, -65 + (order // 5) * 35, 180)
            struct.pack_into('<4f', content, start + 96 * 4, *xyz)
        elif struct.unpack_from('<i', content, start + 92 * 4)[0] == 9:
            struct.pack_into('<i', content, start + 92 * 4, 1)
        path.write_bytes(content)
    # Both executables use 800x600. Disable focus pausing in the new build so
    # diagnostics or a locked desktop cannot silently turn the trial into pause.
    options = bytearray((data / 'Options.dat').read_bytes())
    struct.pack_into('<i', options, 0, 2)
    struct.pack_into('<i', options, 12, shadows)
    (data / 'Options.dat').write_bytes(options)
    settings = [2, 0, 100, 100, 100, 0, 100, 100, 100, 200, 208, 203, 205, 30, 44, 31, 45, 25]
    checksum = 1213484356
    for value in settings:
        checksum ^= value
    (data / 'NativeOptions.dat').write_bytes(struct.pack('<21i', 1213484356, 1, *settings, checksum))
    return len(chosen) + 1


def trial(label, binary, args):
    destination = (args.output / label).resolve()
    if destination.exists():
        raise ValueError(f'Choose a fresh output directory: {destination}')
    installer.install(binary, args.assets, destination, windows.runtime_files(True), True)
    population = crowd(destination / 'Data', args.shadows)
    env = {**os.environ, 'ALSOFT_DRIVERS': 'null', 'GALLIUM_DRIVER': 'llvmpipe',
           'HARDTIME_FPS_LOG': '1', 'HARDTIME_SHOW_FPS': '1', 'HARDTIME_ORIGINAL_FPS': '0'}
    with (destination / 'benchmark.log').open('wb') as log:
        child = subprocess.Popen([str(destination / 'Hard Time.exe')], cwd=destination, env=env,
                                 stdout=log, stderr=subprocess.STDOUT, creationflags=subprocess.CREATE_NO_WINDOW)
        try:
            # Startup loads all original textures synchronously and varies
            # substantially between the two renderers on a software driver.
            time.sleep(8)
            key(child.pid, 123)
            for code in (13, 40, 13):
                key(child.pid, code)
            key(child.pid, 123)
            deadline = time.monotonic() + 90
            while not (destination / 'frame-times.csv').exists():
                if child.poll() is not None or time.monotonic() > deadline:
                    raise RuntimeError('Game did not enter the benchmark scene')
                time.sleep(0.1)
            print(f'{label}: measuring {population} characters for {args.seconds} seconds', flush=True)
            time.sleep(args.warmup + args.seconds)
            key(child.pid, 123)
            user32.PostMessageW(window(child.pid), 0x10, 0, 0)
            if child.wait(timeout=45) != 0:
                raise RuntimeError('Benchmark exited unsuccessfully')
        finally:
            if child.poll() is None:
                child.kill()
                child.wait()
    rows = []
    elapsed = 0
    for row in csv.DictReader((destination / 'frame-times.csv').open()):
        start = elapsed
        elapsed += int(row['elapsed_ms'])
        if start >= args.warmup * 1000:
            rows.append(row)
    if len(rows) < 4:
        raise RuntimeError('Too few steady scene samples')
    elapsed = sum(int(row['elapsed_ms']) for row in rows)
    result = {'name': label, 'executable_sha256': hashlib.sha256(Path(binary).read_bytes()).hexdigest(),
              'characters': population, 'shadows': args.shadows, 'sample_ms': elapsed,
              'fps': 1000 * sum(int(row['frames']) for row in rows) / elapsed,
              'simulation_hz': 1000 * sum(int(row['ticks']) for row in rows) / elapsed,
              'dialogue_samples': sum(int(row['promo']) > 0 for row in rows), 'rows': rows}
    print(json.dumps({key: value for key, value in result.items() if key != 'rows'}), flush=True)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--assets', type=Path, required=True)
    parser.add_argument('--baseline', type=Path, required=True)
    parser.add_argument('--candidate', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--seconds', type=int, default=24)
    parser.add_argument('--warmup', type=int, default=8)
    parser.add_argument('--shadows', type=int, choices=(0, 1, 2), default=2)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    results = [trial('baseline', args.baseline, args), trial('candidate', args.candidate, args)]
    (args.output / 'results.json').write_text(json.dumps(results, indent=2) + '\n')


if __name__ == '__main__':
    main()
