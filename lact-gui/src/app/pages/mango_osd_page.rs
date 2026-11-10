//! Read-only MangoHud configuration overview. Never writes files or launches other apps.
use adw::prelude::*;
use gtk::{self, glib};
use std::{env, fs, path::{Path, PathBuf}};

fn config_candidates() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Ok(xdg) = env::var("XDG_CONFIG_HOME") {
        if !xdg.is_empty() { paths.push(PathBuf::from(xdg).join("MangoHud/MangoHud.conf")); }
    }
    if let Ok(home) = env::var("HOME") {
        paths.push(PathBuf::from(&home).join(".config/MangoHud/MangoHud.conf"));
        paths.push(PathBuf::from(&home).join(".config/MangoHud.conf"));
    }
    paths.push(PathBuf::from("/etc/MangoHud.conf"));
    paths
}

fn read_config() -> Option<(PathBuf, String)> {
    for path in config_candidates() {
        if let Ok(data) = fs::read_to_string(&path) { return Some((path, data)); }
    }
    None
}

fn config_value<'a>(content: &'a str, key: &str) -> Option<&'a str> {
    // Last uncommented occurrence wins. This is a simple preview, not a full MangoHud parser.
    content.lines().filter_map(|line| {
        let line = line.trim();
        if line.starts_with('#') || line.starts_with(';') || line.is_empty() { return None; }
        let (name, value) = line.split_once('=')?;
        // Ignore trailing comments in ordinary key=value entries.
        let value = value.split('#').next().unwrap_or("").trim();
        if name.trim() == key { Some(value) } else { None }
    }).last()
}

fn metric_enabled(content: &str, key: &str) -> bool {
    let mut result = false;
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('#') || line.starts_with(';') { continue; }
        if line == key { result = true; }
        if let Some((name, value)) = line.split_once('=') {
            if name.trim() == key {
                let value = value.split('#').next().unwrap_or("").trim();
                result = !matches!(value, "0" | "false" | "off" | "False" | "OFF");
            }
        }
    }
    result
}

fn row(grid: &gtk::Grid, index: i32, name: &str) -> gtk::Label {
    let title = gtk::Label::new(Some(name));
    title.set_xalign(0.0);
    let value = gtk::Label::new(Some("Not loaded"));
    value.set_xalign(0.0);
    value.set_wrap(true);
    grid.attach(&title, 0, index, 1, 1);
    grid.attach(&value, 1, index, 1, 1);
    value
}

