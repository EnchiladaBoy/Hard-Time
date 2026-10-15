#!/usr/bin/env python3
"""Exercise the compiled setup, optionally using a locally supplied original installer."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
spec = importlib.util.spec_from_file_location('package_windows', ROOT / 'scripts/package-windows.py')
package = importlib.util.module_from_spec(spec)
spec.loader.exec_module(package)


def hashes(path):
    return {p.relative_to(path).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in sorted(path.rglob('*')) if p.is_file()}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--setup', type=Path, default=ROOT / 'dist/Hard-Time-Setup.exe')
    parser.add_argument('--original-installer', type=Path)
    args = parser.parse_args()
    evidence = ROOT / '.work/validation/setup'
    evidence.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='checks-', dir=evidence) as temporary:
        scratch = Path(temporary)
        check = scratch / 'SetupChecks.exe'
        package.compile_cs(check, [ROOT / 'setup/Common.cs', ROOT / 'setup/SetupCore.cs', ROOT / 'tests/SetupChecks.cs'], console=True)
        subprocess.run([str(check), str(scratch / 'core')], check=True)
        original = scratch / 'original'
        for name in ('Characters', 'Graphics', 'Sound', 'Weapons', 'World', 'Data'):
            (original / name).mkdir(parents=True)
            (original / name / 'fixture.dat').write_bytes(b'asset fixture')
        for name in ('Graphics/Logo01.PNG', 'Data/Options.dat', 'Data/Structure.dat'):
            (original / name).write_bytes(b'fixture')
        source = args.original_installer.resolve() if args.original_installer else original
        source_hash = hashlib.sha256(source.read_bytes()).hexdigest() if source.is_file() else hashes(source)
        destination = scratch / 'Native game 猫'
        result = scratch / 'result.json'
        # The end-user process gets only Windows on PATH, with no build tools.
        environment = os.environ.copy()
        environment['PATH'] = os.pathsep.join([os.environ['WINDIR'], str(Path(os.environ['WINDIR']) / 'System32')])
        def run(renderer='software', success=True, target=destination, asset_source=source):
            process = subprocess.run([str(args.setup.resolve()), '--install', '--source', str(asset_source),
                                      '--destination', str(target), '--renderer', renderer, '--no-shortcuts', '--result', str(result)],
                                     env=environment, timeout=120)
            data = json.loads(result.read_text())
            assert (process.returncode == 0) == success, data
            return data
        run()
        print('PASS standalone extraction/install with Windows-only PATH and a Unicode destination')
        (destination / 'Data/player-save.dat').write_bytes(b'player progress')
        (destination / 'Photo Album/screenshot.bmp').write_bytes(b'photo')
        (destination / 'backups/keep').mkdir(parents=True)
        (destination / 'backups/keep/save.dat').write_bytes(b'backup')
        preserved = {name: hashes(destination / name) for name in ('Data', 'Photo Album', 'backups')}
        run('gpu')
        assert not (destination / 'opengl32.dll').exists()
        assert not (destination / 'libgallium_wgl.dll').exists()
        assert not (destination / 'software-renderer.txt').exists()
        assert run('auto')['renderer'] == 'gpu'
        run('software')
        assert (destination / 'opengl32.dll').is_file()
        assert (destination / 'libgallium_wgl.dll').is_file()
        for name, expected in preserved.items():
            assert hashes(destination / name) == expected, name
        print('PASS renderer switching and automatic update preserve saves, photos and backups')
        before = hashes(destination)
        with (destination / '.launch.lock').open('r+b'):
            assert 'Close Hard Time' in run(success=False)['error']
        assert hashes(destination) == before
        print('PASS active installation refused without modifying any installed file')
        (destination / 'Data').rename(destination / 'Data.previous')
        (destination / 'Data.transaction').write_text('Hard Time save transaction v2\nData\n')
        run()
        assert hashes(destination / 'Data') == preserved['Data']
        print('PASS installer recovers saves before considering fresh seed data')
        (destination / 'Data.transaction').write_text('unknown transaction\n')
        (destination / 'Data').rename(destination / 'Data.previous')
        before = hashes(destination)
        assert 'unrecognized' in run(success=False)['error']
        assert hashes(destination) == before
        print('PASS invalid save recovery marker preserves every installed file')
        bogus = scratch / 'not-the-original.exe'
        bogus.write_bytes(b'not an installer')
        assert 'differs' in run(success=False, target=scratch / 'bad-install', asset_source=bogus)['error']
        assert not (scratch / 'bad-install').exists()
        print('PASS unrecognized installer rejected before destination creation')
        after_source = hashlib.sha256(source.read_bytes()).hexdigest() if source.is_file() else hashes(source)
        assert source_hash == after_source
        manifest = json.loads(args.setup.with_suffix('.manifest.json').read_text())
        assert not any(name.split('/')[0] in ('Data', 'Characters', 'Graphics', 'Sound', 'World', 'Weapons') for name in manifest['files'])
        print('PASS source untouched and distributable contains no original game assets')
        record = {'core_checks': 16, 'integration_groups': 7, 'original_installer_tested': bool(args.original_installer),
                  'setup_sha256': hashlib.sha256(args.setup.read_bytes()).hexdigest(), 'windows_only_path': True}
        (evidence / 'latest.json').write_text(json.dumps(record, indent=2) + '\n')


if __name__ == '__main__':
    main()
