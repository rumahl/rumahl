#!/usr/bin/env python3
"""Privileged RRE broker. No paths or commands are accepted from browser clients."""
import hashlib
import fcntl
import json
import os
from pathlib import Path, PurePosixPath
import pwd
import re
import shutil
import socket
import socketserver
import sqlite3
from contextlib import closing
import stat
import struct
import sys
import subprocess
import tarfile
import threading
import time
import uuid

ROOTS = {
    'state': Path('/var/lib/rumahl'), 'home': Path('/home'), 'apps': Path('/apps'),
    'docker': Path('/var/lib/docker'), 'images': Path('/var/lib/rumahl-runtime-images'),
}
STORE = Path('/var/lib/rumahl-recovery')
CONFIG = Path('/etc/rumahl/recovery.json')
UNITS = ['rumahl-platform.service', 'rumahl-shell.service', 'rumahl-runtime-supervisor.service', 'docker.service', 'docker.socket']
ID = re.compile(r'[0-9a-f]{32}')
LOCK = threading.Lock()


def command(args, timeout=300):
    return subprocess.run(args, check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=timeout).stdout


def atomic_json(path, value):
    temporary = path.with_suffix('.new')
    with temporary.open('w', encoding='utf-8') as stream:
        os.chmod(temporary, 0o600)
        json.dump(value, stream)
        stream.flush()
        os.fsync(stream.fileno())
    os.replace(temporary, path)
    descriptor = os.open(path.parent, os.O_RDONLY | os.O_DIRECTORY)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def write_lifecycle(intent):
    """Record why writers are being stopped so the platform can tell every
    client whether the whole system restarts or powers off."""
    path = ROOTS['state'] / 'lifecycle'
    temporary = path.with_suffix('.new')
    with temporary.open('w', encoding='utf-8') as stream:
        os.chmod(temporary, 0o644)
        stream.write(intent)
        stream.flush()
        os.fsync(stream.fileno())
    os.replace(temporary, path)


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def binding():
    # Same appliance and binary only: TPM keys and schema migrations are not portable.
    result = {name: digest(Path(path)) for name, path in {
        'device': '/etc/machine-id', 'binary': '/usr/bin/rumahl-platform-service',
        'configuration': '/etc/rumahl/platform.env',
    }.items()}
    # An external TPM sealed object must still be the same after restoration.
    for line in Path('/etc/rumahl/platform.env').read_text().splitlines():
        if line.startswith('RUMAHL_SECRET_KEY_OBJECT='):
            value = line.split('=', 1)[1].strip().strip('\"').strip("'")
            if value:
                result['sealedKey'] = digest(Path(value))
    return result


def check_submounts(root):
    prefix = str(root.resolve()) + '/'
    for line in Path('/proc/self/mountinfo').read_text().splitlines():
        fields = line.split()
        mount = re.sub(r'\\([0-7]{3})', lambda match: chr(int(match[1], 8)), fields[4])
        if mount.startswith(prefix):
            raise ValueError('nested mounts require separate backup support')


def snapshot_path(identifier):
    if not isinstance(identifier, str) or not ID.fullmatch(identifier):
        raise ValueError('invalid backup ID')
    result = STORE / 'backups' / identifier
    if result.is_symlink() or not result.is_dir():
        raise ValueError('backup unavailable')
    return result


def check_archive(path):
    """Reject traversal, special files and link-parent writes before any extraction."""
    with tarfile.open(path, 'r:') as archive:
        members = archive.getmembers()
        links = set()
        seen = set()
        for member in members:
            name = PurePosixPath(member.name)
            if name.is_absolute() or '..' in name.parts or any(part in ('.rre-stage', '.rre-previous') for part in name.parts) or str(name) in seen:
                raise ValueError('unsafe archive path')
            seen.add(str(name))
            whiteout = path.name == 'docker.tar' and member.ischr() and member.devmajor == 0 and member.devminor == 0
            if not (member.isfile() or member.isdir() or member.issym() or member.islnk() or whiteout):
                raise ValueError('unsupported archive entry')
            if member.issym() or member.islnk():
                target = PurePosixPath(member.linkname)
                # Absolute symlinks are legitimate in installed images, but no archive
                # member may write through a link. Hard links must stay in the archive.
                if member.islnk() and (target.is_absolute() or '..' in target.parts):
                    raise ValueError('unsafe hard link')
                links.add(str(name))
        for member in members:
            if any(str(parent) in links for parent in PurePosixPath(member.name).parents):
                raise ValueError('archive writes through link')


