#!/usr/bin/env python3
"""Fetch pinned upstream sources and apply the native platform patches."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
PLATFORM = os.environ.get("HARDTIME_PLATFORM", "windows-x64" if os.name == "nt" else "linux-arm64")
if PLATFORM not in ("linux-arm64", "windows-x64"):
    raise RuntimeError(f"Unsupported HARDTIME_PLATFORM: {PLATFORM}")
DEFAULT_WORK = ROOT / ".work" / "windows" if PLATFORM == "windows-x64" else ROOT / ".work"
WORK = Path(os.environ.get("HARDTIME_WORK_DIR", DEFAULT_WORK)).expanduser().resolve()
LOCK = json.loads((ROOT / "scripts/source-lock.json").read_text())
PATCHES = ["engine-linux-arm64.patch", "build-dependency-sdl.patch",
           "build-dependency-vorbis.patch", "portable-build.patch", "game-linux.patch"]
if PLATFORM == "windows-x64":
    PATCHES += ["engine-windows-x64.patch", "game-windows.patch",
                "engine-refresh-windows.patch", "game-refresh-windows.patch"]
SUBMODULES = ["crossguid/src", "enet/tree", "freeimage/src", "freetype2/tree",
              "json-hpp/tree", "libpng/tree", "minimp3/tree", "ogg/tree",
              "sdl/tree", "toml11/tree", "utf8.h/tree", "vorbis/tree", "zlib/tree"]
if PLATFORM == "windows-x64":
    SUBMODULES += ["stackwalker/tree"]


def run(*args, cwd=None, capture=False):
    if args[0] == "git":
        args = ("git", "-c", "core.autocrlf=false", *args[1:])
    return subprocess.run(args, cwd=cwd, check=True, text=True,
                          stdout=subprocess.PIPE if capture else None).stdout


def clone(repository, path, commit, reference=None):
    if path.exists() and any(path.iterdir()):
        raise RuntimeError(f"Refusing to replace existing checkout: {path}")
    if reference and (reference / ".git").exists():
        run("git", "clone", "--shared", "--no-checkout", str(reference), str(path))
    else:
        path.mkdir(parents=True, exist_ok=True)
        run("git", "init", str(path))
        run("git", "remote", "add", "origin", repository, cwd=path)
        run("git", "fetch", "--depth=1", "origin", commit, cwd=path)
    run("git", "checkout", "--detach", commit, cwd=path)


def apply(path, patch):
    # Apply against a directory rather than Git's index. This also supports
    # the dependency submodule hunk and the upstream non-Git BASIC source.
    with tempfile.TemporaryDirectory(prefix="hardtime-patch-") as scratch:
        args = ("git", "apply", "--unsafe-paths", "--directory=" + path.as_posix(), str(patch))
        run(*args[:2], "--check", *args[2:], cwd=scratch)
        run(*args, cwd=scratch)


def main():
    digest = hashlib.sha256((ROOT / "scripts/source-lock.json").read_bytes())
    for name in PATCHES:
        digest.update((ROOT / "patches" / name).read_bytes())
    stamp = digest.hexdigest()
    marker = WORK / "prepared.json"
    if marker.exists():
        previous = json.loads(marker.read_text())
        if previous.get("recipe_sha256") != stamp:
            raise RuntimeError("Build recipe changed. Choose a fresh HARDTIME_WORK_DIR.")
        if not (WORK / "engine/CMakeLists.txt").is_file() or not (WORK / "game/Gameplay.bb").is_file():
            raise RuntimeError("Prepared source tree is incomplete. Choose a fresh HARDTIME_WORK_DIR.")
        print(f"Pinned source tree already prepared: {WORK}")
        return
    WORK.mkdir(parents=True, exist_ok=True)
    engine = WORK / "engine"
    reference = os.environ.get("HARDTIME_ENGINE_REFERENCE")
    reference = Path(reference).expanduser().resolve() if reference else None
    clone(LOCK["engine_repository"], engine, LOCK["engine_commit"], reference)
    for submodule in SUBMODULES:
        relative = "deps/" + submodule
        source = reference / relative if reference else None
        if source and (source / ".git").exists():
            commit = run("git", "rev-parse", "HEAD:" + relative, cwd=engine, capture=True).strip()
            clone(str(source), engine / relative, commit, source)
        else:
            run("git", "submodule", "update", "--init", "--depth=1", "--", relative, cwd=engine)
    apply(engine, ROOT / "patches/engine-linux-arm64.patch")
    apply(engine / "deps/sdl/tree", ROOT / "patches/build-dependency-sdl.patch")
    apply(engine / "deps/vorbis/tree", ROOT / "patches/build-dependency-vorbis.patch")
    apply(engine, ROOT / "patches/portable-build.patch")
    original = WORK / "source-original"
    game = WORK / "game"
    original.mkdir(exist_ok=True)
    game.mkdir(exist_ok=True)
    game_reference = os.environ.get("HARDTIME_GAME_SOURCE_REFERENCE")
    upstream = LOCK["repository"].replace("https://github.com/", "https://raw.githubusercontent.com/")
    for name, expected in LOCK["files_sha256"].items():
        if game_reference:
            data = (Path(game_reference) / name).read_bytes()
        else:
            with urllib.request.urlopen(f"{upstream}/{LOCK['commit']}/{name}", timeout=60) as response:
                data = response.read()
        if hashlib.sha256(data).hexdigest() != expected:
            raise RuntimeError(f"Upstream source checksum mismatch: {name}")
        (original / name).write_bytes(data)
        (game / name).write_bytes(data)
    apply(game, ROOT / "patches/game-linux.patch")
    if PLATFORM == "windows-x64":
        apply(engine, ROOT / "patches/engine-windows-x64.patch")
        apply(game, ROOT / "patches/game-windows.patch")
        apply(engine, ROOT / "patches/engine-refresh-windows.patch")
        apply(game, ROOT / "patches/game-refresh-windows.patch")
    marker.write_text(json.dumps({"recipe_sha256": stamp, "engine_commit": LOCK["engine_commit"],
                                  "game_commit": LOCK["commit"], "platform": PLATFORM}, indent=2) + "\n")
    print(f"Prepared pinned engine and game source: {WORK}")


if __name__ == "__main__":
    main()
