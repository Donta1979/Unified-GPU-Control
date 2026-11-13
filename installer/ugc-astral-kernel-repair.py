#!/usr/bin/env python3
"""UGC Astral kernel recovery: preflight and isolated module build.

Never installs, unloads, replaces or activates kernel modules.
"""
import argparse
import json
import os
import shutil
import subprocess
import tempfile
from pathlib import Path

MODULE = Path("/var/usrlocal/lib/astral-hwmon/astral-hwmon.ko")
SOURCE = Path.home() / "astral-hwmon" / "driver"

def run(args, timeout=180):
    return subprocess.run(
        args, text=True, capture_output=True, timeout=timeout
    )

def inspect():
    kernel = os.uname().release
    build = Path("/lib/modules") / kernel / "build"

    installed = None
    if MODULE.is_file():
        result = run(["modinfo", "-F", "vermagic", str(MODULE)])
        if result.returncode == 0:
            installed = result.stdout.split()[0]

    sensors = []
    for root in Path("/sys/class/hwmon").glob("hwmon*"):
        try:
            if (root / "name").read_text().strip() == "astral12vhpwr":
                sensors.append(root)
        except OSError:
            pass

    sensor_ready = len(sensors) == 1 and all(
        (sensors[0] / f"curr{i+1}_input").is_file()
        and (sensors[0] / f"in{i}_input").is_file()
        for i in range(6)
    )

    def active(unit, operation, expected):
        result = run(["systemctl", operation, unit])
        return result.returncode == 0 and result.stdout.strip() == expected

    operational = (
        installed == kernel
        and Path("/sys/module/astral_hwmon").exists()
        and sensor_ready
        and active("astral-hwmon.service", "is-active", "active")
        and active("astral-guard.timer", "is-active", "active")
        and active("astral-guard.timer", "is-enabled", "enabled")
    )

    required = [
        "Kbuild",
        "astral_hwmon.c",
        "astral_detect.c",
        "astral_regs.c",
        "astral.h",
    ]

    prerequisites = (
        build.is_dir()
        and shutil.which("make") is not None
        and shutil.which("gcc") is not None
        and all((SOURCE / name).is_file() for name in required)
    )

    return {
        "kernel": kernel,
        "installed_module_kernel": installed,
        "module_loaded": Path("/sys/module/astral_hwmon").exists(),
        "sensor_ready": sensor_ready,
        "protection_appears_operational": operational,
        "build_prerequisites_available": prerequisites,
        "build_directory": str(build),
        "source_directory": str(SOURCE),
        "rebuild_needed": not operational,
    }


def recovery_decision(report):
    """Determine the required recovery without changing the system."""
    if report["protection_appears_operational"]:
        return {
            "action": "none",
            "reason": "Astral protection appears operational.",
        }

    installed = report["installed_module_kernel"]
    running = report["kernel"]

    if installed is None:
        return {
            "action": "restore_module",
            "reason": "Installed Astral kernel module is missing or unreadable.",
        }

    if installed != running:
        return {
            "action": "rebuild_module",
            "reason": "Installed Astral module does not match the running kernel.",
        }

    if not report["module_loaded"]:
        return {
            "action": "repair_startup",
            "reason": "Compatible module exists but is not loaded.",
        }

    if not report["sensor_ready"]:
        return {
            "action": "diagnose_sensor",
            "reason": "Module is loaded but required sensor channels are unavailable.",
        }

    return {
        "action": "repair_services",
        "reason": "Compatible driver and sensors exist; service configuration needs checking.",
    }


def isolated_build(report):
    if os.geteuid() == 0:
        raise RuntimeError("Refusing to build as root")

    if report["protection_appears_operational"]:
        print("SKIP: Astral protection appears operational.")
        print("No rebuild is necessary.")
        return

    if not report["build_prerequisites_available"]:
        raise RuntimeError("Kernel build prerequisites are missing")

    kernel = report["kernel"]

    with tempfile.TemporaryDirectory(prefix="ugc-astral-build-") as tmp:
        work = Path(tmp)
        for name in (
            "Kbuild",
            "astral_hwmon.c",
            "astral_detect.c",
            "astral_regs.c",
            "astral.h",
        ):
            shutil.copy2(SOURCE / name, work / name)

        command = [
            "make",
            "-C", report["build_directory"],
            f"M={work}",
            "modules",
        ]

        result = run(command)
        if result.returncode != 0:
            raise RuntimeError(
                "Kernel module build failed:\n"
                + result.stdout[-4000:]
                + result.stderr[-4000:]
            )

        candidate = work / "astral-hwmon.ko"
        if not candidate.is_file():
            raise RuntimeError("Build completed without producing the module")

        metadata = run(["modinfo", "-F", "vermagic", str(candidate)])
        if metadata.returncode != 0:
            raise RuntimeError("Cannot verify rebuilt module")

        built_kernel = metadata.stdout.split()[0]

        if built_kernel != kernel:
            raise RuntimeError(
                f"Kernel mismatch: built={built_kernel}, running={kernel}"
            )

        print("PASS: Replacement module compiled successfully.")
        print(f"PASS: Module matches running kernel {kernel}.")
        print("No module was installed, loaded, unloaded or replaced.")
        print("Temporary build files will be removed.")


