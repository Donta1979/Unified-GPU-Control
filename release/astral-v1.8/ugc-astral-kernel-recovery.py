#!/usr/bin/env python3
"""UGC v1.8 Astral kernel mismatch recovery.

Only deploy through an independently authenticated release into a root-owned,
non-writable-by-users location. NEVER run this script from Downloads as root.
Does not unload an active module or test the emergency poweroff interlock.
"""
import fcntl
import hashlib
import json
import os
from pathlib import Path
import shutil
import stat
import subprocess
import sys
import tempfile

STAGE = Path('/var/lib/ugc/astral-release')
SOURCE = STAGE / 'kernel-source'
MODULE = Path('/var/usrlocal/lib/astral-hwmon/astral-hwmon.ko')
LOCK = Path('/run/lock/ugc-astral-recovery.lock')
REQUIRED = ('Kbuild', 'astral_hwmon.c', 'astral_detect.c', 'astral_regs.c', 'astral.h')
# Publisher-pinned hashes of the reviewed upstream source, NOT hashes read from
# the same mutable manifest. A future source update requires a reviewed release.
SOURCE_HASHES = {
    'Kbuild': 'fcfa195da7716e0eba4a356f1b62236f024ec139f2a7ef7c2ab29050085610cf',
    'astral.h': 'c43496577299b8f953a495fbb78a97bb2bf50c84814ffd2611b9a42914eb3648',
    'astral_detect.c': 'd9362f907c1c46444c3ba5f6d15c5cf9aac1d72d46cdd5684c34e73e93f41cfe',
    'astral_hwmon.c': 'a67331b88963717ce2f49568ed81c4978573061323018f51e9a25c9c8f0a5dfc',
    'astral_regs.c': 'ee816538c36a40cf5079e4551f2cba000f70caf76208ba23ad8a0560ef7a3d17',
}


def trusted(path, directory=False):
    path = Path(os.path.abspath(path))
    for component in (path, *path.parents):
        info = component.lstat()
        if info.st_uid != 0 or info.st_mode & 0o022 or stat.S_ISLNK(info.st_mode):
            raise RuntimeError(f'Unsafe ownership, mode or symlink: {component}')
        if component != path and not stat.S_ISDIR(info.st_mode):
            raise RuntimeError(f'Unsafe ancestor: {component}')
    info = path.lstat()
    if directory and not stat.S_ISDIR(info.st_mode):
        raise RuntimeError(f'Not a directory: {path}')
    if not directory and (not stat.S_ISREG(info.st_mode) or info.st_nlink != 1):
        raise RuntimeError(f'Not a safe file: {path}')
    return path


