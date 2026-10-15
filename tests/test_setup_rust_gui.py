#!/usr/bin/env python3
"""Exercise the real Rust installer controls and Play button on a Windows desktop.

Uses only Python's standard library. Installs into a new evidence folder and
closes its own game after capturing the menu. No personal shortcuts are made.
"""
import argparse
import ctypes as c
from ctypes import wintypes as w
import hashlib
import json
import os
from pathlib import Path
import struct
import subprocess
import time
import uuid
import zlib

ROOT = Path(__file__).resolve().parents[1]
u = c.WinDLL('user32', use_last_error=True)
g = c.WinDLL('gdi32', use_last_error=True)
k = c.WinDLL('kernel32', use_last_error=True)
dwm = c.WinDLL('dwmapi', use_last_error=True)
CALLBACK = c.WINFUNCTYPE(w.BOOL, w.HWND, w.LPARAM)


def bind(library, name, args, result):
    function = getattr(library, name)
    function.argtypes, function.restype = args, result
    return function


bind(u, 'EnumWindows', [CALLBACK, w.LPARAM], w.BOOL)
bind(u, 'GetWindowThreadProcessId', [w.HWND, c.POINTER(w.DWORD)], w.DWORD)
bind(u, 'IsWindowVisible', [w.HWND], w.BOOL)
bind(u, 'GetClassNameW', [w.HWND, w.LPWSTR, c.c_int], c.c_int)
bind(u, 'GetDlgItem', [w.HWND, c.c_int], w.HWND)
bind(u, 'SendMessageW', [w.HWND, w.UINT, w.WPARAM, w.LPARAM], w.LPARAM)
bind(u, 'PostMessageW', [w.HWND, w.UINT, w.WPARAM, w.LPARAM], w.BOOL)
bind(u, 'SetForegroundWindow', [w.HWND], w.BOOL)
bind(u, 'GetWindowRect', [w.HWND, c.POINTER(w.RECT)], w.BOOL)
bind(u, 'GetDC', [w.HWND], w.HDC)
bind(u, 'ReleaseDC', [w.HWND, w.HDC], c.c_int)
bind(u, 'IsWindowEnabled', [w.HWND], w.BOOL)
bind(g, 'CreateCompatibleDC', [w.HDC], w.HDC)
bind(g, 'CreateCompatibleBitmap', [w.HDC, c.c_int, c.c_int], w.HBITMAP)
bind(g, 'SelectObject', [w.HDC, w.HGDIOBJ], w.HGDIOBJ)
bind(g, 'DeleteObject', [w.HGDIOBJ], w.BOOL)
bind(g, 'DeleteDC', [w.HDC], w.BOOL)
bind(g, 'BitBlt', [w.HDC, c.c_int, c.c_int, c.c_int, c.c_int, w.HDC, c.c_int, c.c_int, w.DWORD], w.BOOL)
bind(g, 'GetDIBits', [w.HDC, w.HBITMAP, w.UINT, w.UINT, c.c_void_p, c.c_void_p, w.UINT], c.c_int)
bind(k, 'OpenProcess', [w.DWORD, w.BOOL, w.DWORD], w.HANDLE)
bind(k, 'QueryFullProcessImageNameW', [w.HANDLE, w.DWORD, w.LPWSTR, c.POINTER(w.DWORD)], w.BOOL)
bind(k, 'CloseHandle', [w.HANDLE], w.BOOL)
bind(dwm, 'DwmGetWindowAttribute', [w.HWND, w.DWORD, c.c_void_p, w.DWORD], c.c_long)


def windows(pid=None, executable=None):
    found = []
    @CALLBACK
    def visit(hwnd, _):
        current = w.DWORD()
        u.GetWindowThreadProcessId(hwnd, c.byref(current))
        if not u.IsWindowVisible(hwnd) or (pid is not None and current.value != pid):
            return True
        if executable:
            handle = k.OpenProcess(0x1000, False, current.value)
            if not handle:
                return True
            try:
                path, size = c.create_unicode_buffer(32768), w.DWORD(32768)
                if not k.QueryFullProcessImageNameW(handle, 0, path, c.byref(size)):
                    return True
                if os.path.normcase(path.value) != os.path.normcase(str(executable)):
                    return True
            finally:
                k.CloseHandle(handle)
        name = c.create_unicode_buffer(256)
        u.GetClassNameW(hwnd, name, len(name))
        found.append((hwnd, name.value, current.value))
        return True
    u.EnumWindows(visit, 0)
    return found


