#!/usr/bin/env python3
"""Standalone read-only Astral Guard status publisher for UGC."""

import glob
import json
import os
import re
import shutil
import subprocess
import tempfile
import time
from pathlib import Path

APP_ID = "io.github.Donta1979.UnifiedGPUControl"
SYSTEMCTL = shutil.which("systemctl") or "/usr/bin/systemctl"
GUARD_CANDIDATES = [
    "/var/usrlocal/bin/astral-guard",
    "/usr/local/bin/astral-guard",
    "/usr/bin/astral-guard",
]

OUTPUT = (
    Path.home()
    / ".var/app"
    / APP_ID
    / "data/astral-status.json"
)

def run(args, timeout=12):
    try:
        p = subprocess.run(args, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=timeout)
        return p.returncode, p.stdout.strip()
    except Exception as e:
        return 127, str(e)

def show(unit):
    props = "LoadState,ActiveState,SubState,UnitFileState,Result,ExecMainStatus,ExecMainStartTimestamp"
    rc, out = run([SYSTEMCTL, "show", unit, "--no-pager", f"--property={props}"])
    d = {}
    for line in out.splitlines():
        if "=" in line:
            k, v = line.split("=", 1); d[k] = v
    return d

def unit_text(unit):
    rc, out = run([SYSTEMCTL, "cat", unit, "--no-pager"])
    return out if rc == 0 else ""