def sha(path):
    h = hashlib.sha256()
    with open(path, 'rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            h.update(block)
    return h.hexdigest()


def command(*args, timeout=30):
    p = subprocess.run(args, capture_output=True, text=True, timeout=timeout)
    if p.returncode:
        raise RuntimeError(f'{args[0]} failed ({p.returncode}): {(p.stderr or p.stdout)[-1200:]}')
    return p.stdout.strip()


def active(unit):
    p = subprocess.run(('/usr/bin/systemctl', 'is-active', unit), capture_output=True, text=True, timeout=15)
    return p.returncode == 0 and p.stdout.strip() == 'active'


def loaded():
    return Path('/sys/module/astral_hwmon').is_dir()


def vermagic(path):
    output = command('/usr/sbin/modinfo', '-F', 'vermagic', str(path))
    if not output.split():
        raise RuntimeError(f'Empty vermagic: {path}')
    return output.split()[0]


def verify_source():
    trusted(STAGE, True)
    trusted(SOURCE, True)
    manifest = trusted(SOURCE / 'SOURCE-SHA256.json')
    data = json.loads(manifest.read_text())
    if data.get('format') != 'ugc-astral-kernel-source-v1' or data.get('files') != SOURCE_HASHES:
        raise RuntimeError('Source manifest does not match pinned reviewed source')
    if {p.name for p in SOURCE.iterdir()} != set(REQUIRED) | {'SOURCE-SHA256.json'}:
        raise RuntimeError('Unexpected kernel source inventory')
    for name in REQUIRED:
        if sha(trusted(SOURCE / name)) != SOURCE_HASHES[name]:
            raise RuntimeError(f'Pinned kernel source mismatch: {name}')


def inspect():
    kernel = os.uname().release
    installed = vermagic(MODULE) if MODULE.is_file() else None
    return dict(kernel=kernel, installed_kernel=installed, module_loaded=loaded(),
                guard_timer_active=active('astral-guard.timer'),
                kernel_mismatch=installed is not None and installed != kernel)


def verify_recovery(kernel):
    if os.uname().release != kernel or vermagic(MODULE) != kernel or not loaded():
        raise RuntimeError('Kernel/module compatibility or loaded module check failed')
    matches = []
    for hw in Path('/sys/class/hwmon').glob('hwmon*'):
        try:
            if (hw / 'name').read_text().strip() == 'astral12vhpwr':
                matches.append(hw)
        except OSError:
            pass
    if len(matches) != 1:
        raise RuntimeError('Expected exactly one Astral sensor')
    hw = matches[0]
    for index in range(6):
        for name in (f'in{index}_input', f'curr{index+1}_input'):
            value = int((hw / name).read_text().strip())
            if value < 0:
                raise RuntimeError(f'Invalid sensor reading: {name}')
    if not active('astral-hwmon.service') or not active('astral-guard.timer'):
        raise RuntimeError('Astral driver service or emergency guard timer inactive')
    return True


def recover():
    if os.geteuid() != 0:
        raise RuntimeError('Administrator authorization required')
    # The entrypoint must already be securely installed. This is not a
    # substitute for publisher-authenticated deployment of the entrypoint.
    trusted(__file__)
    trusted(MODULE.parent, True)
    trusted(MODULE)
    trusted(STAGE, True)
    # Serialize UGC recovery attempts. A separate third-party driver manager
    # may still race; never unload an already loaded module.
    with os.fdopen(os.open(LOCK, os.O_CREAT | os.O_RDWR | os.O_NOFOLLOW, 0o600), 'r+') as lock:
        info = os.fstat(lock.fileno())
        if info.st_uid != 0 or info.st_mode & 0o077 or not stat.S_ISREG(info.st_mode):
            raise RuntimeError('Unsafe recovery lock')
        fcntl.flock(lock.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
        state = inspect()
        if loaded():
            if sys.argv[1:] == ['prepare'] and not state['kernel_mismatch']:
                return {'status': 'already-loaded', 'kernel': state['kernel']}
            raise RuntimeError('Astral module already loaded; refusing replacement')
        if not state['kernel_mismatch']:
            if sys.argv[1:] == ['prepare']:
                return {'status': 'compatible', 'kernel': state['kernel']}
            raise RuntimeError('No installed kernel mismatch; no replacement needed')
        verify_source()
        kernel = state['kernel']
        build = Path('/lib/modules') / kernel / 'build'
        if not build.is_dir():
            raise RuntimeError('Matching kernel build directory unavailable')
        # Preserve every previous kernel backup. Repeated Bazzite updates
        # and rollbacks must never overwrite or discard an existing backup.
        backup_base = MODULE.parent / ('astral-hwmon.ko.backup-' + state['installed_kernel'])
        backup = backup_base
        suffix = 0
        while backup.exists() or backup.is_symlink():
            suffix += 1
            if suffix > 10000:
                raise RuntimeError('Too many persistent kernel backups')
            backup = Path(str(backup_base) + '.' + str(suffix))
        # Use a root-owned build directory; never compile user-controlled files.
        with tempfile.TemporaryDirectory(prefix='.ugc-astral-build-', dir=STAGE) as workname:
            work = Path(workname)
            for name in REQUIRED:
                shutil.copyfile(SOURCE / name, work / name)
            command('/usr/bin/make', '-C', str(build), f'M={work}', 'modules', timeout=180)
            candidate = work / 'astral-hwmon.ko'
            if not candidate.is_file() or candidate.is_symlink() or vermagic(candidate) != kernel:
                raise RuntimeError('Compiled driver missing or incompatible')
            with tempfile.TemporaryDirectory(prefix='.ugc-astral-replace-', dir=MODULE.parent) as tempname:
                staged = Path(tempname) / 'astral-hwmon.ko'
                shutil.copyfile(candidate, staged)
                staged.chmod(0o644)
                command('/usr/bin/chcon', '-t', 'modules_object_t', str(staged))
                if vermagic(staged) != kernel or loaded():
                    raise RuntimeError('Candidate changed or driver became active')
                # Exclusive, persistent backup with flush before replacement.
                fd = os.open(backup, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
                with os.fdopen(fd, 'wb') as out, MODULE.open('rb') as src:
                    shutil.copyfileobj(src, out)
                    out.flush()
                    os.fsync(out.fileno())
                if sha(backup) != sha(MODULE) or vermagic(MODULE) != state['installed_kernel']:
                    raise RuntimeError('Original module changed during backup')
                if loaded():
                    raise RuntimeError('Driver became active; refusing replacement')
                os.replace(staged, MODULE)
                try:
                    # Make replacement durable before attempting activation.
                    dirfd = os.open(MODULE.parent, os.O_RDONLY | os.O_DIRECTORY)
                    try:
                        os.fsync(dirfd)
                    finally:
                        os.close(dirfd)
                    if vermagic(MODULE) != kernel:
                        raise RuntimeError('Installed module vermagic mismatch')
                    # Driver first, guard timer second; do not invoke the guard
                    # executable as a simulated emergency test.
                    if sys.argv[1:] == ['prepare']:
                        if vermagic(MODULE) != kernel:
                            raise RuntimeError('Prepared module kernel mismatch')
                        return {'status': 'prepared', 'kernel': kernel,
                                'backup': str(backup), 'module_sha256': sha(MODULE)}
                    command('/usr/bin/systemctl', 'enable', '--now', 'astral-hwmon.service', timeout=45)
                    if not loaded():
                        raise RuntimeError('Astral driver failed to load')
                    command('/usr/bin/systemctl', 'enable', '--now', 'astral-guard.timer', timeout=45)
                    verify_recovery(kernel)
                except Exception as exc:
                    # NEVER unload active protection. Preserve new driver and
                    # backup for diagnosis if the module became loaded.
                    if not loaded():
                        os.replace(backup, MODULE)
                        raise RuntimeError(f'Recovery activation failed; original module restored: {exc}') from exc
                    raise RuntimeError(f'Recovery requires attention; loaded driver preserved and backup retained: {exc}') from exc
                return {'status': 'recovered', 'kernel': kernel,
                        'backup': str(backup), 'module_sha256': sha(MODULE),
                        'sensor_channels': 12, 'guard_timer_active': True}


if __name__ == '__main__':
    if sys.argv[1:] not in (['recover'], ['prepare']):
        raise SystemExit('Usage: ugc-astral-kernel-recovery.py recover|prepare')
    try:
        print(json.dumps(recover(), sort_keys=True))
    except Exception as exc:
        print(f'RECOVERY BLOCKED/FAILED: {exc}', file=sys.stderr)
        raise SystemExit(1)
