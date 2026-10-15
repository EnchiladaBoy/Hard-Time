#!/usr/bin/env python3
"""Exercise the compiled setup, optionally using a locally supplied original installer."""
import argparse
import hashlib
import json
import io
import os
from pathlib import Path
import struct
import subprocess
import sys
import tempfile
import time
import zipfile

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
import rust_setup


def hashes(path):
    return {p.relative_to(path).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in sorted(path.rglob('*')) if p.is_file()}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--setup', type=Path, default=ROOT / 'dist/Hard-Time-Setup.exe')
    parser.add_argument('--original-installer', type=Path)
    parser.add_argument('--legacy-setup', type=Path, help='Optional previous C# setup to test an actual upgrade')
    args = parser.parse_args()
    evidence = ROOT / '.work/validation/setup'
    evidence.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='checks-', dir=evidence) as temporary:
        scratch = Path(temporary)
        subprocess.run(rust_setup.cargo('test'), cwd=ROOT, check=True)
        binary = args.setup.resolve().read_bytes()
        footer = binary[-64:]
        assert footer[:16] == rust_setup.MAGIC
        offset, length = struct.unpack('<QQ', footer[16:32])
        assert offset + length == len(binary) - 64
        bundled = binary[offset:offset + length]
        assert hashlib.sha256(bundled).digest() == footer[32:]
        with zipfile.ZipFile(io.BytesIO(bundled)) as archive:
            assert archive.testzip() is None
            manifest = json.loads(archive.read('manifest.json'))
            assert manifest['installer_language'] == 'Rust'
            assert set(archive.namelist()) == set(manifest['files']) | {'manifest.json'}
            for name, digest in manifest['files'].items():
                assert hashlib.sha256(archive.read(name)).hexdigest() == digest, name
            notices = json.loads(archive.read('runtime/licenses/Rust-dependencies.json'))
            assert len(notices) == manifest['rust_dependency_notices'] > 0
            assert all('runtime/licenses/' + item['notice'] in manifest['files'] for item in notices)
            launcher = archive.read('runtime/Play Hard Time.exe')
        print('PASS packaged Rust executable, payload checksums and dependency notices')
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
        help_result = subprocess.run([str(args.setup.resolve()), '--help'], env=environment,
                                     capture_output=True, text=True, timeout=10)
        assert help_result.returncode == 0 and 'Rust Setup' in help_result.stdout, help_result
        bad_argument = subprocess.run([str(args.setup.resolve()), '--source'], env=environment,
                                      capture_output=True, text=True, timeout=10)
        assert bad_argument.returncode == 1 and '--source needs' in bad_argument.stderr
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

        # The appended payload is checked before destination creation.
        broken = scratch / 'corrupt-setup.exe'
        damaged = bytearray(binary)
        damaged[offset + length // 2] ^= 1
        broken.write_bytes(damaged)
        bad_target = scratch / 'corrupt-install'
        corrupt = subprocess.run([str(broken), '--install', '--source', str(source), '--destination', str(bad_target),
                                  '--no-shortcuts', '--result', str(result)], env=environment,
                                 capture_output=True, timeout=30)
        assert corrupt.returncode == 1 and 'checksum failed' in json.loads(result.read_text())['error']
        assert not bad_target.exists()
        print('PASS damaged payload refused before installation')

        # A report path cannot overwrite source assets or installed saves, even
        # when validation fails. Atomic report replacement also protects links.
        for report_path in [original / 'Data/Options.dat', destination / 'Data.previous/player-save.dat']:
            before_report = report_path.read_bytes()
            rejected = subprocess.run([str(args.setup.resolve()), '--install', '--source', str(original),
                                       '--destination', str(destination), '--result', str(report_path)],
                                      env=environment, capture_output=True, timeout=15)
            assert rejected.returncode == 1
            assert report_path.read_bytes() == before_report
        print('PASS automation result paths cannot overwrite saves or original assets')

        # Windows junctions require no developer-mode symlink privileges.
        import _winapi
        junction = scratch / 'linked-original'
        before_junction = hashes(original)
        _winapi.CreateJunction(str(original), str(junction))
        try:
            assert 'link or junction' in run(success=False, target=scratch / 'junction-install', asset_source=junction)['error']
        finally:
            os.rmdir(junction)  # Remove only the reparse point; never recurse into its target.
        assert hashes(original) == before_junction
        print('PASS junction source refused')

        launch_root = scratch / 'Launcher game 猫'
        (launch_root / 'Data').mkdir(parents=True)
        (launch_root / 'Data/player.dat').write_bytes(b'player save with \x00\xff data')
        (launch_root / 'Play Hard Time.exe').write_bytes(launcher)
        (launch_root / 'software-renderer.txt').write_text('software')
        (launch_root / '.hardtime-native-install.json').write_text(json.dumps({
            'platform': 'windows-x64', 'software_renderer': True, 'executable_sha256': 'a' * 64}))
        subprocess.run([rust_setup.tool('rustc'), str(ROOT / 'tests/fixtures/launcher_child.rs'),
                        '--edition=2024', '-C', 'target-feature=+crt-static', '-C', 'opt-level=1',
                        '-o', str(launch_root / 'Hard Time.exe')], cwd=ROOT, check=True)
        before_launch = hashes(launch_root / 'Data')
        child_environment = environment | {'HARDTIME_TEST_STARTED': str(scratch / 'started'),
                                           'HARDTIME_TEST_RELEASE': str(scratch / 'release')}
        launched = subprocess.Popen([str(launch_root / 'Play Hard Time.exe')], env=child_environment)
        try:
            deadline = time.monotonic() + 15
            while not (scratch / 'started').exists() and time.monotonic() < deadline:
                assert launched.poll() is None, 'Launcher exited before starting its game'
                time.sleep(0.05)
            assert (scratch / 'started').is_file(), 'The game did not start'
            assert 'Close Hard Time' in run(success=False, target=launch_root, asset_source=original)['error']
        finally:
            (scratch / 'release').write_bytes(b'release')
            launched.wait(timeout=15)
        assert launched.returncode == 0
        backups = list((launch_root / 'backups').iterdir())
        assert len(backups) == 1 and hashes(backups[0]) == before_launch
        assert hashes(launch_root / 'Data') == before_launch
        logs = list((launch_root / 'logs').glob('*.log'))
        assert any('renderer=llvmpipe' in p.read_text(encoding='utf-8') for p in logs)
        assert any('launcher diagnostic fixture' in p.read_text(encoding='utf-8') for p in logs)
        assert 'Exit status:' in (launch_root / 'logs/latest.txt').read_text(encoding='utf-8')
        print('PASS Rust launcher backup, logs, graphics environment and lock lifetime with Windows-only PATH')

        if args.legacy_setup:
            legacy_root = scratch / 'legacy-upgrade'
            subprocess.run([str(args.legacy_setup.resolve()), '--install', '--source', str(original),
                            '--destination', str(legacy_root), '--renderer', 'software', '--no-shortcuts',
                            '--result', str(result)], env=environment, check=True, timeout=120)
            (legacy_root / 'Data/player.dat').write_bytes(b'legacy player save')
            (legacy_root / 'Photo Album/photo.bmp').write_bytes(b'legacy photo')
            previous = {name: hashes(legacy_root / name) for name in ('Data', 'Photo Album')}
            assert run(target=legacy_root, asset_source=original)['renderer'] == 'software'
            for name, expected in previous.items():
                assert hashes(legacy_root / name) == expected
            assert json.loads((legacy_root / '.hardtime-native-install.json').read_text())['installer'].startswith('rust-')
            print('PASS upgrade from an actual C# installation preserves player files')

        record = {'core_checks': 17, 'integration_groups': 12, 'legacy_upgrade_tested': bool(args.legacy_setup),
                  'original_installer_tested': bool(args.original_installer), 'installer_language': 'Rust',
                  'setup_sha256': hashlib.sha256(args.setup.read_bytes()).hexdigest(), 'windows_only_path': True}
        (evidence / 'latest.json').write_text(json.dumps(record, indent=2) + '\n')


if __name__ == '__main__':
    main()
