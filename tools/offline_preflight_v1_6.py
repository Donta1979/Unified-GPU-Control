#!/usr/bin/env python3
"""Static-only integration preflight. NOT a Rust compilation or runtime test."""
from pathlib import Path
root = Path(__file__).resolve().parents[1]
app = (root / 'lact-gui/src/app.rs').read_text()
mod = (root / 'lact-gui/src/app/pages/mod.rs').read_text()
mango = (root / 'lact-gui/src/app/pages/mango_osd_page.rs').read_text()
astral = (root / 'lact-gui/src/app/pages/astral_page.rs').read_text()
checks = {
 'Astral page registered': 'pub mod astral_page;' in mod and 'make_astral_page().upcast()' in app,
 'Mango page registered': 'pub mod mango_osd_page;' in mod and 'make_mango_osd_page().upcast()' in app,
 'Sidebar IDs registered': 'Self::Astral =>' in mod and 'Self::MangoOsd =>' in mod,
 'Read-only Mango preview': 'fs::read_to_string' in mango and 'fs::write' not in mango and 'OpenOptions' not in mango,
 'Manual resolution': 'width_spin' in mango and 'height_spin' in mango,
 'Exact X/Y controls': 'x_spin' in mango and 'y_spin' in mango,
 'No protection unit modifications': 'systemctl enable' not in astral and 'systemctl disable' not in astral,
 'Mango config comment parsing': 'value.split('#')' in mango,
}
for name, passed in checks.items(): print(('PASS' if passed else 'FAIL') + ': ' + name)
print('STATIC ONLY: compilation and runtime behavior remain unverified')
raise SystemExit(0 if all(checks.values()) else 1)
