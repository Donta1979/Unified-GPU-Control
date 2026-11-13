#!/usr/bin/env python3
"""UGC Astral kernel compatibility preflight.

Read-only. Never installs, removes, loads, or unloads kernel modules.
"""
import json
import os
import shutil
import subprocess
from pathlib import Path

def command(*args):
    try:
        p = subprocess.run(
            args, capture_output=True, text=True, timeout=10
        )
        return p.stdout.strip() if p.returncode == 0 else ""
    except (OSError, subprocess.TimeoutExpired):
        return ""

def main():
    kernel = os.uname().release
    build = Path("/lib/modules") / kernel / "build"

    dkms = shutil.which("dkms") is not None
    make = shutil.which("make") is not None
    compiler = shutil.which("gcc") is not None

    build_available = build.is_dir()

    module_loaded = Path("/sys/module/astral_hwmon").exists()

    module_path = Path(
        "/var/usrlocal/lib/astral-hwmon/astral-hwmon.ko"
    )

    installed_vermagic = (
        command("modinfo", "-F", "vermagic", str(module_path))
        if module_path.is_file() else ""
    )

    module_matches = bool(
        installed_vermagic
        and installed_vermagic.split()[0] == kernel
    )

    sensors = []
    for p in Path("/sys/class/hwmon").glob("hwmon*"):
        try:
            if (p / "name").read_text().strip() == "astral12vhpwr":
                sensors.append(p)
        except OSError:
            pass

    sensor_ready = False

    if len(sensors) == 1:
        s = sensors[0]
        sensor_ready = all(
            (s / f"curr{i+1}_input").is_file()
            and (s / f"in{i}_input").is_file()
            for i in range(6)
        )

    guard_timer = command(
        "systemctl", "is-active", "astral-guard.timer"
    ) == "active"

    guard_enabled = command(
        "systemctl", "is-enabled", "astral-guard.timer"
    ) == "enabled"

    report = {
        "kernel": kernel,
        "kernel_build_available": build_available,
        "dkms_available": dkms,
        "make_available": make,
        "gcc_available": compiler,
        "module_loaded": module_loaded,
        "installed_module_matches_kernel": module_matches,
        "sensor_ready": sensor_ready,
        "guard_timer_active": guard_timer,
        "guard_timer_enabled": guard_enabled,
    }

    report["protection_appears_operational"] = (
        module_loaded
        and sensor_ready
        and guard_timer
        and guard_enabled
    )

    report["kernel_build_prerequisites_available"] = (
        build_available and dkms and make and compiler
    )

    print(json.dumps(report, indent=2))

if __name__ == "__main__":
    main()