def text(hwnd):
    value = c.create_unicode_buffer(32768)
    u.SendMessageW(hwnd, 0x000D, len(value), c.addressof(value))  # WM_GETTEXT
    return value.value


def set_text(hwnd, value):
    buffer = c.create_unicode_buffer(str(value))
    assert u.SendMessageW(hwnd, 0x000C, 0, c.addressof(buffer))  # WM_SETTEXT


def capture(hwnd, path):
    u.SetForegroundWindow(hwnd)
    time.sleep(0.3)
    rect = w.RECT()
    assert u.GetWindowRect(hwnd, c.byref(rect))
    # Exclude invisible resize borders so the screenshot contains only this app.
    dwm.DwmGetWindowAttribute(hwnd, 9, c.byref(rect), c.sizeof(rect))
    width, height = rect.right - rect.left, rect.bottom - rect.top
    screen = u.GetDC(None)
    memory = g.CreateCompatibleDC(screen)
    bitmap = g.CreateCompatibleBitmap(screen, width, height)
    previous = g.SelectObject(memory, bitmap)
    try:
        assert g.BitBlt(memory, 0, 0, width, height, screen, rect.left, rect.top, 0x00CC0020)
        g.SelectObject(memory, previous)
        pixels = c.create_string_buffer(width * height * 4)
        header = c.create_string_buffer(struct.pack('<IiiHHIIiiII', 40, width, -height, 1, 32, 0, len(pixels), 0, 0, 0, 0))
        assert g.GetDIBits(screen, bitmap, 0, height, pixels, header, 0) == height
        raw = pixels.raw
        scanlines = bytearray()
        for y in range(height):
            scanlines.append(0)
            row = raw[y * width * 4:(y + 1) * width * 4]
            rgb = bytearray(width * 3)
            rgb[0::3], rgb[1::3], rgb[2::3] = row[2::4], row[1::4], row[0::4]
            scanlines.extend(rgb)
        def chunk(name, data):
            return struct.pack('>I', len(data)) + name + data + struct.pack('>I', zlib.crc32(name + data))
        path.write_bytes(b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', width, height, 8, 2, 0, 0, 0))
                         + chunk(b'IDAT', zlib.compress(scanlines)) + chunk(b'IEND', b''))
    finally:
        g.DeleteObject(bitmap)
        g.DeleteDC(memory)
        u.ReleaseDC(None, screen)


def hashes(folder):
    return {p.relative_to(folder).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in sorted(folder.rglob('*')) if p.is_file()}


