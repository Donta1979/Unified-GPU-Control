#!/usr/bin/env python3
"""UGC Astral recovery installation preflight.

Read-only. Does not install, load, unload, or replace modules.
"""
import json
import os
import subprocess
from pathlib import Path

MODULE = Path("/var/usrlocal/lib/astral-hwmon/astral-hwmon.ko")
TRUSTED = Path("/var/lib/ugc/astral-release")
SOURCE = TRUSTED / "kernel-source"


def command(*args):
    result = subprocess.run(
        args,
        capture_output=True,
        text=True,
        timeout=15,
    )
    return result.returncode, result.stdout.strip()


def inspect():
    kernel = os.uname().release

    installed = None
    if MODULE.is_file():
        rc, output = command("modinfo", "-F", "vermagic", str(MODULE))
        if rc == 0 and output:
            installed = output.split()[0]

    loaded = Path("/sys/module/astral_hwmon").exists()

    sensor_ready = False
    for sensor in Path("/sys/class/hwmon").glob("hwmon*"):
        try:
            if (sensor / "name").read_text().strip() != "astral12vhpwr":
                continue
            sensor_ready = all(
                (sensor / f"curr{i + 1}_input").is_file()
                and (sensor / f"in{i}_input").is_file()
                for i in range(6)
            )
            if sensor_ready:
                break
        except OSError:
            continue

    rc, timer = command("systemctl", "is-active", "astral-guard.timer")
    timer_active = rc == 0 and timer == "active"

    rc, startup = command("systemctl", "is-active", "astral-hwmon.service")
    startup_active = rc == 0 and startup == "active"

    trusted_source_present = SOURCE.is_dir()
    trusted_source_root_owned = (
        trusted_source_present
        and SOURCE.stat().st_uid == 0
        and not (SOURCE.stat().st_mode & 0o022)
    )

    healthy = (
        installed == kernel
        and loaded
        and sensor_ready
        and timer_active
        and startup_active
    )

    mismatch = installed is not None and installed != kernel

    if healthy:
        decision = "no_repair_needed"
    elif mismatch:
        decision = "kernel_recovery_candidate"
    elif installed is None:
        decision = "module_missing_or_unreadable"
    elif not loaded:
        decision = "startup_repair_needed"
    elif not sensor_ready:
        decision = "sensor_diagnostics_needed"
    else:
        decision = "service_repair_needed"

    return {
        "running_kernel": kernel,
        "installed_module_kernel": installed,
        "module_loaded": loaded,
        "sensor_ready": sensor_ready,
        "guard_timer_active": timer_active,
        "driver_service_active": startup_active,
        "trusted_source_present": trusted_source_present,
        "trusted_source_directory_root_owned": trusted_source_root_owned,
        "protection_appears_operational": healthy,
        "decision": decision,
        "installation_authorized": False,
        "note": "Preflight only. No privileged installation implemented.",
    }


if __name__ == "__main__":
    print(json.dumps(inspect(), indent=2))
