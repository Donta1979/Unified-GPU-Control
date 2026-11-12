# Unified GPU Control v1.8 — Release Candidate

Unified GPU Control (UGC) brings GPU management, MangoHud configuration, and supported ASUS ROG Astral power connector monitoring into one Linux graphical application.

Built on the open-source LACT project, UGC aims to make powerful GPU controls and hardware monitoring more accessible without requiring users to manage everything through terminal commands.

## What's Included

### LACT GPU Management
- GPU hardware and software information.
- Historical performance monitoring.
- Supported GPU power, clock, voltage, and fan controls.
- Voltage/frequency curve editing on compatible hardware.

### MangoHud
- Graphical configuration for supported MangoHud overlay options.
- Convenient management of in-game performance monitoring preferences.

### ASUS ROG Astral Guard / Sense Pin
- Per-pin current and voltage monitoring on compatible ASUS ROG Astral graphics cards.
- Driver, sensor, and protection service status.
- Integration with independent Astral Guard emergency protection.

The independent Astral Guard emergency shutdown mechanism must remain operational regardless of whether UGC is open.

## Additional Features
- UGC desktop launcher and custom application branding.
- Settings export and import.
- Flatpak packaging for Linux x86_64.

## Installation

Download and extract `Unified-GPU-Control-v1.8-Linux-x86_64.tar.gz`.

Open a terminal inside the extracted directory and run:

```bash
./install.sh
```

Follow the installer prompts.

The installer checks for existing installations and supporting components. Administrator authorization may be requested for optional hardware-related setup.

## Compatibility

Developed and tested primarily on Bazzite Linux with KDE.

GPU controls depend on hardware and driver support. MangoHud requires its corresponding software. Astral Guard integration requires compatible ASUS ROG Astral hardware and supporting services.

## Project Background

I previously worked as an AAA game-development environment artist and 3D modeler. While working with Autodesk Maya, I used MEL and some Python to create tools that streamlined repetitive work.

After moving to Linux and Bazzite, I encountered problems keeping track of my ASUS ROG Astral Guard protection following system updates. I wanted an easier way to confirm that the hardware monitoring driver and emergency shutdown services were working without repeatedly using terminal commands.

That led to an Astral Guard graphical interface, followed by the idea of bringing LACT, MangoHud, and Astral Guard together into a single application.

UGC is the result of that effort. My goal is to make GPU management and monitoring more approachable for Linux users.

## Current Validation Status

- Bazzite installation detection and identical-version handling tested.
- Installer shell syntax validated.
- Astral provisioning Python syntax validated.
- Signed Astral release manifest verified.
- Existing Astral monitoring and timer services confirmed operational on the development system.

The following still require validation:
- Clean-system installation.
- Upgrade from a genuinely older UGC version.
- Missing-dependency installation paths.
- Complete GitHub Actions Flatpak build and release archive generation.
- Astral installation and kernel-recovery behavior on compatible test systems.

This is a release candidate, not a fully validated stable release.

## Credits and Licensing

UGC is an independent derivative of LACT, originally developed by Ilya Zlobintsev.

- LACT: https://github.com/ilya-zlobintsev/LACT
- MangoHud: https://github.com/flightlessmango/MangoHud

Astral Guard and hardware-monitoring components retain their respective upstream authorship and licenses.

The original LACT MIT license and copyright notice are preserved in `LICENSE`. The original LACT README is retained in `docs/LACT-Original-README.md`.

UGC is not an official LACT release.

## Hardware Safety

GPU overclocking, power adjustments, and voltage changes may affect system stability.

UGC does not replace independent hardware protection mechanisms. ASUS ROG Astral users must verify that Astral Guard's emergency shutdown service is installed, enabled, and functioning independently of the GUI.