def wait_for(function, seconds=20):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        value = function()
        if value:
            return value
        time.sleep(0.1)
    raise AssertionError('Timed out waiting for the Windows application')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--setup', type=Path, default=ROOT / 'dist/Hard-Time-Setup.exe')
    parser.add_argument('--original-installer', type=Path, default=Path.home() / 'Downloads/hardtime_setup.exe')
    parser.add_argument('--download-original', action='store_true')
    args = parser.parse_args()
    setup = args.setup.resolve(strict=True)
    evidence = ROOT / '.work/validation/setup' / ('gui-rust-' + uuid.uuid4().hex[:8])
    evidence.mkdir(parents=True)
    destination = evidence / 'Installed game 猫'
    environment = os.environ.copy()
    environment['PATH'] = str(Path(environment['WINDIR']) / 'System32')
    process = subprocess.Popen([str(setup)], env=environment)
    game_window = None
    try:
        hwnd = wait_for(lambda: next((item[0] for item in windows(pid=process.pid) if item[1] == 'HardTimeRustSetup'), None))
        control = lambda identifier: u.GetDlgItem(hwnd, identifier)
        set_text(control(101), args.original_installer.resolve())
        set_text(control(102), destination)
        u.SendMessageW(control(103), 0x00F1, int(args.download_original), 0)  # BM_SETCHECK
        u.SendMessageW(hwnd, 0x0111, 103, control(103))  # notify the real download handler
        u.SendMessageW(control(105), 0x00F1, 0, 0)
        assert text(control(102)) == str(destination) and not destination.exists()
        capture(hwnd, evidence / 'setup.png')
        # Cancel while payload verification is beginning, then retry through the
        # same controls. This exercises the UI worker and cancellation lifetime.
        u.SendMessageW(control(106), 0x00F5, 0, 0)
        u.SendMessageW(control(107), 0x00F5, 0, 0)
        wait_for(lambda: text(control(111)).startswith('Setup cancelled.') and u.IsWindowEnabled(control(106)), 15)
        assert not destination.exists()
        u.SendMessageW(control(106), 0x00F5, 0, 0)  # BM_CLICK Install
        def completed():
            dialogs = [item for item in windows(pid=process.pid) if item[1] == '#32770']
            if dialogs:
                capture(dialogs[0][0], evidence / 'error.png')
                raise AssertionError('Installer displayed an error; see ' + str(evidence / 'error.png'))
            assert process.poll() is None, 'Setup exited during installation'
            return text(control(106)) == 'Play Hard Time' and u.IsWindowEnabled(control(106))
        wait_for(completed, 240)
        capture(hwnd, evidence / 'installed.png')
        marker = json.loads((destination / '.hardtime-native-install.json').read_text())
        before = hashes(destination / 'Data')
        u.SendMessageW(control(106), 0x00F5, 0, 0)  # actual completion button
        assert process.wait(timeout=10) == 0
        game_window, _, game_pid = wait_for(lambda: next(iter(windows(executable=destination / 'Hard Time.exe')), None), 30)
        wait_for(lambda: len(list((destination / 'backups').iterdir())) == 1)
        backup = next((destination / 'backups').iterdir())
        assert hashes(backup) == before
        time.sleep(8)
        # The engine replaces its 400x300 initialization window with the menu.
        game_window, _, game_pid = wait_for(lambda: next(iter(windows(executable=destination / 'Hard Time.exe')), None), 30)
        capture(game_window, evidence / 'game.png')
        u.PostMessageW(game_window, 0x0010, 0, 0)  # close only this disposable game
        wait_for(lambda: not windows(executable=destination / 'Hard Time.exe'), 30)
        wait_for(lambda: (destination / 'logs/latest.txt').is_file(), 15)
        latest = (destination / 'logs/latest.txt').read_text(encoding='utf-8')
        assert '0x0' in latest or 'exit code: 0' in latest, latest
        record = {'setup_sha256': hashlib.sha256(setup.read_bytes()).hexdigest(),
                  'destination': str(destination), 'download_tested': args.download_original,
                  'renderer': marker['software_renderer'] and 'software' or 'gpu',
                  'installer_language': marker['installer'], 'windows_only_path': True,
                  'play_button': True, 'backup_matches_seed_data': True, 'game_closed_cleanly': True,
                  'cancel_and_retry': True,
                  'game_pid': game_pid, 'evidence': str(evidence)}
        (evidence / 'result.json').write_text(json.dumps(record, indent=2) + '\n', encoding='utf-8')
        (ROOT / '.work/validation/setup/gui-rust-latest.json').write_text(json.dumps(record, indent=2) + '\n', encoding='utf-8')
        print('PASS real Rust GUI install, Play button, backup, game menu and clean exit')
        print('Evidence: ' + str(evidence))
    finally:
        for current, _, _ in windows(executable=destination / 'Hard Time.exe'):
            u.PostMessageW(current, 0x0010, 0, 0)
        if process.poll() is None:
            for handle, kind, _ in windows(pid=process.pid):
                if kind == '#32770':
                    u.PostMessageW(handle, 0x0111, 1, 0)
                else:
                    u.PostMessageW(handle, 0x0010, 0, 0)
            try:
                process.wait(timeout=15)
            except subprocess.TimeoutExpired:
                process.terminate()
                process.wait(timeout=10)


if __name__ == '__main__':
    main()
