#!/usr/bin/env python3
"""Static integration sanity checks; NOT a Rust compile or runtime test."""
from pathlib import Path
import re
root = Path(__file__).resolve().parents[1]
p = (root / "lact-gui/src/app/pages/mango_osd_page.rs").read_text()
a = (root / "lact-gui/src/app/pages/astral_page.rs").read_text()
app = (root / "lact-gui/src/app.rs").read_text()
checks = {
 "MangoJuice integrated page": "make_mango_osd_page()" in app,
 "Astral integrated page": "make_astral_page()" in app,
 "X/Y spin inputs": 'let x_spin = gtk::SpinButton' in p and 'let y_spin = gtk::SpinButton' in p,
 "Manual resolution fields": 'let width_spin = gtk::SpinButton' in p and 'let height_spin = gtk::SpinButton' in p,
 "Preset selection is dimension based": 'RESOLUTIONS.iter().position' in p,
 "Manual override selects Custom": 'dropdown.set_selected(0)' in p,
 "Preset guard prevents reentrancy": 'guard.set(true)' in p and 'guard.set(false)' in p,
 "No MangoHud config writes in preview": not any(t in p for t in ['fs::write(', 'File::create(', 'OpenOptions::new(']),
 "Astral page does not issue shutdown": not any(t in a for t in ['poweroff(', 'shutdown(', 'systemctl poweroff']),
 "Source uses no external process for OSD": 'Command::new(' not in p,
}
for k, v in checks.items(): print(('PASS' if v else 'FAIL') + ': ' + k)
print(f"{sum(checks.values())}/{len(checks)} static checks passed")
raise SystemExit(0 if all(checks.values()) else 1)