def verify(identifier, current_binding):
    directory = snapshot_path(identifier)
    manifest = json.loads((directory / 'manifest.json').read_text())
    if manifest.get('version') != 1 or manifest.get('binding') != current_binding:
        raise ValueError('backup belongs to a different device, configuration or build')
    if set(manifest.get('volumes', {})) != set(ROOTS):
        raise ValueError('incomplete backup')
    for name, expected in manifest['volumes'].items():
        archive = directory / f'{name}.tar'
        if archive.is_symlink() or not archive.is_file() or digest(archive) != expected:
            raise ValueError('backup checksum mismatch')
        check_archive(archive)
    return directory, manifest


def create_snapshot(current_binding):
    identifier = uuid.uuid4().hex
    parent = STORE / 'backups'
    parent.mkdir(mode=0o700, parents=True, exist_ok=True)
    directory = parent / (identifier + '.partial')
    directory.mkdir(mode=0o700)
    manifest = {'version': 1, 'id': identifier, 'at': int(time.time()), 'binding': current_binding, 'volumes': {}}
    for name, root in ROOTS.items():
        if root.is_symlink() or not root.is_dir():
            raise ValueError(f'required volume unavailable: {name}')
        check_submounts(root)
        archive = directory / f'{name}.tar'
        command(['/bin/tar', '--create', '--file', str(archive), '--xattrs', '--xattrs-include=*', '--acls', '--numeric-owner', '--sparse', '--one-file-system', '--directory', str(root), '.'], timeout=86400)
        check_archive(archive)
        manifest['volumes'][name] = digest(archive)
        with archive.open('rb') as stream:
            os.fsync(stream.fileno())
    atomic_json(directory / 'manifest.json', manifest)
    os.rename(directory, parent / identifier)
    os.sync()
    return identifier


def invalidate_sessions(state):
    # Token and account tables currently share accounts.sqlite. Other databases
    # are inspected too so moving the token store cannot silently resurrect tokens.
    tables = ['session_credential', 'account_session', 'oidc_access_token', 'oidc_refresh_spent', 'oidc_token_family', 'oidc_authorization_code', 'oidc_authorization_transaction']
    if not all((state / name).is_file() for name in ['accounts.sqlite', 'platform.sqlite']):
        raise ValueError('required state database missing')
    for path in state.glob('*.sqlite'):
        if not path.is_file() or path.is_symlink():
            raise ValueError('required state database missing')
        with closing(sqlite3.connect(path)) as connection:
            if connection.execute('PRAGMA integrity_check').fetchone()[0] != 'ok':
                raise ValueError('database integrity check failed')
            existing = {row[0] for row in connection.execute("SELECT name FROM sqlite_master WHERE type='table'")}
            for table in tables:
                if table in existing:
                    connection.execute(f'DELETE FROM {table}')
            connection.commit()
            connection.execute('PRAGMA wal_checkpoint(TRUNCATE)')


