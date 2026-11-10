#!/usr/bin/env python3
from pathlib import Path
import re
root = Path(__file__).resolve().parents[1]
s = (root/'lact-gui/src/app/pages/mango_osd_page.rs').read_text()
a = (root/'lact-gui/src/app/pages/astral_page.rs').read_text()
checks = {
    'aspect ratio uses selected viewport': 'let scale = (w / vw).min(h / vh);' in s,
    'letterbox offsets computed': 'let origin_x = (w - screen_w) / 2.0;' in s and 'let origin_y = (h - screen_h) / 2.0;' in s,
    'preview clipping': 'cr.clip();' in s and 'cr.restore();' in s,
    'no MangoHud config write': not re.search(r'fs::write|File::create|OpenOptions|set_var\(', s),
    'read-only Astral monitoring': not re.search(r'fs::write|File::create|OpenOptions|systemctl.*start|systemctl.*stop', a),
    'manual resolution fields retained': 'let width_spin = gtk::SpinButton::with_range' in s and 'let height_spin = gtk::SpinButton::with_range' in s,
    'pixel coordinate fields retained': 'let x_spin = gtk::SpinButton::with_range' in s and 'let y_spin = gtk::SpinButton::with_range' in s,
}
for label, ok in checks.items(): print(('PASS' if ok else 'FAIL') + ': ' + label)
if not all(checks.values()): raise SystemExit(1)
print('Source checks only — NOT a Rust build or runtime test.')
