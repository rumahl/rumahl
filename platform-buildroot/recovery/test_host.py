"""Offline tests only. Never invoke systemctl, docker, reboot or real host paths."""
import importlib.util
import io
import json
from pathlib import Path
import sqlite3
from contextlib import closing
import tarfile
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('recovery_host', Path(__file__).with_name('host.py'))
host = importlib.util.module_from_spec(spec)
spec.loader.exec_module(host)


class RecoveryTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.roots = {name: self.root / name for name in host.ROOTS}
        for root in self.roots.values():
            root.mkdir()
        self.store = self.root / 'store'
        self.store.mkdir()
        self.patches = [patch.object(host, 'ROOTS', self.roots), patch.object(host, 'STORE', self.store), patch.object(host.os, 'sync')]
        for item in self.patches:
            item.start()
        for name in ['accounts.sqlite', 'platform.sqlite']:
            with closing(sqlite3.connect(self.roots['state'] / name)) as conn:
                conn.executescript('CREATE TABLE session_credential (token TEXT); INSERT INTO session_credential VALUES ("old-secret"); CREATE TABLE account_session (id TEXT); INSERT INTO account_session VALUES ("session");')
        (self.roots['home'] / 'document.txt').write_text('original personal data')
        (self.roots['apps'] / 'app.txt').write_text('installed app')
        self.binding = {'device': 'device-a', 'binary': 'build-a', 'configuration': 'config-a'}

    def tearDown(self):
        for item in reversed(self.patches):
            item.stop()
        self.temp.cleanup()

    def test_complete_snapshot_restores_data_and_revokes_sessions(self):
        identifier = host.create_snapshot(self.binding)
        (self.roots['home'] / 'document.txt').write_text('changed')
        (self.roots['apps'] / 'new.txt').write_text('new app')
        host.restore_snapshot(identifier, self.binding)
        self.assertEqual((self.roots['home'] / 'document.txt').read_text(), 'original personal data')
        self.assertFalse((self.roots['apps'] / 'new.txt').exists())
        self.assertFalse((self.store / 'restore-pending.json').exists())
        with closing(sqlite3.connect(self.roots['state'] / 'accounts.sqlite')) as conn:
            self.assertEqual(conn.execute('SELECT COUNT(*) FROM session_credential').fetchone()[0], 0)
            self.assertEqual(conn.execute('SELECT COUNT(*) FROM account_session').fetchone()[0], 0)
        self.assertTrue(json.loads((self.roots['state'] / 'recovery-settings.json').read_text())['safeMode'])

    def test_corrupt_archive_rejected_before_replacement(self):
        identifier = host.create_snapshot(self.binding)
        archive = host.snapshot_path(identifier) / 'home.tar'
        archive.write_bytes(b'corrupt')
        with self.assertRaises(ValueError):
            host.restore_snapshot(identifier, self.binding)
        self.assertEqual((self.roots['home'] / 'document.txt').read_text(), 'original personal data')

    def test_wrong_device_and_missing_volume_are_rejected(self):
        identifier = host.create_snapshot(self.binding)
        with self.assertRaises(ValueError):
            host.verify(identifier, {**self.binding, 'device': 'other'})
        manifest_path = host.snapshot_path(identifier) / 'manifest.json'
        manifest = json.loads(manifest_path.read_text())
        del manifest['volumes']['apps']
        host.atomic_json(manifest_path, manifest)
        with self.assertRaises(ValueError):
            host.verify(identifier, self.binding)

    def test_archive_cannot_write_through_symlink(self):
        archive = self.root / 'evil.tar'
        with tarfile.open(archive, 'w') as tar:
            link = tarfile.TarInfo('escape'); link.type = tarfile.SYMTYPE; link.linkname = '/tmp'
            tar.addfile(link)
            item = tarfile.TarInfo('escape/overwrite'); item.size = 1
            tar.addfile(item, io.BytesIO(b'x'))
        with self.assertRaises(ValueError):
            host.check_archive(archive)

    def test_traversal_and_special_files_are_rejected(self):
        for name, kind in [('../escape', tarfile.REGTYPE), ('pipe', tarfile.FIFOTYPE), ('.rre-stage/file', tarfile.REGTYPE)]:
            archive = self.root / 'evil.tar'
            with tarfile.open(archive, 'w') as tar:
                item = tarfile.TarInfo(name); item.type = kind; tar.addfile(item)
            with self.assertRaises(ValueError):
                host.check_archive(archive)

    def test_interrupted_replacement_leaves_writer_guard(self):
        identifier = host.create_snapshot(self.binding)
        with patch.object(host, 'invalidate_sessions', side_effect=RuntimeError('failure')):
            with self.assertRaises(RuntimeError):
                host.restore_snapshot(identifier, self.binding)
        self.assertTrue((self.store / 'restore-pending.json').exists())
        self.assertEqual((self.roots['home'] / 'document.txt').read_text(), 'original personal data')

    def test_pending_restore_blocks_new_maintenance(self):
        host.atomic_json(self.store / 'restore-pending.json', {'backupId': 'interrupted'})
        with patch.object(host.threading, 'Timer') as timer:
            self.assertEqual(host.handle({'action': 'backup', 'userId': 'user'}, ['user'])['status'], 409)
            timer.assert_not_called()

    def test_boot_mode_preserves_repair_preference_and_sets_host_guard(self):
        host.atomic_json(self.roots['state'] / 'recovery-settings.json', {'safeMode': False, 'autoRepair': False})
        with patch.object(host.os, 'chown'):
            host.persist_mode(True)
            self.assertTrue((self.store / 'safe-mode').exists())
            host.persist_mode(False)
        self.assertFalse((self.store / 'safe-mode').exists())
        self.assertFalse(json.loads((self.roots['state'] / 'recovery-settings.json').read_text())['autoRepair'])

    def test_missing_permission_and_unknown_actions_fail_closed(self):
        with patch.object(host, 'job') as job:
            self.assertEqual(host.handle({'action': 'reboot', 'userId': 'user', 'safeMode': False}, [])['status'], 403)
            self.assertEqual(host.handle({'action': 'shell', 'userId': 'user'}, ['user'])['status'], 400)
            self.assertEqual(host.handle({'action': 'reboot', 'userId': 'user', 'safeMode': 'yes'}, ['user'])['status'], 400)
            self.assertEqual(host.handle({'action': 'backup', 'userId': 'user', 'path': '/'}, ['user'])['status'], 400)
            job.assert_not_called()


if __name__ == '__main__':
    unittest.main()