def restore_snapshot(identifier, current_binding):
    directory, _ = verify(identifier, current_binding)
    # Stage on each destination filesystem. Original volume contents remain intact
    # until every archive has extracted and state integrity has been checked.
    atomic_json(STORE / 'restore-pending.json', {'backupId': identifier, 'at': int(time.time())})
    stages = {}
    for name, root in ROOTS.items():
        if not root.is_dir() or root.is_symlink():
            raise ValueError('destination volume unavailable')
        stage = root / '.rre-stage'
        if stage.exists() or stage.is_symlink():
            raise ValueError('unfinished staging directory')
        stage.mkdir(mode=0o700)
        command(['/bin/tar', '--extract', '--file', str(directory / f'{name}.tar'), '--xattrs', '--xattrs-include=*', '--acls', '--numeric-owner', '--same-owner', '--directory', str(stage)], timeout=86400)
        stages[name] = stage
    invalidate_sessions(stages['state'])
    settings = stages['state'] / 'recovery-settings.json'
    if settings.is_symlink():
        raise ValueError('invalid recovery settings')
    atomic_json(settings, {'safeMode': True, 'autoRepair': False})
    # Pending marker gates all writers across reboot. Never restart after a partial
    # replacement. The pre-restore snapshot and old data remain for operator recovery.
    atomic_json(STORE / 'restore-pending.json', {'backupId': identifier, 'at': int(time.time())})
    for name, root in ROOTS.items():
        previous = root / '.rre-previous'
        if previous.exists() or previous.is_symlink():
            raise ValueError('previous restore needs operator review')
        previous.mkdir(mode=0o700)
        for child in list(root.iterdir()):
            if child.name not in ('.rre-stage', '.rre-previous'):
                os.rename(child, previous / child.name)
        for child in list(stages[name].iterdir()):
            os.rename(child, root / child.name)
        stages[name].rmdir()
    os.sync()
    # A complete pre-restore snapshot is retained outside these volumes.
    for root in ROOTS.values():
        shutil.rmtree(root / '.rre-previous')
    atomic_json(STORE / 'last-restore.json', {'backupId': identifier, 'at': int(time.time())})
    atomic_json(STORE / 'safe-mode', {'enabled': True})
    (STORE / 'restore-pending.json').unlink()
    os.sync()


def persist_mode(safe):
    path = ROOTS['state'] / 'recovery-settings.json'
    if path.is_symlink():
        raise ValueError('invalid recovery settings')
    try:
        previous = json.loads(path.read_text())
        auto_repair = previous.get('autoRepair', False) is True
    except (OSError, ValueError, AttributeError):
        auto_repair = False
    atomic_json(path, {'safeMode': safe, 'autoRepair': auto_repair})
    # Atomic replacement runs as root; the platform must retain ownership.
    owner = ROOTS['state'].stat()
    os.chown(path, owner.st_uid, owner.st_gid)
    marker = STORE / 'safe-mode'
    if safe:
        atomic_json(marker, {'enabled': True})
    else:
        marker.unlink(missing_ok=True)
    os.sync()


def stop_writers():
    # Stop activation first, then all containers, then the engine and its socket.
    loaded = [unit for unit in UNITS if subprocess.run(['/usr/bin/systemctl', 'show', '--property=LoadState', '--value', unit], check=False, stdout=subprocess.PIPE, stderr=subprocess.PIPE).stdout.strip() == b'loaded']
    front = [unit for unit in loaded if unit in UNITS[:3]]
    if front:
        command(['/usr/bin/systemctl', 'stop', *front])
    docker_active = subprocess.run(['/usr/bin/systemctl', 'is-active', '--quiet', 'docker.service'], check=False).returncode == 0
    running = command(['/usr/bin/docker', 'ps', '--quiet']).decode().split() if docker_active else []
    for container in running:
        if not re.fullmatch(r'[0-9a-f]+', container):
            raise ValueError('invalid container ID')
        command(['/usr/bin/docker', 'stop', '--time', '30', container])
    engine = [unit for unit in loaded if unit in UNITS[3:]]
    if engine:
        command(['/usr/bin/systemctl', 'stop', *engine])
    os.sync()


def mounted_devices():
    """Block devices currently mounted, mapped to their mount points."""
    result = {}
    for line in Path('/proc/self/mountinfo').read_text().splitlines():
        fields = line.split()
        if len(fields) < 5:
            continue
        device = fields[2]
        mount = re.sub(r'\\([0-7]{3})', lambda match: chr(int(match[1], 8)), fields[4])
        if device.startswith('/dev/'):
            result.setdefault(device, []).append(mount)
    return result


