"""Installation must never replace existing saves or the supplied original game."""
import importlib.util
from pathlib import Path
import struct
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
spec = importlib.util.spec_from_file_location('install_windows', ROOT / 'scripts/install-windows.py')
installer = importlib.util.module_from_spec(spec)
spec.loader.exec_module(installer)


@unittest.skipUnless(sys.platform == 'win32', 'Windows installation lock')
class InstallTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix='hardtime-install-test-')
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.assets = self.root / 'original'
        for name in (*installer.ASSETS, 'Data'):
            (self.assets / name).mkdir(parents=True)
            (self.assets / name / 'example.dat').write_bytes(b'original')
        self.binary = self.root / 'sample.exe'
        image = bytearray(256)
        image[:2] = b'MZ'
        struct.pack_into('<I', image, 0x3c, 128)
        image[128:132] = b'PE\0\0'
        struct.pack_into('<H', image, 132, 0x8664)
        struct.pack_into('<H', image, 152, 0x20b)
        self.binary.write_bytes(image)
        self.destination = self.root / 'modern'

    def install(self, destination=None):
        return installer.install(self.binary, self.assets, destination or self.destination, [])

    def test_reinstall_preserves_saves_photos_and_original(self):
        self.install()
        save = self.destination / 'Data/example.dat'
        photo = self.destination / 'Photo Album/new.bmp'
        save.write_bytes(b'player progress')
        photo.write_bytes(b'player photograph')
        self.install()
        self.assertEqual(save.read_bytes(), b'player progress')
        self.assertEqual(photo.read_bytes(), b'player photograph')
        self.assertEqual((self.assets / 'Data/example.dat').read_bytes(), b'original')

    def test_rejects_original_and_overlapping_destinations(self):
        for destination in (self.assets, self.assets / 'nested', self.root):
            with self.subTest(destination=destination), self.assertRaises(ValueError):
                self.install(destination)

    def test_rejects_unrelated_existing_directory(self):
        self.destination.mkdir()
        sentinel = self.destination / 'my-file.txt'
        sentinel.write_text('keep')
        with self.assertRaises(ValueError):
            self.install()
        self.assertEqual(sentinel.read_text(), 'keep')

    def test_rejects_32_bit_binary_before_creating_destination(self):
        image = bytearray(self.binary.read_bytes())
        struct.pack_into('<H', image, 132, 0x14c)
        self.binary.write_bytes(image)
        with self.assertRaises(ValueError):
            self.install()
        self.assertFalse(self.destination.exists())

    def test_update_is_excluded_while_install_lock_is_held(self):
        self.install()
        with installer.install_lock(self.destination):
            with self.assertRaises((ValueError, OSError)):
                self.install()

    def test_renderer_change_cannot_leave_mismatched_dlls(self):
        self.install()
        with self.assertRaises(ValueError):
            installer.install(self.binary, self.assets, self.destination, [], software=True)
        self.assertFalse((self.destination / 'software-renderer.txt').exists())

    def test_update_recovers_interrupted_save_before_copying_original_data(self):
        self.install()
        data = self.destination / 'Data'
        (data / 'example.dat').write_bytes(b'last committed save')
        (self.destination / 'Data.transaction').write_text(
            'Hard Time save transaction v2\nData\n', encoding='utf-8')
        data.rename(self.destination / 'Data.previous')
        relocated = self.root / 'relocated'
        self.destination.rename(relocated)
        self.destination = relocated
        data = self.destination / 'Data'
        self.install()
        self.assertEqual((data / 'example.dat').read_bytes(), b'last committed save')

    def test_update_refuses_unrecognized_recovery_marker(self):
        self.install()
        data = self.destination / 'Data'
        data.rename(self.destination / 'Data.previous')
        (self.destination / 'Data.transaction').write_text('unrecognized transaction\n')
        with self.assertRaises(ValueError):
            self.install()
        self.assertFalse(data.exists())
        self.assertEqual((self.destination / 'Data.previous/example.dat').read_bytes(), b'original')


if __name__ == '__main__':
    unittest.main()
