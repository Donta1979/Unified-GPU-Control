#!/usr/bin/env python3
"""UGC Astral clean-host transaction. Root-owned entry point only.

This source must be independently reviewed and provisioned by a trusted
packaging process. Never run it from Downloads with sudo/pkexec.
"""
import ctypes
import hashlib
import os
from pathlib import Path
import runpy
import stat
import sys
import tempfile
import shutil

STAGE = Path('/var/lib/ugc/astral-release')
BASE = Path('/var/usrlocal/lib/astral-guard-manager')
PINNED = {'payload/SHA256SUMS': '7532be3ff0ee0f9ef2d83b882f77694aa96ff4f7a5531b6cccd41ebda9c6a02b', 'payload/astral-guard': '1d0cddf86c3b5a0b44174ea2b9b0f6881b65d4f78a399a4d948a5782c0631c38', 'payload/astral-hwmon.ko': '03d81f8b9aed4a75caac136b2b4ae59521accc5dc35a1be033f8e6f60ff16845', 'payload/licenses/GPL-2.0.txt': 'edaef632cbb643e4e7a221717a6c441a4c1a7c918e6e4d56debc3d8739b233f6', 'payload/licenses/MIT.txt': '1fd5e6615fcd24827864b425c023fca6a7d027115935f436880bb00754edf722', 'payload/units/astral-guard-poweroff.service': '3f1582f8710adaabda72fc62b0a0119cc3798157e3f8205ef4df4111941ddd5f', 'payload/units/astral-guard.service': 'd9aaa82cb641afda0e604664d7b52e0602761ad8cffe96003550fe1c6a12ae4f', 'payload/units/astral-guard.timer': 'd5410290a64047859fe16ab7a81dcab0014e8c5ec7365422e75d82652e71b8a5', 'payload/units/astral-hwmon.service': '1eb3f0a5340e27397036332410a0fdbf9f28d6662ee29b2f0a5859dd5e9a81b6', 'payload/units/ugc-astral-kernel-prepare.service': 'f5fbd09eadaef790a6b9f27865939fefe02957ef041080c240d9ceb7205ba566', 'ugc-astral-fresh-install.py': 'f9a8b52232c80ddac010fd38d8c259430d1089fbd8f3440e8949acd0aedd5bec', 'ugc-astral-helper.py': 'f7cbdbbb38df3da0cf7450e78510f5441158589eb289551ee2317ab3340b8efe'}
def trusted(path, directory=False):
    path = Path(os.path.abspath(path))
    for part in (path, *path.parents):
        s = part.lstat()
        if s.st_uid != 0 or s.st_mode & 0o022 or stat.S_ISLNK(s.st_mode):
            raise RuntimeError('Untrusted ownership or permissions: ' + str(part))
        if part != path and not stat.S_ISDIR(s.st_mode):
            raise RuntimeError('Non-directory ancestor: ' + str(part))
    s = path.lstat()
    if directory and not stat.S_ISDIR(s.st_mode):
        raise RuntimeError('Not a directory: ' + str(path))
    if not directory and (not stat.S_ISREG(s.st_mode) or s.st_nlink != 1):
        raise RuntimeError('Not a safe regular file: ' + str(path))
    return path


def read_pinned(stage, name, digest):
    path = trusted(stage / name)
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC)
    try:
        s = os.fstat(fd)
        if not stat.S_ISREG(s.st_mode) or s.st_uid != 0 or s.st_mode & 0o022 or s.st_nlink != 1 or s.st_size > 10_000_000:
            raise RuntimeError('Unsafe staged file: ' + name)
        with os.fdopen(os.dup(fd), 'rb') as f:
            data = f.read()
    finally:
        os.close(fd)
    if hashlib.sha256(data).hexdigest() != digest:
        raise RuntimeError('Release-pinned digest mismatch: ' + name)
    return data


def publish_no_replace(src, dst):
    # Linux renameat2 RENAME_NOREPLACE: prevents destination overwrite races.
    libc = ctypes.CDLL(None, use_errno=True)
    result = libc.renameat2(-100, os.fsencode(src), -100, os.fsencode(dst), 1)
    if result != 0:
        err = ctypes.get_errno()
        raise OSError(err, os.strerror(err), str(dst))


def main():
    if os.geteuid() != 0:
        raise RuntimeError('Administrator authorization required')
    if sys.argv[1:] != ['install-fresh']:
        raise RuntimeError('Expected exactly: install-fresh')
    trusted(__file__)
    stage = trusted(STAGE, directory=True)
    if BASE.exists() or BASE.is_symlink():
        raise RuntimeError('Existing Astral manager detected: refusing installation')
    parent = trusted(BASE.parent, directory=True)
    # Validate every byte before making changes.
    contents = {name: read_pinned(stage, name, digest) for name, digest in PINNED.items()}
    # Validate the inner payload manifest against the independently embedded pins.
    listed = {}
    for line in contents['payload/SHA256SUMS'].decode('ascii').splitlines():
        digest, name = line.split(maxsplit=1)
        name = name.lstrip('*')
        if name in listed or 'payload/' + name not in contents:
            raise RuntimeError('Unexpected payload manifest entry')
        listed[name] = digest
    payload_names = {n.removeprefix('payload/') for n in contents if n.startswith('payload/') and n != 'payload/SHA256SUMS'}
    if set(listed) != payload_names or any(hashlib.sha256(contents['payload/'+n]).hexdigest() != h for n,h in listed.items()):
        raise RuntimeError('Payload manifest mismatch')
    # Fail closed on already installed or partially installed protection.
    for target in ('/var/usrlocal/bin/astral-guard', '/var/usrlocal/lib/astral-hwmon/astral-hwmon.ko',
                   '/etc/systemd/system/astral-hwmon.service', '/etc/systemd/system/astral-guard.service',
                   '/etc/systemd/system/astral-guard.timer', '/etc/systemd/system/astral-guard-poweroff.service'):
        p = Path(target)
        if p.exists() or p.is_symlink():
            raise RuntimeError('Existing Astral protection detected: ' + target)
    temp = Path(tempfile.mkdtemp(prefix='.ugc-astral-', dir=parent))
    published = False
    try:
        for name, data in contents.items():
            dest = temp / name
            dest.parent.mkdir(parents=True, exist_ok=True)
            fd = os.open(dest, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
            with os.fdopen(fd, 'wb') as f:
                f.write(data)
                f.flush()
                os.fsync(f.fileno())
            dest.chmod(0o755 if name in ('ugc-astral-helper.py','payload/astral-guard') else 0o644)
        publish_no_replace(temp, BASE)
        published = True
        # The fresh-install module performs its own preflight and activates the
        # protection only on a clean, compatible host.
        try:
            fresh = runpy.run_path(str(BASE/'ugc-astral-fresh-install.py'), run_name='__ugc_internal__')
            result = fresh['fresh_install']()
        except Exception as exc:
            raise RuntimeError(
                'ASTRAL FRESH INSTALL FAILED after support files were published. '
                'A partial installation may exist. Do NOT rerun the installer, '
                'delete files, or disable services automatically. '
                'Inspect the host and Astral Guard status before recovery. '
                f'Original error: {exc}'
            ) from exc
        print(result)
    finally:
        if not published and temp.exists():
            shutil.rmtree(temp)
        # After publication, retain support files for diagnosis; never remove
        # or stop possibly active emergency protection automatically.


if __name__ == '__main__':
    try:
        main()
    except Exception as exc:
        print('ERROR:', exc, file=sys.stderr)
        sys.exit(1)