def fsck_report():
    """Read-only filesystem check. Mounted or busy filesystems are reported as
    needing a maintenance reboot rather than being checked live."""
    mounted = mounted_devices()
    results = []
    for device, mounts in sorted(mounted.items()):
        if mounts and any(mount not in ('none', '') for mount in mounts):
            results.append({'device': device, 'state': 'mounted', 'ok': None, 'detail': ', '.join(sorted(set(mounts)))})
            continue
        try:
            output = command(['/sbin/fsck', '-n', '-f', device], timeout=1800)
            results.append({'device': device, 'state': 'checked', 'ok': True, 'detail': output.decode(errors='replace')[-2000:]})
        except subprocess.CalledProcessError as error:
            detail = (error.stdout or b'').decode(errors='replace')[-2000:]
            results.append({'device': device, 'state': 'checked', 'ok': False, 'detail': detail})
        except Exception:
            results.append({'device': device, 'state': 'unavailable', 'ok': None, 'detail': None})
    return results


def newest_backup(current_binding):
    parent = STORE / 'backups'
    newest = None
    if parent.exists():
        for directory in parent.iterdir():
            if not ID.fullmatch(directory.name) or directory.is_symlink():
                continue
            try:
                manifest = json.loads((directory / 'manifest.json').read_text())
            except (OSError, ValueError, KeyError):
                continue
            if manifest.get('binding') != current_binding:
                continue
            if newest is None or manifest['at'] > newest[1]:
                newest = (directory.name, manifest['at'])
    return newest[0] if newest else None


def prune_backups(keep):
    parent = STORE / 'backups'
    if not parent.exists():
        return
    entries = []
    for directory in parent.iterdir():
        if not ID.fullmatch(directory.name) or directory.is_symlink():
            continue
        try:
            entries.append((json.loads((directory / 'manifest.json').read_text())['at'], directory))
        except (OSError, ValueError, KeyError):
            continue
    entries.sort(key=lambda item: item[0], reverse=True)
    for _, directory in entries[max(0, keep):]:
        shutil.rmtree(directory, ignore_errors=True)


def reset_volumes():
    """Erase user content from every managed volume, keeping the recovery store
    (outside these roots) and the volume directories themselves."""
    for root in ROOTS.values():
        if root.is_symlink() or not root.is_dir():
            raise ValueError('volume unavailable')
        for child in list(root.iterdir()):
            if child.name in ('.rre-stage', '.rre-previous'):
                continue
            if child.is_dir() and not child.is_symlink():
                shutil.rmtree(child)
            else:
                child.unlink()


def load_schedule():
    try:
        return json.loads((STORE / 'schedule.json').read_text())
    except (OSError, ValueError):
        return {'enabled': False, 'intervalHours': 24, 'keep': 3, 'lastRun': 0}


def save_schedule(value):
    atomic_json(STORE / 'schedule.json', value)


def scheduler_loop():
    """Create automatic backups when due. Runs as a daemon inside the broker."""
    while True:
        time.sleep(60)
        config = load_schedule()
        if not config.get('enabled'):
            continue
        if time.time() < config.get('lastRun', 0) + config['intervalHours'] * 3600:
            continue
        if (STORE / 'restore-pending.json').exists() or LOCK.locked():
            continue
        if not LOCK.acquire(blocking=False):
            continue
        # job() releases the lock in its finally block.
        job({'action': 'backup'})
        config['lastRun'] = int(time.time())
        save_schedule(config)


