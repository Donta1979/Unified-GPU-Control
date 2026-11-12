#!/usr/bin/env python3
"""Verify Astral release package consistency without privileges."""
import ast
import hashlib
import json
import sys
from pathlib import Path

def main():
    if len(sys.argv) != 2:
        raise RuntimeError("Usage: verify-astral-release.py PACKAGE_DIRECTORY")

    root = Path(sys.argv[1]).resolve(strict=True)
    manifest = json.loads((root / "PACKAGE-SHA256.json").read_text())

    if manifest.get("schema_version") != 1:
        raise RuntimeError("Unsupported release manifest")

    files = manifest["files"]
    deploy = root / "ugc-astral-deploy.py"

    tree = ast.parse(deploy.read_text())
    pins = next(
        ast.literal_eval(node.value)
        for node in tree.body
        if isinstance(node, ast.Assign)
        and any(
            isinstance(target, ast.Name) and target.id == "PINNED"
            for target in node.targets
        )
    )

    # PINNED is the legacy clean-host installer inventory, not the entire
    # release inventory. Recovery source and entrypoint are separately packaged.
    required_pins = {
        name for name in files
        if name.startswith('payload/') or name in (
            'ugc-astral-fresh-install.py', 'ugc-astral-helper.py'
        )
    }
    if set(pins) != required_pins:
        raise RuntimeError("Clean-host deployment inventory mismatch")
    if 'ugc-astral-kernel-recovery.py' not in files:
        raise RuntimeError("Kernel recovery entrypoint missing")
    if not any(name.startswith('kernel-source/') for name in files):
        raise RuntimeError("Kernel source missing")

    for name, expected in files.items():
        relative = Path(name)

        if relative.is_absolute() or ".." in relative.parts:
            raise RuntimeError("Unsafe manifest path")

        path = root / relative

        if not path.is_file() or path.is_symlink():
            raise RuntimeError("Missing or unsafe file: " + name)

        actual = hashlib.sha256(path.read_bytes()).hexdigest()

        if actual != expected:
            raise RuntimeError("Checksum mismatch: " + name)

        if name in pins and pins[name] != actual:
            raise RuntimeError("Deployment checksum mismatch: " + name)

    print(f"PASS: All {len(files)} packaged Astral files verified.")
    print("PASS: Deployment checksum references verified.")
    print("NOTE: This verifies consistency, not publisher identity.")
    print("No privileged operations performed.")

if __name__ == "__main__":
    try:
        main()
    except Exception as exc:
        print("FAIL:", exc, file=sys.stderr)
        sys.exit(1)
