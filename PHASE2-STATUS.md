# UGC Phase 2 — development snapshot (NOT a finished installer)

This is a **source-only, uncompiled development snapshot**. It has not passed a Fedora 45 build or live GPU tests. Do not replace the installed LACT/UGC executable or Astral Guard service with it yet.

Implemented in this snapshot:
- Window title branding: UGC — Universal GPU Control.
- Astral Guard navigation is shown only when the `astral12vhpwr` sensor is present. Its monitoring remains read-only; a button opens the separately installed Astral Guard Manager.
- MangoHud metric toggles, explicit Save and Reload actions, user configuration file backups, and preservation of unrelated config lines.

Still to implement and validate before a release:
- Full Astral Guard Manager integration with safe privileged operations and supported ASUS hardware detection (including Matrix support validation).
- Full MangoHud configuration coverage, appearance and positioning Apply, profile management, and robust per-game/Flatpak support.
- A distribution-aware installer, package dependency management, icons/desktop integration, uninstall/upgrade support.
- Real branding in every window/resource, compile verification, runtime tests and safety review.

**Important:** Existing system protection is not modified by these source changes. MangoHud settings are changed only if the user presses Save in the new UI. The existing air-raid `siren.wav` is not replaced or packaged; no obsolete siren backup is included.

To validate this source on GitHub, upload its contents to a test branch and run the existing manual Fedora 45 build workflow. Do not deploy over your working installation until that workflow passes and behavior is verified.