def job(request):
    status = {'action': request['action'], 'state': 'running', 'at': int(time.time())}
    atomic_json(STORE / 'job.json', status)
    stopped = False
    active = []
    try:
        action = request['action']
        if action == 'fsck':
            # Read-only: no writers are stopped.
            status['report'] = fsck_report()
            status['state'] = 'complete'
            atomic_json(STORE / 'job.json', status)
            return
        if action == 'restore':
            verify(request['backupId'], binding())
        active = [unit for unit in UNITS if subprocess.run(['/usr/bin/systemctl', 'is-active', '--quiet', unit], check=False).returncode == 0]
        stopped = True
        # Tell the platform (and through it every client) what is happening.
        if action == 'poweroff':
            write_lifecycle('shutting-down')
        elif action in ('reboot', 'restore', 'reset', 'reinstall', 'rollback'):
            write_lifecycle('restarting')
        else:
            write_lifecycle('services')
        stop_writers()
        if action == 'backup':
            status['backupId'] = create_snapshot(binding())
            prune_backups(load_schedule().get('keep', 3))
        elif action == 'restore':
            status['safetyBackupId'] = create_snapshot(binding())
            atomic_json(STORE / 'job.json', status)
            restore_snapshot(request['backupId'], binding())
            persist_mode(True)
        elif action == 'reboot':
            persist_mode(request['safeMode'])
        elif action in ('reset', 'reinstall'):
            reset_volumes()
            persist_mode(True)
        elif action == 'rollback':
            identifier = newest_backup(binding())
            if identifier is None:
                raise ValueError('no backup available')
            status['safetyBackupId'] = create_snapshot(binding())
            status['backupId'] = identifier
            atomic_json(STORE / 'job.json', status)
            restore_snapshot(identifier, binding())
            persist_mode(True)
        status['state'] = 'complete'
        atomic_json(STORE / 'job.json', status)
        if action in ('reboot', 'restore', 'reset', 'reinstall', 'rollback'):
            command(['/usr/bin/systemctl', 'reboot', '--no-block'])
            stopped = False
        elif action == 'poweroff':
            command(['/usr/bin/systemctl', 'poweroff', '--no-block'])
            stopped = False
    except Exception as error:
        # Expose a category, not paths, secrets or command output to the browser.
        status['state'] = 'failed'
        status['error'] = type(error).__name__
        atomic_json(STORE / 'job.json', status)
    finally:
        if stopped and not (STORE / 'restore-pending.json').exists():
            try:
                if active:
                    command(['/usr/bin/systemctl', 'start', *reversed(active)])
            except Exception:
                status['state'] = 'failed'
                status['error'] = 'ServiceStartFailed'
                atomic_json(STORE / 'job.json', status)
        LOCK.release()


def handle(request, operators):
    if request.get('userId') not in operators:
        return {'status': 403, 'error': 'operator permission required'}
    action = request.get('action')
    allowed = {
        'status': {'userId', 'action'}, 'backup': {'userId', 'action'},
        'reboot': {'userId', 'action', 'safeMode'}, 'restore': {'userId', 'action', 'backupId'},
        'fsck': {'userId', 'action'}, 'reset': {'userId', 'action'},
        'reinstall': {'userId', 'action'}, 'rollback': {'userId', 'action'},
        'poweroff': {'userId', 'action'},
        'schedule': {'userId', 'action', 'enabled', 'intervalHours', 'keep'},
    }
    if action not in allowed or set(request) != allowed[action]:
        return {'status': 400, 'error': 'invalid action'}
    if action == 'status':
        backups = []
        current_binding = binding()
        parent = STORE / 'backups'
        if parent.exists():
            for directory in sorted(parent.iterdir(), reverse=True):
                if not ID.fullmatch(directory.name) or directory.is_symlink():
                    continue
                try:
                    manifest = json.loads((directory / 'manifest.json').read_text())
                    backups.append({'id': directory.name, 'at': manifest['at'], 'compatible': manifest['binding'] == current_binding})
                except (OSError, ValueError, KeyError):
                    continue
                if len(backups) >= 100:
                    break
        status = json.loads((STORE / 'job.json').read_text()) if (STORE / 'job.json').exists() else None
        pending = (STORE / 'restore-pending.json').exists()
        return {'status': 200, 'backups': backups, 'job': status, 'busy': LOCK.locked(), 'restorePending': pending, 'safeMode': (STORE / 'safe-mode').exists(), 'schedule': load_schedule(), 'backupAvailable': all(root.is_dir() and not root.is_symlink() for root in ROOTS.values())}
    if action == 'reboot' and type(request['safeMode']) is not bool:
        return {'status': 400}
    if action == 'restore' and (not isinstance(request['backupId'], str) or not ID.fullmatch(request['backupId'])):
        return {'status': 400}
    if action == 'schedule':
        enabled, interval, keep = request['enabled'], request['intervalHours'], request['keep']
        if type(enabled) is not bool or type(interval) is not int or type(keep) is not int or not 1 <= interval <= 720 or not 1 <= keep <= 30:
            return {'status': 400}
        value = {'enabled': enabled, 'intervalHours': interval, 'keep': keep, 'lastRun': load_schedule().get('lastRun', 0)}
        save_schedule(value)
        return {'status': 200, 'schedule': value}
    if action == 'fsck':
        # Read-only, does not stop writers, so it can run inline.
        return {'status': 200, 'report': fsck_report()}
    if (STORE / 'restore-pending.json').exists() or not LOCK.acquire(blocking=False):
        return {'status': 409, 'error': 'maintenance in progress or interrupted restore'}
    # Acknowledge before shutting down the gateway. All requests are serialized.
    timer = threading.Timer(2, job, args=(request,))
    timer.start()
    return {'status': 202, 'accepted': True}


