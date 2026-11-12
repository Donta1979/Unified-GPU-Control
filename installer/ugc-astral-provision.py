#!/usr/bin/env python3
"""UGC Astral provisioning: authenticated input required.

This helper must only be invoked by an independently authenticated
installation process. It never activates Astral Guard.
"""
import hashlib
import json
import os
import stat
import subprocess
import sys
import tempfile
from pathlib import Path
import shutil

TRUSTED_PUBLIC_KEY_DER_SHA256 = (
    "6965d54f1b12011a01dcb23ec8414c55803c960081135624afc922c1b15126b3"
)

TRUSTED_PUBLIC_KEY_PEM = b"""-----BEGIN PUBLIC KEY-----
MCowBQYDK2VwAyEAjK/mVWWMfPDc5whPfuAeCoNZ6FFq23h1hXtfG5J7uJQ=
-----END PUBLIC KEY-----
"""

DEST = Path("/var/lib/ugc")
STAGE = DEST / "astral-release"
ENTRY = STAGE / "ugc-astral-deploy.py"

def fail(message):
    raise RuntimeError(message)

def digest(path):
    h = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()

def check_source(root, relative):
    if relative.is_absolute() or ".." in relative.parts or not relative.parts:
        fail("Unsafe package path")

    current = root
    for component in relative.parts:
        current = current / component
        if current.is_symlink():
            fail("Symlink in package: " + str(relative))

    if not current.is_file():
        fail("Missing package file: " + str(relative))

    return current

def main():
    if os.geteuid() != 0:
        fail("Administrator authorization required")

    if len(sys.argv) != 2:
        fail("Expected RELEASE_DIRECTORY")

    source = Path(sys.argv[1]).resolve(strict=True)
    manifest_path = check_source(source, Path("PACKAGE-SHA256.json"))
    signature_path = check_source(source, Path("PACKAGE-SHA256.sig"))

    # Authenticate the manifest before trusting its file inventory.
    # The public key is embedded, not loaded from the release archive.
    with tempfile.TemporaryDirectory(prefix="ugc-signature-") as tmp:
        public_key = Path(tmp) / "trusted-public.pem"
        public_key.write_bytes(TRUSTED_PUBLIC_KEY_PEM)

        der = subprocess.run(
            ["openssl", "pkey", "-pubin", "-in", str(public_key),
             "-outform", "DER"],
            check=True, capture_output=True
        ).stdout

        if hashlib.sha256(der).hexdigest() != TRUSTED_PUBLIC_KEY_DER_SHA256:
            fail("Pinned signing key fingerprint mismatch")

        result = subprocess.run(
            ["openssl", "pkeyutl", "-verify", "-rawin", "-pubin",
             "-inkey", str(public_key),
             "-in", str(manifest_path),
             "-sigfile", str(signature_path)],
            capture_output=True, text=True
        )

        if result.returncode != 0:
            fail("Astral release signature verification failed")

    manifest = json.loads(manifest_path.read_text())

    if manifest.get("schema_version") != 1:
        fail("Unsupported manifest")

    files = manifest.get("files")

    if not isinstance(files, dict) or not files:
        fail("Invalid file inventory")

    required_entry = "ugc-astral-deploy.py"

    if required_entry not in files:
        fail("Deployment entry point missing")

    # Refuse existing or partially installed protection.
    existing = [
        "/var/usrlocal/lib/astral-guard-manager",
        "/var/usrlocal/bin/astral-guard",
        "/var/usrlocal/lib/astral-hwmon/astral-hwmon.ko",
        "/etc/systemd/system/astral-hwmon.service",
        "/etc/systemd/system/astral-guard.service",
        "/etc/systemd/system/astral-guard.timer",
        "/etc/systemd/system/astral-guard-poweroff.service",
    ]

    for name in existing:
        p = Path(name)
        if p.exists() or p.is_symlink():
            fail("Existing protection detected: " + name)

    if STAGE.exists() or STAGE.is_symlink():
        fail("Release already provisioned")

    # Do not follow a preexisting unsafe destination.
    if DEST.is_symlink():
        fail("Unsafe destination")

    if not DEST.exists():
        DEST.mkdir(mode=0o755, parents=True, exist_ok=False)

    if DEST.stat().st_uid != 0 or DEST.stat().st_mode & 0o022:
        fail("Destination ownership or permissions unsafe")

    # Prepare all files before publishing anything.
    temporary = Path(tempfile.mkdtemp(prefix=".astral-stage-", dir=DEST))

    published = False

    try:
        for name, expected in files.items():
            if not isinstance(name, str) or not isinstance(expected, str):
                fail("Invalid manifest entry")

            relative = Path(name)
            original = check_source(source, relative)

            if digest(original) != expected:
                fail("Checksum mismatch: " + name)

            target = temporary / relative
            target.parent.mkdir(parents=True, exist_ok=True)

            with original.open("rb") as inp:
                fd = os.open(
                    target,
                    os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW,
                    0o600
                )
                with os.fdopen(fd, "wb") as out:
                    shutil.copyfileobj(inp, out)
                    out.flush()
                    os.fsync(out.fileno())

            if digest(target) != expected:
                fail("Copied file verification failed: " + name)

            target.chmod(0o755 if name == required_entry else 0o644)

        temporary.chmod(0o755)

        # Refuse replacement even if another installer created STAGE.
        # A directory rename is atomic only when the destination
        # does not already exist; renameat2 enforces that here.
        import ctypes

        libc = ctypes.CDLL(None, use_errno=True)
        result = libc.renameat2(
            -100, os.fsencode(temporary),
            -100, os.fsencode(STAGE), 1
        )

        if result != 0:
            err = ctypes.get_errno()
            raise OSError(err, os.strerror(err))

        published = True

    finally:
        if not published and temporary.exists():
            shutil.rmtree(temporary)

    print("PASS: Complete Astral release provisioned.")
    print("PASS: No existing protection replaced.")
    print("PASS: Astral Guard not activated.")
    print("Entrypoint:", ENTRY)

if __name__ == "__main__":
    try:
        main()
    except Exception as exc:
        print("ERROR:", exc, file=sys.stderr)
        sys.exit(1)