def status():
    units = {u: show(u) for u in (
        "astral-hwmon.service", "astral-guard.service",
        "astral-guard.timer", "astral-guard-poweroff.service")}
    guard = next((p for p in GUARD_CANDIDATES if os.path.isfile(p) and os.access(p, os.X_OK)), None)
    sensor = None
    for nf in glob.glob("/sys/class/hwmon/hwmon*/name"):
        try:
            if Path(nf).read_text().strip() == "astral12vhpwr":
                root = Path(nf).parent
                sensor = {"path": str(root),
                          "current_channels": sum((root/f"curr{i}_input").exists() for i in range(1,7)),
                          "voltage_channels": sum((root/f"in{i}_input").exists() for i in range(6))}
                break
        except OSError:
            pass
    pins = []
    if sensor:
        root = Path(sensor["path"])
        for i in range(6):
            try:
                mv = int((root/f"in{i}_input").read_text().strip())
                ma = int((root/f"curr{i+1}_input").read_text().strip())
                pins.append({"pin":i+1,"volts":round(mv/1000,3),"amps":round(ma/1000,3),"mv":mv,"ma":ma})
            except (OSError, ValueError):
                pins.append({"pin":i+1,"volts":None,"amps":None,"mv":None,"ma":None})
    kernel = run(["uname", "-r"])[1]
    module = "/var/usrlocal/lib/astral-hwmon/astral-hwmon.ko"
    modver = None
    if os.path.exists(module):
        rc, out = run(["modinfo", "-F", "vermagic", module])
        if rc == 0 and out: modver = out.split()[0]
    driver_txt = unit_text("astral-hwmon.service")
    guard_txt = unit_text("astral-guard.service")
    timer_txt = unit_text("astral-guard.timer")
    power_txt = unit_text("astral-guard-poweroff.service")
    checks = []
    def add(label, ok, detail, warning=False):
        checks.append({"label":label, "ok":bool(ok), "detail":detail, "warning":warning})
    loaded = Path("/sys/module/astral_hwmon").exists()
    add("Kernel driver loaded", loaded, "astral_hwmon is present in the running kernel" if loaded else "The astral_hwmon kernel module is not loaded")
    driver_enabled = units["astral-hwmon.service"].get("UnitFileState") == "enabled"
    add("Driver starts at boot", driver_enabled, "astral-hwmon.service is enabled" if driver_enabled else "astral-hwmon.service is not enabled")
    sensor_ok = bool(sensor and sensor["current_channels"] == 6 and sensor["voltage_channels"] == 6)
    add("Sensor interface available", sensor_ok,
        (f"Found {sensor['current_channels']}/6 current and {sensor['voltage_channels']}/6 voltage channels" if sensor else "No astral12vhpwr hwmon device found"))
    add("Guard executable installed", bool(guard), guard or "astral-guard executable not found")
    timer_ok = units["astral-guard.timer"].get("ActiveState") == "active" and units["astral-guard.timer"].get("UnitFileState") == "enabled"
    add("Periodic guard timer", timer_ok, "Timer is enabled and active" if timer_ok else "Timer is not both enabled and active")
    trigger_ok = ("OnFailure=astral-guard-poweroff.service" in guard_txt and
                  re.search(r"(?m)^SuccessExitStatus=.*\b1\b.*\b3\b", guard_txt) is not None)
    add("Critical-only trigger", trigger_ok,
        "Guard is wired to the shutdown unit; WARNING and UNKNOWN are excluded" if trigger_ok else "Expected OnFailure / SuccessExitStatus settings not found")
    power_ok = (re.search(r"(?m)^ExecStartPre=.*MONITOR_EXIT_STATUS.*=.*2", power_txt) is not None and
                re.search(r"(?m)^ExecStartPre=.*sleep\s+(?:5|10|15|20|30|60)\s*$", power_txt) is not None and
                re.search(r"(?m)^ExecStart=/usr/bin/systemctl poweroff\s*$", power_txt) is not None)
    add("Emergency shutdown action", power_ok,
        "CRITICAL interlock, configured countdown and poweroff action found" if power_ok else "Poweroff unit missing or safety interlock does not match")
    add("Kernel module matches boot", modver == kernel,
        f"Module built for {modver or 'unknown'}; running kernel is {kernel}",
        warning=True)
    # Kernel-update recovery readiness; diagnostic only.
    build_dir = Path("/lib/modules") / kernel / "build"
    recovery = {
        "kernel_build_available": build_dir.is_dir(),
        "dkms_available": shutil.which("dkms") is not None,
        "make_available": shutil.which("make") is not None,
        "gcc_available": shutil.which("gcc") is not None,
        "module_matches_kernel": modver == kernel,
    }
    recovery["build_prerequisites_available"] = all(
        recovery[key] for key in (
            "kernel_build_available",
            "dkms_available",
            "make_available",
            "gcc_available",
        )
    )
    last = units["astral-guard.service"]
    return {"recovery":recovery, "time":time.strftime("%Y-%m-%d %H:%M:%S"), "kernel":kernel,
            "module_version":modver or "not found", "guard_path":guard or "not found",
            "sensor":sensor, "pins":pins, "units":units, "checks":checks,
            "all_ok":all(c["ok"] for c in checks),
            "last_result":last.get("Result","unknown"),
            "last_exit":last.get("ExecMainStatus","unknown"),
            "last_run":last.get("ExecMainStartTimestamp","not recorded")}


def collect():
    result = status()
    return {
        "schema_version": 1,
        "generated_at": int(time.time()),
        "all_ok": result["all_ok"],
        "checks": result["checks"],
        "last_result": result["last_result"],
        "last_exit": result["last_exit"],
        "kernel": result["kernel"],
        "module_version": result["module_version"],
        "recovery": result["recovery"],
    }


def publish(data):
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)

    fd, temporary = tempfile.mkstemp(
        prefix=".astral-status-",
        suffix=".tmp",
        dir=OUTPUT.parent,
    )

    try:
        with os.fdopen(fd, "w") as stream:
            json.dump(data, stream)
            stream.write("\n")
            stream.flush()
            os.fsync(stream.fileno())

        os.chmod(temporary, 0o600)
        os.replace(temporary, OUTPUT)

    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)

def main():
    try:
        data = collect()
    except Exception as error:
        data = {
            "schema_version": 1,
            "generated_at": int(time.time()),
            "all_ok": False,
            "checks": [],
            "error": str(error),
        }

    publish(data)

    print(
        "PASS: Status snapshot published"
        if data["all_ok"]
        else "WARNING: Status snapshot reports incomplete protection"
    )

if __name__ == "__main__":
    main()
