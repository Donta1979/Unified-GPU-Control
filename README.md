# Unified GPU Control v1.8

Unified GPU Control (UGC) is a Linux GPU management interface built on LACT.

The goal is to combine useful GPU controls, monitoring configuration,
and supported hardware safety integrations in one application.

## Features

- GPU controls through LACT
- MangoHud configuration
- ASUS ROG Astral monitoring and guard integration
- Settings export and import
- Flatpak packaging

## Installation

Download and extract the Linux release archive.

Open a terminal in the extracted folder and run:

    ./install.sh

Follow the on-screen instructions.

## Requirements

- Linux x86_64
- Flatpak
- LACT daemon for GPU controls
- MangoHud for supported monitoring features

Astral Guard is optional, requires compatible hardware,
and must be configured separately.

## Project Status

UGC v1.8 is currently a release candidate.

Installation detection and identical-version handling
have been tested on Bazzite.

Fresh installations, genuine upgrades, and missing-dependency
setup still require validation.

## Contributing

Bug reports, improvements, and pull requests are welcome.

## Credits

Built upon the open-source LACT project, with additional
interface and integration functionality.

## Open Source and Attribution

Unified GPU Control is based on LACT, originally developed
by Ilya Zlobintsev.

Original LACT project:
https://github.com/ilya-zlobintsev/LACT

UGC is an independent derivative project and is not
an official LACT release.

The original MIT license and copyright notice are
preserved in the LICENSE file.

The original LACT README is preserved at:
docs/LACT-Original-README.md

## Hardware Safety

ASUS ROG Astral monitoring and guard integration
requires compatible hardware and separately installed
supporting components.

Unified GPU Control does not replace the Astral Guard
emergency shutdown service.

Always verify that hardware protection services
are functioning independently of the GUI.
