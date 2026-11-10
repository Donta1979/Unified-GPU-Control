//! Read-only Astral connector dashboard. No privileged actions or configuration writes.
use adw::prelude::*;
use gtk::{self, glib};
use std::{fs, path::{Path, PathBuf}, process::Command};

const PIN_WARN_MA: f64 = 9200.0;
const PIN_CRIT_MA: f64 = 9500.0;
const VOLT_WARN_MV: f64 = 11400.0;
const VOLT_CRIT_MV: f64 = 11000.0;

fn read_number(path: &Path) -> Option<f64> {
    fs::read_to_string(path).ok()?.trim().parse::<f64>().ok()
}
fn sensor_dir() -> Option<PathBuf> {
    for entry in fs::read_dir("/sys/class/hwmon").ok()?.flatten() {
        let path = entry.path();
        if fs::read_to_string(path.join("name")).ok()?.trim() == "astral12vhpwr" {
            return Some(path);
        }
    }
    None
}
fn status(label: &gtk::Label, text: &str, color: &str) {
    for old in ["success", "warning", "error", "dim-label"] { label.remove_css_class(old); }
    label.set_text(text);
    label.add_css_class(color);
}
fn unit_active(name: &str) -> bool {
    Command::new("systemctl").args(["is-active", "--quiet", name]).status().is_ok_and(|s| s.success())
}
fn unit_enabled(name: &str) -> bool {
    Command::new("systemctl").args(["is-enabled", "--quiet", name]).status().is_ok_and(|s| s.success())
}
fn make_row(title: &str, grid: &gtk::Grid, idx: i32) -> gtk::Label {
    let name = gtk::Label::new(Some(title));
    name.set_xalign(0.0);
    let result = gtk::Label::new(Some("Not checked"));
    result.set_xalign(0.0);
    result.add_css_class("dim-label");
    grid.attach(&name, 0, idx, 1, 1);
    grid.attach(&result, 1, idx, 1, 1);
    result
}
pub fn make_astral_page() -> gtk::Box {
    let root = gtk::Box::new(gtk::Orientation::Vertical, 14);
    root.set_margin_top(20);
    root.set_margin_bottom(20);
    root.set_margin_start(20);
    root.set_margin_end(20);
    let title = gtk::Label::new(Some("Astral Guard — Sense Pins"));
    title.add_css_class("title-2");
    title.set_xalign(0.0);
    root.append(&title);
    let note = gtk::Label::new(Some("Read-only view. The separate Astral Guard service is responsible for protection and emergency shutdown. This page never changes its settings."));
    note.set_wrap(true);
    note.set_xalign(0.0);
    root.append(&note);
    let manage = gtk::Button::with_label("Open Astral Guard Manager — Safety Settings");
    manage.set_halign(gtk::Align::Start);
    let manager_status = gtk::Label::new(Some("Safety settings remain in the existing independent manager."));
    manager_status.set_xalign(0.0);
    manager_status.set_wrap(true);
    {
        let manager_status = manager_status.clone();
        manage.connect_clicked(move |_| {
            match Command::new("astral-guard-manager").spawn() {
                Ok(_) => manager_status.set_text("Astral Guard Manager launched."),
                Err(err) => manager_status.set_text(&format!("Could not launch Astral Guard Manager: {err}")),
            }
        });
    }
    root.append(&manage);
    root.append(&manager_status);
    let overall = gtk::Label::new(Some("Waiting for sensor readings…"));
    overall.set_xalign(0.0);
    root.append(&overall);
    let grid = gtk::Grid::new();
    grid.set_column_spacing(12);
    grid.set_row_spacing(12);
    let mut pins = Vec::new();
    for i in 0..6 {
        let card = gtk::Box::new(gtk::Orientation::Vertical, 5);
        card.add_css_class("card");
        card.set_margin_top(8);
        card.set_margin_bottom(8);
        card.set_margin_start(8);
        card.set_margin_end(8);
        let heading = gtk::Label::new(Some(&format!("PIN {}", i + 1)));
        heading.set_xalign(0.0);
        let value = gtk::Label::new(Some("— A / — V"));
        value.set_xalign(0.0);
        let state = gtk::Label::new(Some("UNKNOWN"));
        state.set_xalign(0.0);
        card.append(&heading);
        card.append(&value);
        card.append(&state);
        grid.attach(&card, i % 3, i / 3, 1, 1);
        pins.push((value, state));
    }
    root.append(&grid);
    let connector_power = gtk::Label::new(Some("Estimated connector power: waiting for readings…"));
    connector_power.set_xalign(0.0);
    root.append(&connector_power);
    let power_note = gtk::Label::new(Some("Connector estimate = sum of six measured input currents × their voltages. This is not total GPU board power, and is not ASUS Cable Mileage."));
    power_note.set_wrap(true);
    power_note.set_xalign(0.0);
    power_note.add_css_class("dim-label");
    root.append(&power_note);
    let limits_note = gtk::Label::new(Some("Display thresholds: current warning ≥ 9.2 A, critical ≥ 9.5 A per input; voltage warning < 11.4 V, critical < 11.0 V. These values are display-only; the independent guard uses its own configuration."));
    limits_note.set_wrap(true);
    limits_note.set_xalign(0.0);
    limits_note.add_css_class("dim-label");
    root.append(&limits_note);
    let checks_title = gtk::Label::new(Some("Protection checks (read-only)"));
    checks_title.add_css_class("title-3");
    checks_title.set_xalign(0.0);
    root.append(&checks_title);
    let checks = gtk::Grid::new();
    checks.set_column_spacing(24);
    checks.set_row_spacing(10);
    let driver = make_row("Kernel sensor driver", &checks, 0);
    let guard = make_row("Guard executable", &checks, 1);
    let timer = make_row("Periodic guard timer", &checks, 2);
    let startup = make_row("Sensor startup service", &checks, 3);
    let channels = make_row("Six current + voltage channels", &checks, 4);
    let shutdown = make_row("Emergency shutdown unit", &checks, 5);
    let trigger = make_row("Guard failure trigger", &checks, 6);
    let critical_only = make_row("Critical-only exit configuration", &checks, 7);
    root.append(&checks);
    let check_note = gtk::Label::new(Some("Configuration checks update every 20 seconds. ENABLED, CONFIGURED, and WIRED are not proof that emergency shutdown will work."));
    check_note.set_wrap(true);
    check_note.set_xalign(0.0);
    check_note.add_css_class("dim-label");
    root.append(&check_note);
    let disclaimer = gtk::Label::new(Some("These checks do not prove that a critical-event shutdown will execute. Use the dedicated Astral Guard Manager for full protection verification."));
    disclaimer.set_wrap(true);
    disclaimer.set_xalign(0.0);
    disclaimer.add_css_class("dim-label");
    root.append(&disclaimer);
    let refresh_pins = move || {
        if let Some(dir) = sensor_dir() {
            let mut complete = true;
            let mut warning = false;
            let mut critical = false;
            let mut total_watts = 0.0_f64;
            for (i, (value, state)) in pins.iter().enumerate() {
                let current = read_number(&dir.join(format!("curr{}_input", i + 1)));
                let voltage = read_number(&dir.join(format!("in{}_input", i)));
                match (current, voltage) {
                    (Some(ma), Some(mv)) if ma.is_finite() && mv.is_finite() && ma >= 0.0 && mv > 0.0 => {
                        total_watts += (ma / 1000.0) * (mv / 1000.0);
                        value.set_text(&format!("{:.3} A / {:.3} V", ma / 1000.0, mv / 1000.0));
                        if ma >= PIN_CRIT_MA || mv < VOLT_CRIT_MV { status(state, "CRITICAL READING", "error"); critical = true; }
                        else if ma >= PIN_WARN_MA || mv < VOLT_WARN_MV { status(state, "WARNING", "warning"); warning = true; }
                        else { status(state, "OK", "success"); }
                    }
                    _ => { value.set_text("— A / — V"); status(state, "NO DATA", "dim-label"); complete = false; }
                }
            }
            if complete {
                connector_power.set_text(&format!("Estimated 12V connector input power: {:.1} W", total_watts));
            } else {
                connector_power.set_text("Estimated connector power: unavailable (incomplete sensor data)");
            }
            if !complete { status(&overall, "Sensor data missing — protection status not verified", "error"); }
            else if critical { status(&overall, "CRITICAL connector reading — inspect protection immediately", "error"); }
            else if warning { status(&overall, "Connector reading warning", "warning"); }
            else { status(&overall, "All six connector readings within displayed thresholds", "success"); }
        } else {
            connector_power.set_text("Estimated connector power: unavailable (no sensor)");
            status(&overall, "Astral sensor not detected — protection status not verified", "error");
            for (value, state) in &pins { value.set_text("— A / — V"); status(state, "NO DATA", "dim-label"); }
        }
        glib::ControlFlow::Continue
    };
    refresh_pins();
    glib::timeout_add_seconds_local(2, refresh_pins);
    let refresh_checks = move || {
        let present = sensor_dir().is_some();
        status(&driver, if present { "DETECTED" } else { "NOT DETECTED" }, if present { "success" } else { "error" });
        let installed = Path::new("/var/usrlocal/bin/astral-guard").is_file();
        status(&guard, if installed { "PRESENT" } else { "MISSING" }, if installed { "success" } else { "error" });
        let active = unit_active("astral-guard.timer");
        let enabled = unit_enabled("astral-guard.timer");
        status(&timer, if active && enabled { "ACTIVE + ENABLED" } else { "NOT VERIFIED" }, if active && enabled { "success" } else { "error" });
        let enabled = unit_enabled("astral-hwmon.service");
        // A oneshot startup unit can be inactive after successful completion.
        // Treat enabled as a startup configuration check, not proof of execution.
        status(&startup, if enabled { "ENABLED (NOT TESTED)" } else { "NOT VERIFIED" }, if enabled { "warning" } else { "error" });
        let complete = sensor_dir().is_some_and(|dir| (0..6).all(|i| {
            matches!(read_number(&dir.join(format!("curr{}_input", i + 1))), Some(ma) if ma.is_finite() && ma >= 0.0)
                && matches!(read_number(&dir.join(format!("in{}_input", i))), Some(mv) if mv.is_finite() && mv > 0.0)
        }));
        status(&channels, if complete { "6/6 PRESENT" } else { "NOT VERIFIED" }, if complete { "success" } else { "error" });
        let shutdown_unit = Path::new("/etc/systemd/system/astral-guard-poweroff.service");
        let shutdown_text = fs::read_to_string(shutdown_unit).ok();
        let shutdown_present = shutdown_text.as_deref().is_some_and(|t| t.contains("poweroff"));
        status(&shutdown, if shutdown_present { "CONFIGURED (NOT TESTED)" } else { "NOT VERIFIED" }, if shutdown_present { "warning" } else { "error" });
        let guard_text = fs::read_to_string("/etc/systemd/system/astral-guard.service").ok();
        let trigger_ok = guard_text.as_deref().is_some_and(|t| t.lines().any(|l| l.trim() == "OnFailure=astral-guard-poweroff.service"));
        status(&trigger, if trigger_ok { "WIRED (NOT TESTED)" } else { "NOT VERIFIED" }, if trigger_ok { "warning" } else { "error" });
        let critical_ok = guard_text.as_deref().is_some_and(|t| t.lines().any(|l| {
            let l = l.trim();
            l.starts_with("SuccessExitStatus=") && l.split_once('=').is_some_and(|(_, values)| {
                let codes: Vec<&str> = values.split_whitespace().collect();
                codes.contains(&"1") && codes.contains(&"3") && !codes.contains(&"2")
            })
        }));
        status(&critical_only, if critical_ok { "CONFIGURED (NOT TESTED)" } else { "NOT VERIFIED" }, if critical_ok { "warning" } else { "error" });
        glib::ControlFlow::Continue
    };
    refresh_checks();
    glib::timeout_add_seconds_local(20, refresh_checks);
    root
}
