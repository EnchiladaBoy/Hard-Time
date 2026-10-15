"""Run actual combat code with original assets in an isolated installation.

No test hooks are added to the distributed game. The harness is compiled from
the prepared source with startup routed into the fixture. Original saves are
read only; all combat, settings and serialization operate in the evidence copy.
"""
import argparse
import hashlib
import importlib.util
import os
from pathlib import Path
import shutil
import struct
import subprocess
import sys
import uuid

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
import windows


def hashes(path):
    return {p.relative_to(path).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in path.rglob('*') if p.is_file()}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--assets', type=Path, required=True)
    parser.add_argument('--game-source', type=Path, default=windows.WORK / 'game')
    parser.add_argument('--binary', type=Path, default=windows.WORK / 'game/Hard Time.exe')
    args = parser.parse_args()
    assets = args.assets.resolve()
    original_hashes = hashes(assets / 'Data')
    output = ROOT / '.work/validation' / ('features-' + uuid.uuid4().hex[:8])
    source = output / 'source'
    source.mkdir(parents=True)
    print('Evidence:', output, flush=True)
    spec = importlib.util.spec_from_file_location('installer', ROOT / 'scripts/install-windows.py')
    installer = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(installer)
    destination = output / 'game'
    installer.install(args.binary.resolve(), assets, destination, windows.runtime_files(True), True)
    for path in args.game_source.glob('*.bb'):
        shutil.copy2(path, source / path.name)
    shutil.copy2(ROOT / 'tests/fixtures/game-features.bb', source / 'GameFeatureChecks.bb')
    main_source = source / 'Gameplay.bb'
    text = main_source.read_text(encoding='cp1252')
    text = text.replace('Intro()\n', '; Skip the introductory screen in the test harness.\n', 1)
    start = text.index(';SCREEN MANAGEMENT')
    end = text.index(';Let every screen unwind', start)
    text = text[:start] + '''Include "GameFeatureChecks.bb"
slot=2 : oldLocation=0
GenerateGame()
gamLocation(slot)=9 : gamWarrant(slot)=0 : gamMission(slot)=0
Local selected=0
For char=1 To no_chars
 charLocation(char)=1 : charWeapon(char)=0
 If char=gamChar(slot) Or selected<2
  charLocation(char)=9 : charHealth(char)=100 : charHP(char)=10
  charInjured(char)=0 : charHappiness(char)=100 : charStrength(char)=100
  charX#(char)=100+char*10 : charY#(char)=20 : charZ#(char)=0
  For count=1 To 40 : charScar(char,count)=0 : Next
  If char<>gamChar(slot)
   selected=selected+1
   If selected=2 Then charHealth(char)=0
  EndIf
 EndIf
Next
camX#=0 : camY#=30 : camZ#=-70
screen=50 : Gameplay()
End

''' + text[end:]
    needle = 'go=0 : gotim=-50 : keytim=20\nWhile go=0'
    assert text.count(needle) == 1
    text = text.replace(needle, 'go=0 : gotim=-50 : keytim=20\nNativeFeatureChecks()\nWhile go=0')
    main_source.write_text(text, encoding='cp1252')
    settings = [2, 0, 100, 100, 100, 0, 100, 100, 100, 200, 208, 203, 205, 30, 44, 31, 45, 25]
    checksum = 1213484356
    for value in settings:
        checksum ^= value
    (destination / 'Data/NativeOptions.dat').write_bytes(struct.pack('<21i', 1213484356, 1, *settings, checksum))
    # No user cheats/audio preferences should influence the controlled fixture.
    for name in ('NativeExtras.dat', 'NativeExtras.dat.bak'):
        (destination / 'Data' / name).unlink(missing_ok=True)
    windows.developer_environment()
    env = os.environ | {'blitzpath': str(windows.WORK / 'release'), 'ALSOFT_DRIVERS': 'null', 'GALLIUM_DRIVER': 'llvmpipe'}
    with (output / 'build.log').open('wb') as log:
        subprocess.run([str(windows.WORK / 'release/bin/blitzcc64.exe'), '-r', 'opengl', '-o',
                        str(destination / 'FeatureChecks.exe'), str(main_source)],
                       cwd=source, env=env, stdout=log, stderr=subprocess.STDOUT, check=True, timeout=120)
    with (output / 'run.log').open('wb') as log:
        subprocess.run([str(destination / 'FeatureChecks.exe')], cwd=destination, env=env,
                       stdout=log, stderr=subprocess.STDOUT, check=True, timeout=120)
    result = (destination / 'game-features-result.txt').read_text().strip()
    assert result.startswith('PASS real punch hooks'), result
    assert hashes(assets / 'Data') == original_hashes, 'Original saves were modified'
    print(result)
    print('PASS original save hashes unchanged')


if __name__ == '__main__':
    main()
