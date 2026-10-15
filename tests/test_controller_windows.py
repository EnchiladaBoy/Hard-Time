"""Compile the production SDL controller wrapper against virtual SDL devices."""
from pathlib import Path
import os
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
import windows


def main():
    windows.developer_environment()
    work = windows.WORK
    includes = [work / 'engine/src/modules', work / 'build/deps/sdl/tree/include',
                work / 'build/deps/sdl/tree/include-config-minsizerel']
    with tempfile.TemporaryDirectory(prefix='hardtime-controller-test-') as temporary:
        output = Path(temporary)
        binary = output / 'controller-reconnect.exe'
        command = ['cl.exe', '/nologo', '/std:c++17', '/EHsc', '/MT', '/O2',
                   '/DWIN32', '/DBB64', '/DBB_MSVC', '/DBB_WINDOWS', '/DBB_DESKTOP', '/DNOMINMAX',
                   *['/I' + str(path) for path in includes],
                   str(ROOT / 'tests/sources/controller-reconnect.cpp'),
                   str(work / 'engine/src/modules/bb/input/device.cpp'),
                   str(work / 'release/bin/x86_64-windows-win32/lib/SDL2.lib'),
                   '/link', '/OUT:' + str(binary), 'user32.lib', 'gdi32.lib', 'winmm.lib',
                   'imm32.lib', 'ole32.lib', 'oleaut32.lib', 'version.lib', 'uuid.lib',
                   'advapi32.lib', 'setupapi.lib', 'shell32.lib']
        result = subprocess.run(command, cwd=output, capture_output=True, text=True)
        if result.returncode:
            print(result.stdout + result.stderr)
            return result.returncode
        subprocess.run([str(binary)], cwd=output, check=True, timeout=20)
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
