#!/usr/bin/env python3
"""UGC fresh-install transaction, NOT YET WIRED TO THE PRIVILEGED HELPER.

Only for a genuinely clean host. Must be reviewed and tested on an isolated
Bazzite system before exposing it through a privileged action or the GUI.
"""
import hashlib
import os
from pathlib import Path
import stat
import subprocess

BASE = Path('/var/usrlocal/lib/astral-guard-manager')
PAYLOAD = BASE / 'payload'
TARGETS = {
    'astral-guard': Path('/var/usrlocal/bin/astral-guard'),
    'astral-hwmon.ko': Path('/var/usrlocal/lib/astral-hwmon/astral-hwmon.ko'),
    **{f'units/{name}': Path('/etc/systemd/system') / name for name in (
        'astral-hwmon.service', 'astral-guard.service',
        'astral-guard.timer', 'astral-guard-poweroff.service')},
}
EXPECTED = set(TARGETS) | {'licenses/GPL-2.0.txt', 'licenses/MIT.txt'}


def run(*args):
    p = subprocess.run(args, capture_output=True, text=True, timeout=30)
    if p.returncode:
        raise RuntimeError(f'{args[0]} failed: {p.stderr[-500:]}')
    return p.stdout.strip()


def secure_file(path):
    s = path.lstat()
    if not stat.S_ISREG(s.st_mode) or s.st_uid != 0 or s.st_mode & 0o022 or s.st_nlink != 1:
        raise RuntimeError(f'Unsafe payload file: {path}')


def preflight():
    if os.geteuid() != 0:
        raise RuntimeError('Root required')
    for d in (BASE, PAYLOAD, PAYLOAD / 'units', PAYLOAD / 'licenses'):
        s = d.lstat()
        if not stat.S_ISDIR(s.st_mode) or s.st_uid != 0 or s.st_mode & 0o022:
            raise RuntimeError(f'Unsafe payload directory: {d}')
    manifest = PAYLOAD / 'SHA256SUMS'
    secure_file(manifest)
    hashes = {}
    for line in manifest.read_text().splitlines():
        digest, name = line.split(maxsplit=1)
        name = name.lstrip('*')
        if name not in EXPECTED or name in hashes or len(digest) != 64 or any(c not in '0123456789abcdef' for c in digest):
            raise RuntimeError('Invalid payload manifest')
        hashes[name] = digest
    if set(hashes) != EXPECTED:
        raise RuntimeError('Incomplete payload manifest')
    for name, digest in hashes.items():
        p = PAYLOAD / name
        secure_file(p)
        if hashlib.sha256(p.read_bytes()).hexdigest() != digest:
            raise RuntimeError(f'Payload checksum mismatch: {name}')
    if run('modinfo', '-F', 'vermagic', str(PAYLOAD / 'astral-hwmon.ko')).split()[0] != os.uname().release:
        raise RuntimeError('Kernel module mismatch')
    if any(p.exists() or p.is_symlink() for p in TARGETS.values()):
        raise RuntimeError('Existing or partial installation; no changes made')
    if Path('/sys/module/astral_hwmon').exists():
        raise RuntimeError('Module already loaded; no changes made')
    for unit in ('astral-hwmon.service', 'astral-guard.service', 'astral-guard.timer', 'astral-guard-poweroff.service'):
        if run('systemctl', 'show', unit, '-p', 'LoadState', '--value') != 'not-found':
            raise RuntimeError(f'Existing systemd unit detected: {unit}')


