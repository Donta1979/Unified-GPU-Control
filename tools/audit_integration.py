#!/usr/bin/env python3
"""Offline structural checks. This does NOT compile or runtime-test the GUI."""
from pathlib import Path
root = Path(__file__).resolve().parents[1]
app = (root/'lact-gui/src/app.rs').read_text()
pages = (root/'lact-gui/src/app/pages/mod.rs').read_text()
astral = (root/'lact-gui/src/app/pages/astral_page.rs').read_text()
checks = {
  'LACT workspace manifest present': (root/'Cargo.toml').is_file(),
  'Astral page declared': 'pub mod astral_page;' in pages,
  'Astral page registered in sidebar': 'PageId::Astral' in app and 'make_astral_page().upcast()' in app,
  'Six pin cards created': 'for i in 0..6' in astral,
  'Two-second sensor polling': 'timeout_add_seconds_local(2, refresh_pins)' in astral,
  'Twenty-second configuration polling': 'timeout_add_seconds_local(20, refresh_checks)' in astral,
  'Sensor read-only': 'fs::write(' not in astral and 'Command::new("sudo")' not in astral,
  'No protection service changes': 'systemctl").args(["enable"' not in astral and 'systemctl").args(["restart"' not in astral,
  'Oneshot startup handled': 'ENABLED (NOT TESTED)' in astral,
  'Power estimate requires all channels': 'if complete {' in astral and 'total_watts' in astral and 'connector_power.set_text' in astral,
  'Explicitly not ASUS mileage': 'not ASUS Cable Mileage' in astral,
  'No missing channel falsely OK': 'ma.is_finite() && ma >= 0.0' in astral and 'mv.is_finite() && mv > 0.0' in astral,
}
for name, ok in checks.items(): print(('PASS' if ok else 'FAIL') + '  ' + name)
raise SystemExit(0 if all(checks.values()) else 1)