pub fn make_mango_osd_page() -> gtk::Box {
    let root = gtk::Box::new(gtk::Orientation::Vertical, 14);
    root.set_margin_top(20);
    root.set_margin_bottom(20);
    root.set_margin_start(20);
    root.set_margin_end(20);
    let title = gtk::Label::new(Some("MangoJuice OSD"));
    title.add_css_class("title-2");
    title.set_xalign(0.0);
    root.append(&title);
    let note = gtk::Label::new(Some("Integrated read-only preview of your MangoHud settings. No external windows, file changes, or GPU controls."));
    note.set_wrap(true); note.set_xalign(0.0); root.append(&note);
    let source = gtk::Label::new(Some("Checking configuration…"));
    source.set_xalign(0.0); source.set_wrap(true); root.append(&source);
    let heading = gtk::Label::new(Some("Overlay placement"));
    heading.add_css_class("title-3"); heading.set_xalign(0.0); root.append(&heading);
    let grid = gtk::Grid::new(); grid.set_column_spacing(24); grid.set_row_spacing(10);
    let position = row(&grid, 0, "Position preset");
    let offset_x = row(&grid, 1, "Horizontal offset");
    let offset_y = row(&grid, 2, "Vertical offset");
    let alpha = row(&grid, 3, "Background opacity");
    root.append(&grid);
    // v1.0: top-left-origin positioning sandbox, strictly preview-only.
    // Slider bounds follow a user-selected viewport size; exact coordinate entry
    // accepts values outside those bounds. Nothing is written to MangoHud.
    let preview_title = gtk::Label::new(Some("Top-left OSD positioning (preview only)"));
    preview_title.add_css_class("title-3");
    preview_title.set_xalign(0.0);
    root.append(&preview_title);
    let preview_help = gtk::Label::new(Some("X=0, Y=0 starts at the upper-left. Increasing X moves right; increasing Y moves down. Exact inputs can extend the slider range beyond the viewport (up to 1,000,000 px). This is only a preview and does not change MangoHud."));
    preview_help.set_wrap(true);
    preview_help.set_xalign(0.0);
    root.append(&preview_help);

    let dimensions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    dimensions.append(&gtk::Label::new(Some("Preview resolution:")));
    let width_spin = gtk::SpinButton::with_range(320.0, 16384.0, 1.0);
    width_spin.set_value(3440.0);
    let height_spin = gtk::SpinButton::with_range(240.0, 8640.0, 1.0);
    height_spin.set_value(1440.0);
    // Broad convenience presets: handhelds, legacy PC, 16:9, 16:10,
    // ultrawide, super-ultrawide and high-resolution desktop displays.
    // These are not an exhaustive list of monitor modes. Custom entry always works.
    const RESOLUTIONS: &[(&str, u32, u32)] = &[
        ("Custom / manual", 0, 0),
        ("Handheld · 640 × 480", 640, 480),
        ("Handheld · 800 × 480", 800, 480),
        ("Handheld · 960 × 544", 960, 544),
        ("Handheld · 1024 × 600", 1024, 600),
        ("Handheld · 1280 × 720", 1280, 720),
        ("Handheld · 1280 × 800", 1280, 800),
        ("Handheld · 1600 × 900", 1600, 900),
        ("Handheld · 1920 × 1080", 1920, 1080),
        ("Handheld · 1920 × 1200", 1920, 1200),
        ("Legacy PC · 800 × 600", 800, 600),
        ("Legacy PC · 1024 × 768", 1024, 768),
        ("Legacy PC · 1152 × 864", 1152, 864),
        ("Legacy PC · 1280 × 1024", 1280, 1024),
        ("Legacy PC · 1360 × 768", 1360, 768),
        ("Legacy PC · 1366 × 768", 1366, 768),
        ("Standard · 1600 × 900", 1600, 900),
        ("Standard · 1920 × 1080", 1920, 1080),
        ("Standard · 2048 × 1152", 2048, 1152),
        ("Standard · 2560 × 1440", 2560, 1440),
        ("Standard · 3200 × 1800", 3200, 1800),
        ("Standard · 3840 × 2160", 3840, 2160),
        ("16:10 · 1280 × 800", 1280, 800),
        ("16:10 · 1440 × 900", 1440, 900),
        ("16:10 · 1680 × 1050", 1680, 1050),
        ("16:10 · 1920 × 1200", 1920, 1200),
        ("16:10 · 2560 × 1600", 2560, 1600),
        ("16:10 · 2880 × 1800", 2880, 1800),
        ("16:10 · 3200 × 2000", 3200, 2000),
        ("16:10 · 3840 × 2400", 3840, 2400),
        ("Ultrawide · 2560 × 1080", 2560, 1080),
        ("Ultrawide · 3440 × 1440", 3440, 1440),
        ("Ultrawide · 3840 × 1600", 3840, 1600),
        ("Ultrawide · 5120 × 2160", 5120, 2160),
        ("Super ultrawide · 3840 × 1080", 3840, 1080),
        ("Super ultrawide · 5120 × 1440", 5120, 1440),
        ("Super ultrawide · 7680 × 2160", 7680, 2160),
        ("High resolution · 4096 × 2160", 4096, 2160),
        ("High resolution · 5120 × 2880", 5120, 2880),
        ("High resolution · 6016 × 3384", 6016, 3384),
        ("High resolution · 7680 × 4320", 7680, 4320),
    ];
    let preset_names: Vec<&str> = RESOLUTIONS.iter().map(|(name, _, _)| *name).collect();
    let resolution_presets = gtk::DropDown::from_strings(&preset_names);
    // The preview defaults to 3440x1440, but presets are always optional.
    // Derive the preset index from its dimensions rather than relying on list order.
    let default_preset = RESOLUTIONS.iter().position(|(_, w, h)| *w == 3440 && *h == 1440).unwrap_or(0);
    resolution_presets.set_selected(default_preset as u32);
    dimensions.append(&gtk::Label::new(Some("Preset:")));
    dimensions.append(&resolution_presets);
    dimensions.append(&width_spin);
    dimensions.append(&gtk::Label::new(Some("×")));
    dimensions.append(&height_spin);
    root.append(&dimensions);
    let resolution_note = gtk::Label::new(Some("Choose a handheld, standard, ultrawide or high-resolution preset, or enter any width and height manually. Presets do not restrict your coordinates. Display detection is not yet implemented."));
    resolution_note.set_wrap(true);
    resolution_note.set_xalign(0.0);
    root.append(&resolution_note);
    // Manual dimensions switch the dropdown to Custom so the UI never claims
    // that a preset is active when the entered resolution differs.
    // The guard prevents preset selection from immediately resetting to Custom.
    let applying_preset = std::rc::Rc::new(std::cell::Cell::new(false));
    {
        let guard = applying_preset.clone();
        let width_spin = width_spin.clone();
        let height_spin = height_spin.clone();
        let dropdown = resolution_presets.clone();
        dropdown.connect_selected_notify(move |dropdown| {
            if let Some((_, w, h)) = RESOLUTIONS.get(dropdown.selected() as usize) {
                if *w > 0 && *h > 0 {
                    guard.set(true);
                    width_spin.set_value(*w as f64);
                    height_spin.set_value(*h as f64);
                    guard.set(false);
                }
            }
        });
    }
    for dimension in [&width_spin, &height_spin] {
        let guard = applying_preset.clone();
        let dropdown = resolution_presets.clone();
        dimension.connect_value_changed(move |_| {
            if !guard.get() && dropdown.selected() != 0 {
                dropdown.set_selected(0);
            }
        });
    }

    let x_adjustment = gtk::Adjustment::new(0.0, 0.0, 3440.0, 1.0, 10.0, 0.0);
    let y_adjustment = gtk::Adjustment::new(0.0, 0.0, 1440.0, 1.0, 10.0, 0.0);
    let controls = gtk::Grid::new();
    controls.set_column_spacing(12);
    controls.set_row_spacing(8);
    let x_slider = gtk::Scale::new(gtk::Orientation::Horizontal, Some(&x_adjustment));
    let y_slider = gtk::Scale::new(gtk::Orientation::Horizontal, Some(&y_adjustment));
    x_slider.set_hexpand(true);
    y_slider.set_hexpand(true);
    x_slider.set_draw_value(false);
    y_slider.set_draw_value(false);
    // Exact entries can extend the slider range beyond the viewport when necessary.
    let x_spin = gtk::SpinButton::with_range(0.0, 1000000.0, 1.0);
    let y_spin = gtk::SpinButton::with_range(0.0, 1000000.0, 1.0);
    x_spin.set_numeric(true);
    y_spin.set_numeric(true);
    controls.attach(&gtk::Label::new(Some("X (px)")), 0, 0, 1, 1);
    controls.attach(&x_slider, 1, 0, 1, 1);
    controls.attach(&x_spin, 2, 0, 1, 1);
    controls.attach(&gtk::Label::new(Some("Y (px)")), 0, 1, 1, 1);
    controls.attach(&y_slider, 1, 1, 1, 1);
    controls.attach(&y_spin, 2, 1, 1, 1);
    root.append(&controls);

    let frame = gtk::Frame::new(Some("Aspect-correct viewport preview (not actual game rendering)"));
    let canvas = gtk::DrawingArea::new();
    canvas.set_content_width(620);
    canvas.set_content_height(260);
    canvas.set_hexpand(true);
    canvas.set_draw_func({
        let x_spin = x_spin.clone();
        let y_spin = y_spin.clone();
        let width_spin = width_spin.clone();
        let height_spin = height_spin.clone();
        move |_, cr, w, h| {
            let w = w as f64;
            let h = h as f64;
            cr.set_source_rgb(0.075, 0.095, 0.12);
            let _ = cr.paint();
            cr.set_source_rgb(0.25, 0.34, 0.40);
            cr.set_line_width(1.0);
            cr.move_to(0.0, 0.0);
            cr.line_to(w, 0.0);
            cr.move_to(0.0, 0.0);
            cr.line_to(0.0, h);
            let _ = cr.stroke();
            let vw = width_spin.value().max(1.0);
            let vh = height_spin.value().max(1.0);
            // Fit the actual selected aspect ratio inside the preview area.
            // The older prototype stretched every resolution into 620x260,
            // which misrepresented handheld and portrait-ish viewports.
            let scale = (w / vw).min(h / vh);
            let screen_w = vw * scale;
            let screen_h = vh * scale;
            let origin_x = (w - screen_w) / 2.0;
            let origin_y = (h - screen_h) / 2.0;
            cr.set_source_rgb(0.12, 0.16, 0.20);
            cr.rectangle(origin_x, origin_y, screen_w, screen_h);
            let _ = cr.fill();
            cr.set_source_rgb(0.32, 0.42, 0.50);
            cr.rectangle(origin_x + 0.5, origin_y + 0.5, screen_w - 1.0, screen_h - 1.0);
            let _ = cr.stroke();
            // Clip the illustrative OSD to the selected screen, not the
            // entire widget; out-of-bounds coordinates stay unchanged.
            let _ = cr.save();
            cr.rectangle(origin_x, origin_y, screen_w, screen_h);
            cr.clip();
            let px = origin_x + x_spin.value() * scale;
            let py = origin_y + y_spin.value() * scale;
            cr.set_source_rgb(0.20, 0.68, 0.49);
            cr.rectangle(px, py, 92.0, 34.0);
            let _ = cr.fill();
            cr.set_source_rgb(1.0, 1.0, 1.0);
            cr.move_to(px + 6.0, py + 20.0);
            let _ = cr.show_text("GPU | FPS");
            let _ = cr.restore();
        }
    });
    frame.set_child(Some(&canvas));
    root.append(&frame);

    // Bidirectional slider / exact-value synchronization without recursive updates.
    {
        let x_spin = x_spin.clone();
        x_slider.connect_value_changed(move |slider| x_spin.set_value(slider.value()));
    }
    {
        let y_spin = y_spin.clone();
        y_slider.connect_value_changed(move |slider| y_spin.set_value(slider.value()));
    }
    {
        let x_slider = x_slider.clone();
        let canvas = canvas.clone();
        x_spin.connect_value_changed(move |spin| {
            // Keep the slider and exact input synchronized, even beyond the viewport.
            // GTK Scale clamps values to its adjustment upper bound.
            let upper = x_slider.adjustment().upper();
            if spin.value() > upper { x_slider.adjustment().set_upper(spin.value()); }
            x_slider.set_value(spin.value());
            canvas.queue_draw();
        });
    }
    {
        let y_slider = y_slider.clone();
        let canvas = canvas.clone();
        y_spin.connect_value_changed(move |spin| {
            let upper = y_slider.adjustment().upper();
            if spin.value() > upper { y_slider.adjustment().set_upper(spin.value()); }
            y_slider.set_value(spin.value());
            canvas.queue_draw();
        });
    }
    for (dimension, slider, spin) in [
        (&width_spin, &x_slider, &x_spin),
        (&height_spin, &y_slider, &y_spin),
    ] {
        let slider = slider.clone();
        let spin = spin.clone();
        let canvas = canvas.clone();
        dimension.connect_value_changed(move |dimension| {
            let extent = dimension.value();
            // Keep the full entered coordinate representable after resolution changes.
            slider.adjustment().set_upper(extent.max(spin.value()));
            slider.set_value(spin.value());
            canvas.queue_draw();
        });
    }
    let reset = gtk::Button::with_label("Upper-left (X=0, Y=0)");
    {
        let x_spin = x_spin.clone();
        let y_spin = y_spin.clone();
        reset.connect_clicked(move |_| { x_spin.set_value(0.0); y_spin.set_value(0.0); });
    }
    root.append(&reset);

    let metrics_title = gtk::Label::new(Some("Metrics"));
    metrics_title.add_css_class("title-3"); metrics_title.set_xalign(0.0); root.append(&metrics_title);
    let metrics = gtk::Grid::new(); metrics.set_column_spacing(24); metrics.set_row_spacing(8);
    let keys = ["fps", "frame_timing", "gpu_stats", "gpu_temp", "cpu_stats", "cpu_temp", "vram", "ram"];
    let labels: Vec<gtk::Label> = keys.iter().enumerate().map(|(i, key)| row(&metrics, i as i32, key)).collect();
    root.append(&metrics);
    let caveat = gtk::Label::new(Some("This preview reads only the first accessible global configuration file. MangoHud can also use per-game configs, environment overrides, and Flatpak-specific settings. The editor is preview-only. Actual MangoHud placement is constrained by its renderer and must be investigated before enabling Apply."));
    caveat.set_xalign(0.0); caveat.set_wrap(true); caveat.add_css_class("dim-label"); root.append(&caveat);
    let refresh = move || {
        if let Some((path, content)) = read_config() {
            source.set_text(&format!("Global configuration: {}", path.display()));
            position.set_text(config_value(&content, "position").unwrap_or("default (not explicitly set)"));
            offset_x.set_text(config_value(&content, "offset_x").unwrap_or("default (not explicitly set)"));
            offset_y.set_text(config_value(&content, "offset_y").unwrap_or("default (not explicitly set)"));
            alpha.set_text(config_value(&content, "background_alpha").unwrap_or("default (not explicitly set)"));
            for (key, label) in keys.iter().zip(labels.iter()) {
                label.set_text(if metric_enabled(&content, key) { "Enabled" } else { "Not explicitly enabled in this file" });
            }
        } else {
            source.set_text("No readable global MangoHud configuration found. The game may use defaults, a per-game file, or Flatpak settings.");
            for label in [&position, &offset_x, &offset_y, &alpha] { label.set_text("Not found"); }
            for label in &labels { label.set_text("Unknown"); }
        }
        glib::ControlFlow::Continue
    };
    refresh();
    glib::timeout_add_seconds_local(10, refresh);
    root
}
