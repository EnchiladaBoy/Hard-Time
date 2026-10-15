#!/usr/bin/env python3
"""Compile and run isolated regressions on Linux ARM64 or Windows x64."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import struct
import subprocess
import sys
import tempfile
import time
import zlib

HERE = Path(__file__).resolve().parent
WINDOWS = os.name == "nt"
COMPILER = "blitzcc64.exe" if WINDOWS else "blitzcc"
WINDOWS_FONTS = {"Comfortaa": "Comic Sans MS", "Liberation Sans:style=Bold": "Arial",
                 "Liberation Sans": "Arial"}
CORE = ["boolean-smoke", "rounding-smoke", "semantics"]
GRAPHICS = ["runtime-smoke", "graphics-smoke", "material-smoke"]
FONTS = ["font-smoke", "hud-fit-smoke"]
WINDOWS_GRAPHICS = ["texture-alpha-smoke", "window-close-smoke", "capture-repeat-smoke",
                    "timer-poll-smoke", "interpolation-smoke"]
RESULT_FILES = {
    "semantics": "semantics-results.txt",
    "graphics-postflip-smoke": "graphics-smoke-result.txt",
}


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def verify_sources() -> None:
    manifest = json.loads((HERE / "SOURCE-MANIFEST.json").read_text())
    for name, expected in manifest["sha256"].items():
        actual = hashlib.sha256((HERE / name).read_bytes()).hexdigest()
        require(actual == expected, f"Source hash differs from recorded regression: {name}")


def bmp_pixel(path: Path, width: int, height: int, x: int, y: int) -> tuple[int, int, int]:
    data = path.read_bytes()
    require(data[:2] == b"BM", f"{path.name}: missing BMP signature")
    offset = struct.unpack_from("<I", data, 10)[0]
    actual_width, actual_height = struct.unpack_from("<ii", data, 18)
    planes, bits, compression = struct.unpack_from("<HHI", data, 26)
    require((actual_width, abs(actual_height)) == (width, height), f"{path.name}: dimensions")
    require((planes, bits, compression) == (1, 24, 0), f"{path.name}: expected plain 24-bit BMP")
    row = height - 1 - y if actual_height > 0 else y
    stride = (width * 3 + 3) & ~3
    start = offset + row * stride + x * 3
    pixel = data[start:start + 3]
    require(len(pixel) == 3, f"{path.name}: truncated pixels")
    return tuple(reversed(pixel))


def validate_result(name: str, work: Path) -> None:
    output_name = RESULT_FILES.get(name, name + "-result.txt")
    lines = (work / output_name).read_text().splitlines()
    expected = HERE / "expected" / output_name
    if expected.exists():
        require(lines == expected.read_text().splitlines(), f"{output_name}: expected-output mismatch")
    elif name == "runtime-smoke":
        values = dict(line.split("=", 1) for line in lines)
        require(int(values["Ticks"]) >= 5, "Timer lost initial ticks")
        require(int(values["Catchup"]) >= 3, "Timer failed elapsed-tick catch-up")
        require(250 <= int(values["Elapsed"]) <= 1000, "Timer cadence outside test bounds")
        wanted = struct.pack("<4i2fI", -(2**31), -1, 0, 2**31 - 1, 1.25, -2.5, 9) + b"Hard Time"
        require((work / "runtime-smoke.bin").read_bytes() == wanted, "int32/float32/string byte format changed")
    elif name == "timer-poll-smoke":
        values = dict(line.split("=", 1) for line in lines)
        elapsed = int(values["Elapsed"])
        ticks = int(values["Ticks"])
        require(1000 <= elapsed <= 2000, "Poll test stalled")
        require(abs(ticks - elapsed * 0.03) <= 2, "Polling changed the 30 Hz simulation rate")
        require(int(values["Renders"]) > ticks * 1.5, "Rendering did not exceed the simulation rate")
        require(int(values["EmptyPolls"]) > 0, "PollTimer never returned without a tick")
        require(int(values["Catchup"]) >= 3 and int(values["Waited"]) >= 1,
                "Poll/Wait interoperability lost ticks")
    elif name == "font-smoke":
        require(len(lines) == 11 and lines[-1] == "PASS", "Font lifecycle test incomplete")
        for iteration, line in enumerate(lines[:-1], 1):
            match = re.fullmatch(r"Iteration=(\d+) Width=(\d+) Height=(\d+)", line)
            require(match is not None, "Unexpected font result")
            current, width, height = map(int, match.groups())
            require(current == iteration and width > 0 and height == 96, "Font metrics/lifecycle mismatch")
    elif name == "hud-fit-smoke":
        require(len(lines) == 31 and lines[-1] == "PASS", "HUD layout test incomplete")
        for line in lines[:-1]:
            require(float(line.split("outline-gap=", 1)[1]) >= 0, "HUD label/value overlap")
    else:
        raise ValueError(f"No result validator for {name}")

    if name in ("graphics-smoke", "graphics-postflip-smoke"):
        checks = [
            ("graphics-grab.bmp", 320, 240, 10, 10, (255, 0, 0)),
            ("graphics-grab.bmp", 320, 240, 200, 200, (0, 255, 0)),
            ("graphics-resized.bmp", 160, 120, 10, 10, (255, 0, 0)),
            ("graphics-resized.bmp", 160, 120, 120, 100, (0, 255, 0)),
            ("graphics-drawn.bmp", 320, 240, 120, 100, (0, 255, 0)),
            ("graphics-image-buffer.bmp", 20, 20, 2, 2, (0, 255, 255)),
        ]
        for filename, width, height, x, y, color in checks:
            require(bmp_pixel(work / filename, width, height, x, y) == color, f"{filename}: pixel {x},{y}")


def resolve_blitzpath(args: argparse.Namespace) -> Path:
    explicit = args.blitzpath or os.environ.get("BLITZPATH") or os.environ.get("blitzpath")
    if explicit:
        candidates = [Path(explicit).expanduser()]
    else:
        root = Path(args.build_root).expanduser()
        candidates = [root / "release", root / "engine" / "_release", root / "_release", root]
    for candidate in candidates:
        if (candidate / "bin" / COMPILER).is_file():
            return candidate.resolve()
    raise ValueError("Built blitzcc not found; set BLITZPATH to the release directory or use --build-root")


def run_close_test(binary, work, env, log, timeout):
    """Send WM_CLOSE only to this child process's SDL window, then check cleanup."""
    import ctypes
    from ctypes import wintypes
    user32 = ctypes.WinDLL("user32", use_last_error=True)
    callback_type = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)
    user32.EnumWindows.argtypes = [callback_type, wintypes.LPARAM]
    user32.GetWindowThreadProcessId.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.DWORD)]
    user32.GetClassNameW.argtypes = [wintypes.HWND, wintypes.LPWSTR, ctypes.c_int]
    user32.PostMessageW.argtypes = [wintypes.HWND, wintypes.UINT, wintypes.WPARAM, wintypes.LPARAM]
    with subprocess.Popen([str(binary)], cwd=work, env=env, stdout=log, stderr=subprocess.STDOUT) as child:
        deadline = time.monotonic() + timeout
        try:
            while not (work / "ready.txt").exists():
                require(child.poll() is None, "Close test exited before ready")
                require(time.monotonic() < deadline, "Close test did not become ready")
                time.sleep(0.05)
            windows = []
            @callback_type
            def visit(handle, parameter):
                owner = wintypes.DWORD()
                user32.GetWindowThreadProcessId(handle, ctypes.byref(owner))
                name = ctypes.create_unicode_buffer(256)
                user32.GetClassNameW(handle, name, 256)
                if owner.value == child.pid and name.value == "SDL_app":
                    windows.append(handle)
                return True
            user32.EnumWindows(visit, 0)
            require(len(windows) == 1, "Expected the close test's SDL window")
            require(user32.PostMessageW(windows[0], 0x10, 0, 0), "WM_CLOSE delivery failed")
            require(child.wait(timeout=max(0.1, deadline - time.monotonic())) == 0, "Close test failed")
        finally:
            if child.poll() is None:
                child.kill()
                child.wait()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--blitzpath", help="Release directory containing bin/blitzcc and ARM64 runtime")
    parser.add_argument("--build-root", default=os.environ.get("HARDTIME_WORK_DIR") or os.environ.get("HARDTIME_BUILD_ROOT") or os.environ.get("BUILD_ROOT") or str(HERE.parent / (".work/windows" if WINDOWS else ".work")))
    parser.add_argument("--noninteractive", action="store_true", help="Run the default core numeric/boolean subset")
    parser.add_argument("--graphics", action="store_true", help="Also verify timers, save byte format, image/BMP operations and 3D materials")
    parser.add_argument("--fonts", action="store_true", help="Also check installed Comfortaa and Liberation Sans fonts")
    parser.add_argument("--post-flip", action="store_true", help="Also check driver-sensitive backbuffer retention after Flip")
    parser.add_argument("--all", action="store_true", help="Run core, graphics and font tests; post-Flip still requires --post-flip")
    parser.add_argument("--verify-sources", action="store_true", help="Verify copied-source hashes and exit without building or running")
    parser.add_argument("--timeout", type=float, default=30, help="Seconds per test executable (default: 30)")
    parser.add_argument("--software-renderer", action="store_true", help="Use the pinned app-local Mesa renderer on Windows")
    parser.add_argument("--only", choices=CORE + GRAPHICS + FONTS + ["graphics-postflip-smoke"] + WINDOWS_GRAPHICS, help="Run a single regression")
    args = parser.parse_args()
    try:
        verify_sources()
        if args.verify_sources:
            print("PASS copied regression source hashes")
            return 0
        require(args.timeout > 0, "--timeout must be positive")
        native_linux = platform.system() == "Linux" and platform.machine().lower() in ("aarch64", "arm64")
        native_windows = WINDOWS and platform.machine().lower() in ("amd64", "x86_64")
        require(native_linux or native_windows, "These tests require Linux ARM64 or Windows x64")
        dlls = []
        if WINDOWS:
            sys.path.insert(0, str(HERE.parent / "scripts"))
            import windows
            windows.developer_environment()
            dlls = windows.runtime_files(args.software_renderer)
        release = resolve_blitzpath(args)
        env = os.environ.copy()
        env["blitzpath"] = str(release)
        env.setdefault("SDL_VIDEODRIVER", "windows" if WINDOWS else "x11")
        env.setdefault("ALSOFT_DRIVERS", "null")
        if WINDOWS and args.software_renderer:
            env["GALLIUM_DRIVER"] = "llvmpipe"
        if not WINDOWS:
            require(bool(env.get("DISPLAY")), "An OpenGL-capable X display is required; use xvfb-run or your desktop DISPLAY")
        names = list(CORE)
        if args.graphics or args.all:
            names += GRAPHICS
            if WINDOWS:
                names += WINDOWS_GRAPHICS
        if args.fonts or args.all:
            names += FONTS
        if args.post_flip:
            names.append("graphics-postflip-smoke")
        if args.only:
            names = [args.only]
        work_root = Path(tempfile.mkdtemp(prefix="hardtime-native-tests-"))
        print(f"Test outputs: {work_root}", flush=True)
        results = []
        for name in names:
            work = work_root / name
            work.mkdir()
            binary = work / (name + ".exe" if WINDOWS else name)
            result = {"name": name, "passed": False}
            try:
                if name == "texture-alpha-smoke":
                    def chunk(kind, data):
                        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))
                    # White RGB at 25% opacity must retain its supplied alpha.
                    png = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">2I5B", 8, 8, 8, 6, 0, 0, 0))
                    png += chunk(b"IDAT", zlib.compress((b"\0" + bytes((255, 255, 255, 64)) * 8) * 8)) + chunk(b"IEND", b"")
                    (work / "alpha-rgba.png").write_bytes(png)
                for dll, filename in dlls:
                    shutil.copy2(dll, work / filename)
                source = HERE / "sources" / (name + ".bb")
                if WINDOWS and name in FONTS:
                    # The assertions are unchanged; exercise the game's Windows font families.
                    text = source.read_text()
                    for old, new in WINDOWS_FONTS.items():
                        text = text.replace('"' + old + '"', '"' + new + '"')
                    source = work / (name + ".bb")
                    source.write_text(text)
                with (work / "build.log").open("wb") as log:
                    subprocess.run([str(release / "bin" / COMPILER), "-r", "opengl", "-o", str(binary), str(source)], cwd=work, env=env, stdout=log, stderr=subprocess.STDOUT, check=True, timeout=120)
                header = binary.read_bytes()
                if WINDOWS:
                    require(header[:2] == b"MZ", "Compiler did not emit a Windows executable")
                    pe = struct.unpack_from("<I", header, 0x3c)[0]
                    require(header[pe:pe+4] == b"PE\0\0" and struct.unpack_from("<H", header, pe+4)[0] == 0x8664 and struct.unpack_from("<H", header, pe+24)[0] == 0x20b,
                            "Compiler did not emit PE32+ x86-64")
                else:
                    require(header[:6] == b"\x7fELF\x02\x01" and int.from_bytes(header[18:20], "little") == 183, "Compiler did not emit ELF64 little-endian AArch64")
                with (work / "run.log").open("wb") as log:
                    if name == "window-close-smoke":
                        require(WINDOWS, "Window close test requires Windows")
                        run_close_test(binary, work, env, log, args.timeout)
                    else:
                        subprocess.run([str(binary)], cwd=work, env=env, stdout=log, stderr=subprocess.STDOUT, check=True, timeout=args.timeout)
                validate_result(name, work)
                result["passed"] = True
                print(f"PASS {name}", flush=True)
            except (OSError, ValueError, KeyError, struct.error, subprocess.SubprocessError) as error:
                result["error"] = str(error)
                print(f"FAIL {name}: {error}", file=sys.stderr, flush=True)
            results.append(result)
        (work_root / "results.json").write_text(json.dumps({"platform": platform.platform(), "software_renderer": args.software_renderer, "tests": results}, indent=2) + "\n")
        return 0 if all(result["passed"] for result in results) else 1
    except (OSError, ValueError, json.JSONDecodeError) as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
