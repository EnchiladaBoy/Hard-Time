"""Check the portable Rust ZIP after scripts/package-rust.py. No game assets."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
import zipfile

ROOT = Path(__file__).resolve().parents[1]
PREFIX = 'Hard-Time-Rust-preview/'


class RustPackageTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.archive = zipfile.ZipFile(ROOT / 'dist/Hard-Time-Rust-preview.zip')
        cls.addClassCleanup(cls.archive.close)
        cls.manifest = json.loads(cls.archive.read(PREFIX + 'manifest.json'))

    def test_complete_manifest_and_no_original_assets(self):
        self.assertIsNone(self.archive.testzip())
        self.assertEqual(set(self.archive.namelist()),
                         {PREFIX + name for name in self.manifest['files']} | {PREFIX + 'manifest.json'})
        for name, expected in self.manifest['files'].items():
            with self.subTest(file=name):
                self.assertNotIn('..', Path(name).parts)
                self.assertFalse(Path(name).is_absolute())
                self.assertEqual(hashlib.sha256(self.archive.read(PREFIX + name)).hexdigest(), expected)
                self.assertNotIn(Path(name).suffix.lower(), {'.dat', '.3ds', '.jpg', '.png', '.wav', '.dll'})
        self.assertIn('Hard-Time-Rust.exe', self.manifest['files'])
        self.assertIn('NOTICE.txt', self.manifest['files'])
        self.assertIn('Read Me.txt', self.manifest['files'])

    def test_dependency_notices_are_present(self):
        index = json.loads(self.archive.read(PREFIX + 'licenses/index.json'))
        self.assertEqual(len(index), self.manifest['dependency_notice_count'])
        self.assertGreater(len(index), 0)
        for package in index:
            with self.subTest(package=package['name']):
                self.assertTrue(any(Path(file).name.lower().startswith(
                    ('license', 'licence', 'copying', 'copyright', 'notice', 'ofl'))
                    for file in package['files']))
                for file in package['files']:
                    self.assertIn('licenses/' + file, self.manifest['files'])
        self.assertIn('licenses/rust/COPYRIGHT-library.html', self.manifest['files'])

    @unittest.skipUnless(os.name == 'nt', 'Windows executable')
    def test_executable_starts_without_adjacent_runtime_dlls(self):
        # Only unpack the executable into a newly owned temporary directory.
        # Restrict PATH so a developer's installed libraries cannot mask a
        # missing runtime dependency. CLI checks exit before GPU initialization.
        with tempfile.TemporaryDirectory(prefix='hardtime-rust-package-') as temporary:
            executable = Path(temporary) / 'Hard-Time-Rust.exe'
            executable.write_bytes(self.archive.read(PREFIX + 'Hard-Time-Rust.exe'))
            env = os.environ.copy()
            env['PATH'] = str(Path(env['WINDIR']) / 'System32')
            for args, code, message in [(['--help'], 0, 'Rust migration preview'),
                                        (['--unknown'], 1, 'Unknown argument'),
                                        (['--assets'], 1, '--assets needs a folder')]:
                result = subprocess.run([str(executable), *args], cwd=temporary, env=env,
                                        text=True, capture_output=True, timeout=20)
                self.assertEqual(result.returncode, code, result.stderr)
                self.assertIn(message, result.stdout + result.stderr)


if __name__ == '__main__':
    unittest.main(verbosity=2)
