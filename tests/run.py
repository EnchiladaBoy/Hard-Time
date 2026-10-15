#!/usr/bin/env python3
"""Compile and run isolated synthetic regressions for the native ARM64 port."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import struct
import subprocess
import sys
import tempfile

HERE = Path(__file__).resolve().parent
CORE = ["boolean-smoke", "rounding-smoke", "semantics"]
GRAPHICS = ["runtime-smoke", "graphics-smoke", "material-smoke"]
FONTS = ["font-smoke", "hud-fit-smoke"]
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
        if (candidate / "bin" / "blitzcc").is_file():
            return candidate.resolve()
    raise ValueError("Built blitzcc not found; set BLITZPATH to the release directory or use --build-root")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--blitzpath", help="Release directory containing bin/blitzcc and ARM64 runtime")
    parser.add_argument("--build-root", default=os.environ.get("HARDTIME_WORK_DIR") or os.environ.get("HARDTIME_BUILD_ROOT") or os.environ.get("BUILD_ROOT") or str(HERE.parent / ".work"))
    parser.add_argument("--noninteractive", action="store_true", help="Run the default core numeric/boolean subset")
    parser.add_argument("--graphics", action="store_true", help="Also verify timers, save byte format, image/BMP operations and 3D materials")
    parser.add_argument("--fonts", action="store_true", help="Also check installed Comfortaa and Liberation Sans fonts")
    parser.add_argument("--post-flip", action="store_true", help="Also check driver-sensitive backbuffer retention after Flip")
    parser.add_argument("--all", action="store_true", help="Run core, graphics and font tests; post-Flip still requires --post-flip")
    parser.add_argument("--verify-sources", action="store_true", help="Verify copied-source hashes and exit without building or running")
    parser.add_argument("--timeout", type=float, default=30, help="Seconds per test executable (default: 30)")
    args = parser.parse_args()
    try:
        verify_sources()
        if args.verify_sources:
            print("PASS copied regression source hashes")
            return 0
        require(args.timeout > 0, "--timeout must be positive")
        require(platform.system() == "Linux" and platform.machine().lower() in ("aarch64", "arm64"), "These tests run native Linux ARM64 executables")
        release = resolve_blitzpath(args)
        env = os.environ.copy()
        env["blitzpath"] = str(release)
        env.setdefault("SDL_VIDEODRIVER", "x11")
        env.setdefault("ALSOFT_DRIVERS", "null")
        require(bool(env.get("DISPLAY")), "An OpenGL-capable X display is required; use xvfb-run or your desktop DISPLAY")
        names = list(CORE)
        if args.graphics or args.all:
            names += GRAPHICS
        if args.fonts or args.all:
            names += FONTS
        if args.post_flip:
            names.append("graphics-postflip-smoke")
        work_root = Path(tempfile.mkdtemp(prefix="hardtime-native-tests-"))
        print(f"Test outputs: {work_root}", flush=True)
        results = []
        for name in names:
            work = work_root / name
            work.mkdir()
            binary = work / name
            result = {"name": name, "passed": False}
            try:
                with (work / "build.log").open("wb") as log:
                    subprocess.run([str(release / "bin" / "blitzcc"), "-r", "opengl", "-o", str(binary), str(HERE / "sources" / (name + ".bb"))], cwd=work, env=env, stdout=log, stderr=subprocess.STDOUT, check=True, timeout=120)
                header = binary.read_bytes()[:20]
                require(header[:6] == b"\x7fELF\x02\x01" and int.from_bytes(header[18:20], "little") == 183, "Compiler did not emit ELF64 little-endian AArch64")
                with (work / "run.log").open("wb") as log:
                    subprocess.run([str(binary)], cwd=work, env=env, stdout=log, stderr=subprocess.STDOUT, check=True, timeout=args.timeout)
                validate_result(name, work)
                result["passed"] = True
                print(f"PASS {name}", flush=True)
            except (OSError, ValueError, KeyError, struct.error, subprocess.SubprocessError) as error:
                result["error"] = str(error)
                print(f"FAIL {name}: {error}", file=sys.stderr, flush=True)
            results.append(result)
        (work_root / "results.json").write_text(json.dumps({"tests": results}, indent=2) + "\n")
        return 0 if all(result["passed"] for result in results) else 1
    except (OSError, ValueError, json.JSONDecodeError) as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
