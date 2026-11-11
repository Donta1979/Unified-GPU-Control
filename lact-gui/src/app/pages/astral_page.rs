//! Astral connector dashboard with guarded verification and safety configuration.
//! The independent systemd emergency protection is never owned by the GUI.
use adw::prelude::*;
use gtk::{self, glib};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::mpsc,
    thread,
    time::Duration,
};

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
    for old in ["success", "warning", "error", "dim-label"] {
        label.remove_css_class(old);
    }
    label.set_text(text);
    label.add_css_class(color);
}
fn unit_active(name: &str) -> bool {
    Command::new("systemctl")
        .args(["is-active", "--quiet", name])
        .status()
        .is_ok_and(|s| s.success())
}
fn unit_enabled(name: &str) -> bool {
    Command::new("systemctl")
        .args(["is-enabled", "--quiet", name])
        .status()
        .is_ok_and(|s| s.success())
}

// Testing16: read-only host Astral Guard status.
// Development integration only; public release requires a restricted host interface.
fn host_astral_status() -> Result<serde_json::Value, String> {
    // Flatpak's HOME is its private application directory.
    // The host publisher writes into this application's data directory.
    let home = std::env::var("HOME")
        .map_err(|e| format!("Cannot determine application home: {e}"))?;

    let path = std::env::var("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(home).join("data"))
        .join("astral-status.json");

    let content = fs::read(&path)
        .map_err(|e| format!("Cannot read Astral status snapshot: {e}"))?;

    let result: serde_json::Value = serde_json::from_slice(&content)
        .map_err(|e| format!("Invalid Astral status JSON: {e}"))?;

    if result["schema_version"].as_u64() != Some(1) {
        return Err("Unsupported Astral status schema".to_string());
    }

    let generated = result["generated_at"]
        .as_u64()
        .ok_or("Astral status snapshot has no timestamp")?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| format!("System clock error: {e}"))?
        .as_secs();

    if generated > now || now - generated > 15 {
        return Err("Astral status snapshot is stale".to_string());
    }

    Ok(result)
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

// Never treat spawning pkexec as proof that a privileged operation succeeded.
fn background_action(result: gtk::Label, title: &'static str, args: Vec<String>) {
    result.set_text(&format!("{title}: waiting for authorization / completion…"));
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let answer = Command::new("pkexec").args(&args).output().map(|output| {
            let details = format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            (output.status.success(), details.trim().to_string())
        });
        let _ = tx.send(answer);
    });
    glib::timeout_add_local(Duration::from_millis(200), move || match rx.try_recv() {
        Ok(Ok((true, details))) => {
            status(
                &result,
                &format!(
                    "{title}: {}",
                    if details.is_empty() {
                        "Completed"
                    } else {
                        &details
                    }
                ),
                "success",
            );
            glib::ControlFlow::Break
        }
        Ok(Ok((false, details))) => {
            status(&result, &format!("{title} FAILED: {details}"), "error");
            glib::ControlFlow::Break
        }
        Ok(Err(e)) => {
            status(&result, &format!("{title} FAILED: {e}"), "error");
            glib::ControlFlow::Break
        }
        Err(mpsc::TryRecvError::Disconnected) => {
            status(&result, "Action failed: worker disconnected", "error");
            glib::ControlFlow::Break
        }
        Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
    });
}
fn confirm_action(title: &'static str, details: &str, action: impl Fn() + 'static) {
    let dialog = gtk::MessageDialog::builder()
        .modal(true)
        .message_type(gtk::MessageType::Warning)
        .buttons(gtk::ButtonsType::None)
        .text(title)
        .secondary_text(details)
        .build();
    dialog.add_button("Cancel", gtk::ResponseType::Cancel);
    dialog.add_button("Continue", gtk::ResponseType::Accept);
    dialog.connect_response(move |dialog, response| {
        dialog.close();
        if response == gtk::ResponseType::Accept {
            action();
        }
    });
    dialog.present();
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
    let note = gtk::Label::new(Some("Live monitoring and safety management. The independent Astral Guard systemd services continue operating when UGC closes."));
    note.set_wrap(true);
    note.set_xalign(0.0);
    root.append(&note);

    let safety_title = gtk::Label::new(Some("Safety management"));
    safety_title.add_css_class("title-3");
    safety_title.set_xalign(0.0);
    root.append(&safety_title);
    let safety_actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let refresh = gtk::Button::with_label("Refresh Status");
    let startup = gtk::Button::with_label("Enable / Repair Startup");
    let verify = gtk::Button::with_label("Run Guard Check");
    let reinstall = gtk::Button::with_label("Install / Reinstall");
    let countdown = gtk::DropDown::from_strings(&[
        "5 seconds",
        "10 seconds",
        "15 seconds",
        "20 seconds",
        "30 seconds",
    ]);
    countdown.set_selected(1);
    let configure = gtk::Button::with_label("Apply Shutdown Countdown…");
    safety_actions.append(&refresh);
    safety_actions.append(&startup);
    safety_actions.append(&verify);
    safety_actions.append(&reinstall);
    safety_actions.append(&countdown);
    safety_actions.append(&configure);
    root.append(&safety_actions);
    let siren = gtk::Button::with_label("Test Siren (no shutdown)");
    let notifier_repair = gtk::Button::with_label("Repair Desktop Notifier");
    let notifier_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    notifier_row.append(&siren);
    notifier_row.append(&notifier_repair);
    root.append(&notifier_row);
    let manager_status = gtk::Label::new(Some("A critical shutdown is never simulated by this page. Configuration changes require graphical administrator authorization."));
    manager_status.set_xalign(0.0);
    manager_status.set_wrap(true);
    root.append(&manager_status);

    // Testing16: sandbox-safe controls.
    // Host management actions require a separately tested, authorized interface.
    refresh.set_sensitive(false);
    startup.set_sensitive(false);
    verify.set_sensitive(false);
    reinstall.set_sensitive(false);
    configure.set_sensitive(false);
    notifier_repair.set_sensitive(false);
    siren.set_sensitive(false);

    countdown.set_sensitive(false);
    manager_status.set_text(
        "Read-only monitoring mode. Host management actions are unavailable inside this Flatpak. Astral Guard continues operating independently."
    );

    {
        let result = manager_status.clone();
        verify.connect_clicked(move |_| {
            match Command::new("/var/usrlocal/bin/astral-guard").output() {
                Ok(output) => {
                    let code = output.status.code();
                    let message = String::from_utf8_lossy(&output.stdout);
                    let verdict = match code {
                        Some(0) => "OK",
                        Some(1) => "WARNING",
                        Some(2) => "CRITICAL — inspect protection immediately",
                        Some(3) => "UNKNOWN — protection not verified",
                        _ => "ERROR — unexpected guard exit",
                    };
                    result.set_text(&format!("Guard check: {verdict}. {}", message.trim()));
                }
                Err(e) => result.set_text(&format!("Guard check could not run: {e}")),
            }
        });
    }
    {
        let result = manager_status.clone();
        let countdown = countdown.clone();
        configure.connect_clicked(move |_| {
            let seconds = match countdown.selected() { 0 => "5", 1 => "10", 2 => "15", 3 => "20", 4 => "30", _ => "10" };
            let dialog = gtk::MessageDialog::builder()
                .modal(true)
                .message_type(gtk::MessageType::Warning)
                .buttons(gtk::ButtonsType::None)
                .text("Change emergency shutdown countdown?")
                .secondary_text(format!("Set the critical-event countdown to {seconds} seconds. The independent guard will continue running. Administrator authorization is required."))
                .build();
            dialog.add_button("Cancel", gtk::ResponseType::Cancel);
            dialog.add_button("Authorize Change", gtk::ResponseType::Accept);
            let result = result.clone();
            let seconds = seconds.to_string();
            dialog.connect_response(move |dialog, response| {
                dialog.close();
                if response != gtk::ResponseType::Accept { return; }
                let helper = "/usr/local/lib/astral-guard-manager/admin.py";
                if !Path::new(helper).is_file() {
                    result.set_text("Administrator helper missing. No changes made.");
                    return;
                }
                background_action(result.clone(), "Shutdown countdown", vec![
                    "/usr/bin/python3".into(), helper.into(), "configure".into(), seconds.clone()
                ]);
            });
            dialog.present();
        });
    }

    {
        let result = manager_status.clone();
        refresh.connect_clicked(move |_| {
            let now = glib::DateTime::now_local()
                .and_then(|dt| dt.format("%H:%M:%S"))
                .map(|s| s.to_string())
                .unwrap_or_else(|_| "now".into());
            let timer = unit_active("astral-guard.timer");
            let enabled = unit_enabled("astral-guard.timer");
            let pins = sensor_dir().is_some();
            let message = format!(
                "Manual refresh {now}: sensor {}, timer {}, startup {}. Live pins and protection checks also refresh automatically.",
                if pins { "detected" } else { "MISSING" },
                if timer { "active" } else { "INACTIVE" },
                if enabled { "enabled" } else { "DISABLED" }
            );
            status(&result, &message, if pins && timer && enabled { "success" } else { "warning" });
        });
    }
    {
        let result = manager_status.clone();
        startup.connect_clicked(move |_| {
            let result = result.clone();
            confirm_action("Enable / Repair Startup", "This enables and starts the existing Astral services. If the guard detects a real CRITICAL condition, the independent shutdown protection may activate. Continue?", move || {
                background_action(result.clone(), "Startup repair", vec!["/usr/bin/systemctl".into(), "enable".into(), "--now".into(), "astral-hwmon.service".into(), "astral-guard.timer".into()]);
            });
        });
    }
    {
        let result = manager_status.clone();
        reinstall.connect_clicked(move |_| {
            let result = result.clone();
            confirm_action("Install / Reinstall Protection", "Only use if protection is broken. The existing root helper rebuilds the driver and replaces safety units. This recovery path has not been end-to-end tested; a working installation should not be reinstalled. Continue anyway?", move || {
                let helper = "/usr/local/lib/astral-guard-manager/admin.py";
                if !Path::new(helper).is_file() { status(&result, "Root repair helper missing", "error"); return; }
                background_action(result.clone(), "Reinstall", vec!["/usr/bin/python3".into(), helper.into(), "reinstall".into()]);
            });
        });
    }
    {
        let result = manager_status.clone();
        notifier_repair.connect_clicked(move |_| {
            let result = result.clone();
            confirm_action("Repair desktop notifier?",
                "Restore the desktop notification launcher, siren and user service from the bundled UGC files. This does not change the independent root emergency shutdown protection.",
                move || {
                    let script = std::env::var_os("HOME").map(PathBuf::from)
                        .map(|home| home.join(".local/share/ugc/astral-notifier-repair.sh"));
                    let Some(script) = script.filter(|p| p.is_file()) else {
                        status(&result, "Notifier repair package is missing. Reinstall the UGC Stage 4 notifier component.", "error");
                        return;
                    };
                    status(&result, "Desktop notifier repair in progress…", "warning");
                    let (tx, rx) = mpsc::channel();
                    thread::spawn(move || {
                        let answer = Command::new("/usr/bin/bash").arg(&script).output()
                            .map(|output| (output.status.success(), format!("{}{}",
                                String::from_utf8_lossy(&output.stdout),
                                String::from_utf8_lossy(&output.stderr))));
                        let _ = tx.send(answer);
                    });
                    let result = result.clone();
                    glib::timeout_add_local(Duration::from_millis(200), move || match rx.try_recv() {
                        Ok(Ok((true, details))) => {
                            status(&result, &format!("Desktop notifier: {}", details.trim()), "success");
                            glib::ControlFlow::Break
                        }
                        Ok(Ok((false, details))) => {
                            status(&result, &format!("Desktop notifier repair FAILED: {}", details.trim()), "error");
                            glib::ControlFlow::Break
                        }
                        Ok(Err(e)) => {
                            status(&result, &format!("Desktop notifier repair FAILED: {e}"), "error");
                            glib::ControlFlow::Break
                        }
                        Err(mpsc::TryRecvError::Disconnected) => {
                            status(&result, "Desktop notifier repair worker disconnected", "error");
                            glib::ControlFlow::Break
                        }
                        Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                    });
                });
        });
    }
    {
        let result = manager_status.clone();
        siren.connect_clicked(move |_| {
            let sound = std::env::var_os("HOME")
                .map(PathBuf::from)
                .map(|p| p.join(".local/share/astral-guard-manager/siren.wav"));
            let Some(sound) = sound.filter(|p| p.is_file()) else {
                status(&result, "Siren file not found", "error");
                return;
            };
            let player = ["pw-play", "paplay", "aplay"].iter().find(|name| {
                std::env::var_os("PATH").is_some_and(|paths| {
                    std::env::split_paths(&paths).any(|dir| dir.join(name).is_file())
                })
            });
            if let Some(player) = player {
                match Command::new(player).arg(sound).spawn() {
                    Ok(_) => result.set_text("Siren playback started; no shutdown requested."),
                    Err(e) => status(&result, &format!("Siren failed: {e}"), "error"),
                }
            } else {
                status(&result, "No supported audio player found", "error");
            }
        });
    }

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
    let critical_only = make_row("Kernel module matches boot", &checks, 7);
    root.append(&checks);
    let check_note = gtk::Label::new(Some("Configuration checks update every 5 seconds. ENABLED, CONFIGURED, and WIRED are not proof that emergency shutdown will work."));
    check_note.set_wrap(true);
    check_note.set_xalign(0.0);
    check_note.add_css_class("dim-label");
    root.append(&check_note);
    let disclaimer = gtk::Label::new(Some("These checks do not prove that a critical-event shutdown will execute. A real critical-event shutdown is not simulated here."));
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
                    (Some(ma), Some(mv))
                        if ma.is_finite() && mv.is_finite() && ma >= 0.0 && mv > 0.0 =>
                    {
                        total_watts += (ma / 1000.0) * (mv / 1000.0);
                        value.set_text(&format!("{:.3} A / {:.3} V", ma / 1000.0, mv / 1000.0));
                        if ma >= PIN_CRIT_MA || mv < VOLT_CRIT_MV {
                            status(state, "CRITICAL READING", "error");
                            critical = true;
                        } else if ma >= PIN_WARN_MA || mv < VOLT_WARN_MV {
                            status(state, "WARNING", "warning");
                            warning = true;
                        } else {
                            status(state, "OK", "success");
                        }
                    }
                    _ => {
                        value.set_text("— A / — V");
                        status(state, "NO DATA", "dim-label");
                        complete = false;
                    }
                }
            }
            if complete {
                connector_power.set_text(&format!(
                    "Estimated 12V connector input power: {:.1} W",
                    total_watts
                ));
            } else {
                connector_power
                    .set_text("Estimated connector power: unavailable (incomplete sensor data)");
            }
            if !complete {
                status(
                    &overall,
                    "Sensor data missing — protection status not verified",
                    "error",
                );
            } else if critical {
                status(
                    &overall,
                    "CRITICAL connector reading — inspect protection immediately",
                    "error",
                );
            } else if warning {
                status(&overall, "Connector reading warning", "warning");
            } else {
                status(
                    &overall,
                    "All six connector readings within displayed thresholds",
                    "success",
                );
            }
        } else {
            connector_power.set_text("Estimated connector power: unavailable (no sensor)");
            status(
                &overall,
                "Astral sensor not detected — protection status not verified",
                "error",
            );
            for (value, state) in &pins {
                value.set_text("— A / — V");
                status(state, "NO DATA", "dim-label");
            }
        }
        glib::ControlFlow::Continue
    };
    refresh_pins();
    glib::timeout_add_seconds_local(2, refresh_pins);

    // Testing16: asynchronous host verification with stale-result handling.
    let check_labels = [
        driver,
        guard,
        timer,
        startup,
        channels,
        shutdown,
        trigger,
        critical_only,
    ];

    let (tx, rx) = mpsc::channel::<Result<serde_json::Value, String>>();

    let request_pending = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

    let last_success = std::rc::Rc::new(std::cell::Cell::new(None::<std::time::Instant>));

    let refresh_checks = {
        let request_pending = request_pending.clone();

        move || {
            use std::sync::atomic::Ordering;

            if request_pending.swap(true, Ordering::AcqRel) {
                return glib::ControlFlow::Continue;
            }

            let tx = tx.clone();
            let pending = request_pending.clone();

            thread::spawn(move || {
                let result = host_astral_status();
                let _ = tx.send(result);
                pending.store(false, Ordering::Release);
            });

            glib::ControlFlow::Continue
        }
    };

    refresh_checks();
    glib::timeout_add_seconds_local(5, refresh_checks);

    glib::timeout_add_local(Duration::from_millis(200), move || {
        match rx.try_recv() {
            Ok(Ok(result)) => {
                let checks = result["checks"].as_array();

                // Each row uses the corresponding host verification.
                let mapping = [
                    "Kernel driver loaded",
                    "Guard executable installed",
                    "Periodic guard timer",
                    "Driver starts at boot",
                    "Sensor interface available",
                    "Emergency shutdown action",
                    "Critical-only trigger",
                    "Kernel module matches boot",
                ];

                let mut complete = true;

                for (label, name) in check_labels.iter().zip(mapping.iter()) {
                    let check = checks.and_then(|items| {
                        items
                            .iter()
                            .find(|item| item["label"].as_str() == Some(*name))
                    });

                    match check {
                        Some(check) if check["ok"].as_bool() == Some(true) => {
                            status(label, "CONFIGURED (NOT TESTED)", "success");
                        }
                        Some(check) if check["ok"].as_bool() == Some(false) => {
                            status(label, "NOT VERIFIED", "error");
                            complete = false;
                        }
                        _ => {
                            status(label, "UNABLE TO VERIFY", "error");
                            complete = false;
                        }
                    }
                }

                if complete {
                    last_success.set(Some(std::time::Instant::now()));
                } else {
                    last_success.set(None);
                }
            }
            Ok(Err(_)) | Err(mpsc::TryRecvError::Disconnected) => {
                last_success.set(None);
                for label in &check_labels {
                    status(label, "UNABLE TO VERIFY", "error");
                }
            }
            Err(mpsc::TryRecvError::Empty) => {}
        }

        // Do not display old successful verification indefinitely.
        let stale = last_success
            .get()
            .is_some_and(|instant| instant.elapsed() > Duration::from_secs(15));

        if stale {
            last_success.set(None);
            for label in &check_labels {
                status(label, "STATUS STALE", "warning");
            }
        }

        glib::ControlFlow::Continue
    });
    root
}