def fresh_install():
    """Clean-host only; retain files if service activation outcome is uncertain."""
    preflight()
    created_files, created_dirs, enabled = [], [], []
    activation_attempted = False
    try:
        for name, target in TARGETS.items():
            missing = []
            parent = target.parent
            while not parent.exists():
                missing.append(parent)
                parent = parent.parent
            for ancestor in (parent, *parent.parents):
                if ancestor.is_symlink() or not ancestor.is_dir():
                    raise RuntimeError(f'Unsafe ancestor: {ancestor}')
            for directory in reversed(missing):
                directory.mkdir(mode=0o755)
                created_dirs.append(directory)
            # O_EXCL prevents overwriting an existing destination.
            fd = os.open(target, os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, 'O_NOFOLLOW', 0),
                         0o755 if name == 'astral-guard' else 0o644)
            created_files.append(target)
            with os.fdopen(fd, 'wb') as out:
                out.write((PAYLOAD / name).read_bytes())
                out.flush()
                os.fsync(out.fileno())
            os.chmod(target, 0o755 if name == 'astral-guard' else 0o644)
        # Bazzite SELinux: label the newly installed kernel module correctly.
        # Never disable SELinux or modify an existing installation.
        module = TARGETS['astral-hwmon.ko']
        rc = subprocess.run(
            ['chcon', '-t', 'modules_object_t', str(module)],
            capture_output=True, text=True, timeout=30
        )
        if rc.returncode:
            raise RuntimeError(
                'Cannot apply required kernel module SELinux label: '
                + rc.stderr.strip()
            )

        run('systemctl', 'daemon-reload')
        # Once activation begins, never tear down potentially live protection
        # automatically on an uncertain systemd outcome.
        activation_attempted = True
        run('systemctl', 'enable', 'astral-hwmon.service')
        enabled.append('astral-hwmon.service')
        run('systemctl', 'start', 'astral-hwmon.service')
        if not Path('/sys/module/astral_hwmon').exists():
            raise RuntimeError('Module failed to load')
        # Before activating the timer, confirm a live sensor is present.
        sensors = [p for p in Path('/sys/class/hwmon').glob('hwmon*')
                   if (p / 'name').read_text().strip() == 'astral12vhpwr']
        if len(sensors) != 1 or len(list(sensors[0].glob('curr*_input'))) != 6 or len(list(sensors[0].glob('in*_input'))) != 6:
            raise RuntimeError('Astral sensor channels unavailable')
        # Validate all six live current and voltage readings before
        # enabling periodic emergency monitoring.
        sensor = sensors[0]
        for i in range(6):
            try:
                ma = int((sensor / f'curr{i+1}_input').read_text().strip())
                mv = int((sensor / f'in{i}_input').read_text().strip())
            except (OSError, ValueError) as exc:
                raise RuntimeError(
                    f'Cannot read Astral sensor channel {i+1}'
                ) from exc

            if not (0 <= ma <= 20000 and 10500 <= mv <= 13500):
                raise RuntimeError(
                    f'Invalid Astral sensor reading on channel {i+1}'
                )

        run('systemctl', 'enable', 'astral-guard.timer')
        enabled.append('astral-guard.timer')
        run('systemctl', 'start', 'astral-guard.timer')
        for unit in enabled:
            if run('systemctl', 'is-enabled', unit) != 'enabled' or run('systemctl', 'is-active', unit) != 'active':
                raise RuntimeError(f'Activation verification failed: {unit}')
    except Exception as error:
        # Never stop/disable services after any activation attempt. systemctl
        # can change state before reporting failure; guard protection might
        # already be live. Retain files and require explicit manual recovery.
        if activation_attempted or Path('/sys/module/astral_hwmon').exists():
            raise RuntimeError(
                'Installation incomplete after service activation was attempted; '
                'installed files and any running protection were left untouched. '
                'Manual recovery required; do not assume monitoring is active.'
            ) from error
        failures = []
        for target in reversed(created_files):
            try: target.unlink()
            except Exception as exc: failures.append(str(exc))
        for directory in reversed(created_dirs):
            try: directory.rmdir()
            except Exception as exc: failures.append(str(exc))
        try: run('systemctl', 'daemon-reload')
        except Exception as exc: failures.append(str(exc))
        if failures:
            raise RuntimeError('Pre-activation rollback incomplete: ' + '; '.join(failures)) from error
        raise
    return 'Fresh Astral installation activated'

# Intentionally NO main entry point and NO GUI/privileged-helper integration yet.