def prepare_recovery(report):
    """Build and stage a candidate; never modify installed protection."""
    import hashlib

    if os.geteuid() == 0:
        raise RuntimeError("Refusing to prepare recovery as root")

    if report["protection_appears_operational"]:
        print("SKIP: Astral protection is already operational.")
        return

    decision = recovery_decision(report)

    if decision["action"] != "rebuild_module":
        print("SKIP:", decision["reason"])
        print("A kernel rebuild is not indicated.")
        return

    if not report["build_prerequisites_available"]:
        raise RuntimeError("Kernel build prerequisites unavailable")

    package = Path(__file__).resolve().parent.parent / "release/astral-v1.8/kernel-source"
    manifest = package / "SOURCE-SHA256.json"

    data = json.loads(manifest.read_text())

    if data.get("format") != "ugc-astral-kernel-source-v1":
        raise RuntimeError("Unsupported source manifest")

    names = (
        "Kbuild",
        "astral_hwmon.c",
        "astral_detect.c",
        "astral_regs.c",
        "astral.h",
    )

    if set(data.get("files", {})) != set(names):
        raise RuntimeError("Unexpected source inventory")

    with tempfile.TemporaryDirectory(prefix="ugc-astral-recovery-") as tmp:
        work = Path(tmp)

        for name in names:
            content = (package / name).read_bytes()
            digest = hashlib.sha256(content).hexdigest()

            if digest != data["files"][name]:
                raise RuntimeError(f"Source checksum mismatch: {name}")

            (work / name).write_bytes(content)

        result = run([
            "make", "-C", report["build_directory"],
            f"M={work}", "modules"
        ])

        if result.returncode != 0:
            raise RuntimeError(
                "Recovery build failed:\\n"
                + result.stdout[-4000:]
                + result.stderr[-4000:]
            )

        candidate = work / "astral-hwmon.ko"

        if not candidate.is_file():
            raise RuntimeError("Recovery module not produced")

        metadata = run(["modinfo", "-F", "vermagic", str(candidate)])

        if metadata.returncode != 0:
            raise RuntimeError("Cannot verify candidate module")

        if metadata.stdout.split()[0] != report["kernel"]:
            raise RuntimeError("Candidate module kernel mismatch")

        stage = Path.home() / ".local/share/ugc/astral-recovery"
        stage.mkdir(parents=True, exist_ok=True, mode=0o700)
        stage.chmod(0o700)

        output = stage / "astral-hwmon.ko"
        temporary = stage / "astral-hwmon.ko.tmp"

        shutil.copy2(candidate, temporary)
        temporary.replace(output)

        info = {
            "kernel": report["kernel"],
            "module_sha256": hashlib.sha256(output.read_bytes()).hexdigest(),
            "source_manifest": hashlib.sha256(manifest.read_bytes()).hexdigest(),
            "status": "prepared_not_installed",
        }

        (stage / "recovery.json").write_text(
            json.dumps(info, indent=2) + "\\n"
        )

        print("PASS: Recovery module prepared.")
        print("PASS: Kernel compatibility verified.")
        print("PASS: Source checksums verified.")
        print("STAGED:", output)
        print("No installed driver or service modified.")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "action", choices=("check", "build", "prepare")
    )
    args = parser.parse_args()

    report = inspect()

    if args.action == "check":
        report["recovery_decision"] = recovery_decision(report)
        print(json.dumps(report, indent=2))
    elif args.action == "prepare":
        prepare_recovery(report)
    else:
        decision = recovery_decision(report)
        if decision["action"] == "rebuild_module":
            isolated_build(report)
        else:
            print("Recovery decision:", decision["action"])
            print(decision["reason"])
            print("No system changes performed.")

if __name__ == "__main__":
    main()