class Handler(socketserver.StreamRequestHandler):
    def handle(self):
        self.connection.settimeout(5)
        _, uid, _ = struct.unpack('3i', self.connection.getsockopt(socket.SOL_SOCKET, socket.SO_PEERCRED, 12))
        if uid != self.server.platform_uid:
            self.wfile.write(b'{"status":403}\n')
            return
        try:
            raw = self.rfile.readline(4097)
            if len(raw) > 4096 or not raw.endswith(b'\n'):
                raise ValueError('invalid frame')
            request = json.loads(raw)
            if not isinstance(request, dict):
                raise ValueError('invalid request')
            response = handle(request, self.server.operators)
        except Exception:
            response = {'status': 400}
        self.wfile.write(json.dumps(response).encode() + b'\n')


def main():
    os.umask(0o077)
    info = CONFIG.lstat()
    if info.st_uid != 0 or info.st_mode & 0o022 or not stat.S_ISREG(info.st_mode):
        raise ValueError('recovery configuration must be root-owned and not writable by others')
    config = json.loads(CONFIG.read_text())
    operators = config.get('operators', [])
    if not isinstance(operators, list) or not all(isinstance(user, str) and str(uuid.UUID(user)) == user for user in operators):
        raise ValueError('invalid operators')
    STORE.mkdir(mode=0o700, parents=True, exist_ok=True)
    process_lock = (STORE / 'broker.lock').open('a')
    fcntl.flock(process_lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
    if (STORE / 'job.json').exists():
        previous = json.loads((STORE / 'job.json').read_text())
        if previous.get('state') == 'running':
            previous.update(state='failed', error='Interrupted')
            atomic_json(STORE / 'job.json', previous)
    platform = pwd.getpwnam('rumahl-platform')
    socket_path = '/run/rumahl-recovery/control.sock'
    Path(socket_path).unlink(missing_ok=True)
    with socketserver.UnixStreamServer(socket_path, Handler) as server:
        os.chown(socket_path, platform.pw_uid, platform.pw_gid)
        os.chmod(socket_path, 0o600)
        server.platform_uid = platform.pw_uid
        server.operators = operators
        threading.Thread(target=scheduler_loop, daemon=True).start()
        server.serve_forever()


def offline_restore(identifier):
    if os.geteuid() != 0:
        raise PermissionError('offline restore requires root')
    process_lock = (STORE / 'broker.lock').open('a')
    fcntl.flock(process_lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
    current = binding()
    verify(identifier, current)
    stop_writers()
    atomic_json(STORE / 'restore-pending.json', {'backupId': identifier, 'offline': True})
    for root in ROOTS.values():
        for name in ['.rre-stage', '.rre-previous']:
            path = root / name
            if path.is_symlink():
                raise ValueError('unexpected staging link')
            if path.exists():
                shutil.rmtree(path)
    restore_snapshot(identifier, current)
    persist_mode(True)
    print('Restore complete. Services remain stopped; reboot into safe mode.')


if __name__ == '__main__':
    if len(sys.argv) == 3 and sys.argv[1] == '--restore-offline':
        offline_restore(sys.argv[2])
    elif len(sys.argv) == 1:
        main()
    else:
        raise SystemExit('usage: rumahl-recovery.py [--restore-offline BACKUP_ID]')
