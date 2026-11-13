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

// Run Astral operations on the host when packaged as Flatpak.
// Administrative changes still require host polkit authorization.
fn host_command(program: &str, args: &[&str]) -> Command {
    if std::env::var_os("FLATPAK_ID").is_some() {
        let mut command = Command::new("flatpak-spawn");
        command.arg("--host").arg(program).args(args);
        command
    } else {
        let mut command = Command::new(program);
        command.args(args);
        command
    }
}

// Testing45: read-only recovery triage. This never runs the guard or changes services.
fn recovery_triage() -> Result<(&'static str, String), String> {
    let module = "/var/usrlocal/lib/astral-hwmon/astral-hwmon.ko";
    let guard = "/var/usrlocal/bin/astral-guard";
    let paths = [
        module, guard,
        "/etc/systemd/system/astral-hwmon.service",
        "/etc/systemd/system/astral-guard.service",
        "/etc/systemd/system/astral-guard.timer",
        "/etc/systemd/system/astral-guard-poweroff.service",
    ];
    let present = paths.iter().filter(|path| host_file_exists(path)).count();
    let loaded = host_command("/usr/bin/test", &["-d", "/sys/module/astral_hwmon"])
        .status().map_err(|e| format!("Cannot inspect kernel module: {e}"))?.success();
    if present == 0 && !loaded {
        return Ok(("warning", "Astral Guard not installed. Automatic installation is disabled pending isolated testing.".into()));
    }
    if present != paths.len() {
        return Ok(("error", format!("Partial installation: {present}/{} required files found; module loaded={loaded}. Manual diagnosis required.", paths.len())));
    }
    let uname = host_command("/usr/bin/uname", &["-r"])
        .output().map_err(|e| format!("Kernel query failed: {e}"))?;
    let modinfo = host_command("/usr/sbin/modinfo", &["-F", "vermagic", module])
        .output().map_err(|e| format!("Driver metadata query failed: {e}"))?;
    if !uname.status.success() || !modinfo.status.success() {
        return Err("Kernel or module metadata unavailable; no repair attempted".into());
    }
    let kernel = String::from_utf8_lossy(&uname.stdout).trim().to_string();
    let magic = String::from_utf8_lossy(&modinfo.stdout);
    if magic.split_whitespace().next() != Some(kernel.as_str()) {
        return Ok(("error", "Kernel mismatch. Automatic driver replacement blocked.".into()));
    }
    let check = |op: &str, unit: &str| -> Result<bool, String> {
        let output = host_command("/usr/bin/systemctl", &[op, unit])
            .output().map_err(|e| format!("{unit}: {e}"))?;
        Ok(output.status.success() && String::from_utf8_lossy(&output.stdout).trim() ==
            (if op == "is-active" { "active" } else { "enabled" }))
    };
    let ready = loaded
        && check("is-active", "astral-hwmon.service")?
        && check("is-active", "astral-guard.timer")?
        && check("is-enabled", "astral-guard.timer")?;
    if ready {
        Ok(("success", "Driver loaded, kernel matches, and monitoring services report active/enabled. Emergency shutdown was NOT tested.".into()))
    } else {
        Ok(("error", "Protection is not fully operational. Automatic reinstall blocked; inspect service status before any repair.".into()))
    }
}

// Testing59: read-only installation preflight. A fresh candidate is NOT authorization
// to install; the privileged host installer must independently recheck everything.
fn installation_preflight() -> Result<(&'static str, String), String> {
    let files = [
        "/var/usrlocal/lib/astral-hwmon/astral-hwmon.ko",
        "/var/usrlocal/bin/astral-guard",
        "/etc/systemd/system/astral-hwmon.service",
        "/etc/systemd/system/astral-guard.service",
        "/etc/systemd/system/astral-guard.timer",
        "/etc/systemd/system/astral-guard-poweroff.service",
        "/var/usrlocal/lib/astral-guard-manager/admin.py",
    ];
    let mut found = 0;
    for file in files {
        let output = host_command("/usr/bin/test", &["-e", file])
            .status().map_err(|e| format!("Cannot inspect {file}: {e}"))?;
        if output.success() { found += 1; }
    }
    let loaded = host_command("/usr/bin/test", &["-d", "/sys/module/astral_hwmon"])
        .status().map_err(|e| format!("Cannot inspect loaded module: {e}"))?.success();
    let units = ["astral-hwmon.service", "astral-guard.service",
        "astral-guard.timer", "astral-guard-poweroff.service"];
    let mut all_units_absent = true;
    for unit in units {
        let output = host_command("/usr/bin/systemctl",
            &["show", unit, "-p", "LoadState", "--value"])
            .output().map_err(|e| format!("Cannot inspect {unit}: {e}"))?;
        if !output.status.success() {
            return Err(format!("Cannot verify systemd unit {unit}; installation blocked"));
        }
        let state = String::from_utf8_lossy(&output.stdout);
        match state.trim() {
            "not-found" => {},
            "loaded" | "masked" | "error" | "bad-setting" => all_units_absent = false,
            other => return Err(format!("Unknown systemd state for {unit}: {other}; installation blocked")),
        }
    }
    if found == 0 && !loaded && all_units_absent {
        return Ok(("warning", "FRESH INSTALL CANDIDATE: No Astral files, loaded module, or systemd units detected. This is informational only; installation remains disabled until a separately verified privileged installer is available.".into()));
    }
    if found != 0 && found != files.len() || (found == 0 && (loaded || !all_units_absent)) {
        return Ok(("error", format!("BLOCKED: Incomplete or inconsistent installation (files {found}/{}, loaded module={loaded}). No changes made.", files.len())));
    }
    // Existing installations are checked, never replaced by this preflight.
    let (color, details) = recovery_triage()?;
    Ok((color, format!("EXISTING INSTALLATION: {details} No installation attempted.")))
}

fn host_file_exists(path: &str) -> bool {
    host_command("/usr/bin/test", &["-f", path])
        .status()
        .is_ok_and(|status| status.success())
}

fn host_background_action(
    result: gtk::Label,
    title: &'static str,
    program: &'static str,
    args: Vec<String>,
) {
    result.set_text(&format!("{title}: waiting for authorization / completion…"));
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
        let answer = host_command(program, &borrowed).output().map(|output| {
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
        Ok(Err(err)) => {
            status(&result, &format!("{title} FAILED: {err}"), "error");
            glib::ControlFlow::Break
        }
        Err(mpsc::TryRecvError::Disconnected) => {
            status(&result, "Action failed: worker disconnected", "error");
            glib::ControlFlow::Break
        }
        Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
    });
}

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
// Testing16: read-only host Astral Guard status.
// Development integration only; public release requires a restricted host interface.
fn host_astral_status() -> Result<serde_json::Value, String> {
    // Flatpak's HOME is its private application directory.
    // The host publisher writes into this application's data directory.
    let home =
        std::env::var("HOME").map_err(|e| format!("Cannot determine application home: {e}"))?;

    let path = std::env::var("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(home).join("data"))
        .join("astral-status.json");

    let content =
        fs::read(&path).map_err(|e| format!("Cannot read Astral status snapshot: {e}"))?;

    let result: serde_json::Value =
        serde_json::from_slice(&content).map_err(|e| format!("Invalid Astral status JSON: {e}"))?;

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
    let note = gtk::Label::new(Some(
        "Live monitoring and safety management. The independent Astral Guard systemd services continue operating when UGC closes.",
    ));
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
    let verify = gtk::Button::with_label("Check Guard Health");
    let reinstall = gtk::Button::with_label("Install Astral Guard (fresh only)");
    let recovery_check = gtk::Button::with_label("Check Recovery State");
    let install_preflight = gtk::Button::with_label("Check Installation Readiness");
    let install_preview = gtk::Button::with_label("Preview Fresh Install (no changes)");
    let countdown = gtk::DropDown::from_strings(&[
        "5 seconds",
        "10 seconds",
        "15 seconds",
        "20 seconds",
        "30 seconds",
    ]);
    // Read the actual emergency shutdown delay from the host.
    // Never change the independent guard configuration here.
    let current_delay = host_command(
        "/usr/bin/systemctl",
        &[
            "show",
            "astral-guard-poweroff.service",
            "-p",
            "ExecStartPre",
            "--value",
        ],
    )
    .output();

    let selected_delay = current_delay.ok().and_then(|output| {
        if !output.status.success() {
            return None;
        }

        let details = String::from_utf8_lossy(&output.stdout);

        for (index, seconds) in ["5", "10", "15", "20", "30"].iter().enumerate() {
            let expected = format!("argv[]=/usr/bin/sleep {seconds} ;");
            if details.contains(&expected) {
                return Some(index as u32);
            }
        }

        None
    });

    // An unknown configuration must not be presented as 10 seconds.
    countdown.set_selected(selected_delay.unwrap_or(gtk::INVALID_LIST_POSITION));

    let configure = gtk::Button::with_label("Apply Shutdown Countdown…");
    let siren = gtk::Button::with_label("Test Siren (no shutdown)");
    let notifier_repair = gtk::Button::with_label("Repair Desktop Notifier");

    // Everyday controls remain visible.
    safety_actions.append(&refresh);
    safety_actions.append(&startup);
    safety_actions.append(&verify);
    safety_actions.append(&siren);
    root.append(&safety_actions);

    // Less frequently used tools remain available without crowding the page.
    let advanced = gtk::Expander::new(Some("Advanced"));
    advanced.set_expanded(false);

    let advanced_actions = gtk::Box::new(gtk::Orientation::Vertical, 8);
    let advanced_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    advanced_row.append(&reinstall);
    advanced_row.append(&recovery_check);
    advanced_row.append(&install_preflight);
    advanced_row.append(&install_preview);
    advanced_actions.append(&advanced_row);

    let countdown_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    countdown_row.append(&countdown);
    countdown_row.append(&configure);
    countdown_row.append(&notifier_repair);
    advanced_actions.append(&countdown_row);

    advanced.set_child(Some(&advanced_actions));
    root.append(&advanced);
    let manager_status = gtk::Label::new(Some(
        "A critical shutdown is never simulated by this page. Configuration changes require graphical administrator authorization.",
    ));
    manager_status.set_xalign(0.0);
    manager_status.set_wrap(true);
    root.append(&manager_status);

    // Recovery/reinstallation must not be exposed until its privileged helper
    // has been separately audited and tested on the host.
    reinstall.set_sensitive(false);
    reinstall.set_tooltip_text(Some(
        "Fresh installation is available through the UGC release installer; in-app reinstallation is unavailable",
    ));
    manager_status.set_text("Host controls enabled. Privileged actions require authorization. Use the UGC release installer for fresh Astral Guard installation.");

    {
        let result = manager_status.clone();
        verify.connect_clicked(move |_| {
            // Read existing service state only. Never start the real guard.
            let query = |args: &[&str]| -> Result<String, String> {
                let output = host_command("/usr/bin/systemctl", args)
                    .output()
                    .map_err(|e| format!("Cannot query guard status: {e}"))?;
                if !output.status.success() {
                    return Err(format!(
                        "Guard status query failed: {}",
                        String::from_utf8_lossy(&output.stderr).trim()
                    ));
                }
                Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
            };

            let health = (|| -> Result<(String, bool), String> {
                let timer = query(&["is-active", "astral-guard.timer"])?;
                let enabled = query(&["is-enabled", "astral-guard.timer"])?;
                let details = query(&[
                    "show", "astral-guard.service",
                    "-p", "Result",
                    "-p", "ExecMainStatus",
                    "-p", "ExecMainExitTimestamp",
                    "--no-pager",
                ])?;

                let mut guard_result = None;
                let mut exit_code = None;
                let mut completed = None;

                for line in details.lines() {
                    if let Some(value) = line.strip_prefix("Result=") {
                        guard_result = Some(value);
                    } else if let Some(value) = line.strip_prefix("ExecMainStatus=") {
                        exit_code = Some(value);
                    } else if let Some(value) = line.strip_prefix("ExecMainExitTimestamp=") {
                        completed = Some(value);
                    }
                }

                let guard_result = guard_result.ok_or("Missing guard result")?;
                let exit_code = exit_code.ok_or("Missing guard exit code")?;
                let completed = completed.ok_or("Missing guard completion time")?;

                let healthy = timer == "active"
                    && enabled == "enabled"
                    && guard_result == "success"
                    && exit_code == "0"
                    && !completed.is_empty();

                Ok((
                    format!(
                        "Guard health: timer {timer}, startup {enabled}, last result {guard_result}, exit code {exit_code}, completed {completed}. No guard execution requested."
                    ),
                    healthy,
                ))
            })();

            match health {
                Ok((message, healthy)) => {
                    status(&result, &message, if healthy { "success" } else { "warning" });
                }
                Err(error) => status(&result, &error, "error"),
            }
        });
    }
    {
        let result = manager_status.clone();
        let countdown = countdown.clone();
        configure.connect_clicked(move |_| {
            let seconds = match countdown.selected() { 0 => "5", 1 => "10", 2 => "15", 3 => "20", 4 => "30", _ => {
                status(&result, "Select a valid shutdown countdown first. No changes made.", "warning");
                return;
            } };
            // Avoid rewriting working protection when the selected delay
            // already matches the host's emergency shutdown configuration.
            let current = host_command(
                "/usr/bin/systemctl",
                &["show", "astral-guard-poweroff.service", "-p", "ExecStartPre", "--value"],
            ).output();

            if let Ok(output) = current {
                if output.status.success() {
                    let details = String::from_utf8_lossy(&output.stdout);
                    let expected = format!("/usr/bin/sleep {seconds}");
                    if details.contains(&expected) {
                        status(
                            &result,
                            &format!("Shutdown countdown already configured for {seconds} seconds. No changes made."),
                            "success",
                        );
                        return;
                    }
                }
            }

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
                let helper = "/var/usrlocal/lib/astral-guard-manager/admin.py";
                if !host_file_exists(helper) {
                    result.set_text("Administrator helper missing. No changes made.");
                    return;
                }
                host_background_action(result.clone(), "Shutdown countdown", "/usr/bin/pkexec", vec![
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
            let timer = host_command("/usr/bin/systemctl", &["is-active", "--quiet", "astral-guard.timer"])
                .status().is_ok_and(|s| s.success());
            let enabled = host_command("/usr/bin/systemctl", &["is-enabled", "--quiet", "astral-guard.timer"])
                .status().is_ok_and(|s| s.success());
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

            // Check the independent host services before offering repairs.
            let services = ["astral-hwmon.service", "astral-guard.timer"];
            let mut all_ready = true;

            for service in services {
                let check = |operation: &str| -> Result<String, String> {
                    let output = host_command(
                        "/usr/bin/systemctl", &[operation, service]
                    ).output().map_err(|e| format!("{service}: {e}"))?;

                    let state = String::from_utf8_lossy(&output.stdout)
                        .trim().to_string();

                    if state.is_empty() || !matches!(
                        state.as_str(),
                        "active" | "inactive" | "failed" | "activating"
                        | "deactivating" | "enabled" | "disabled"
                        | "static" | "masked" | "indirect"
                    ) {
                        return Err(format!(
                            "Unable to determine {service} {operation} status: {state}"
                        ));
                    }

                    Ok(state)
                };

                // Only offer the existing repair command for
                // ordinary inactive or disabled services.
                let active_state = match check("is-active") {
                    Ok(state) => state,
                    Err(error) => {
                        status(&result, &error, "error");
                        return;
                    }
                };

                let enabled_state = match check("is-enabled") {
                    Ok(state) => state,
                    Err(error) => {
                        status(&result, &error, "error");
                        return;
                    }
                };

                if !matches!(active_state.as_str(), "active" | "inactive")
                    || !matches!(enabled_state.as_str(), "enabled" | "disabled")
                {
                    status(
                        &result,
                        &format!(
                            "{service} requires manual review: active={active_state}, enabled={enabled_state}. No changes made."
                        ),
                        "error",
                    );
                    return;
                }

                if active_state != "active" || enabled_state != "enabled" {
                    all_ready = false;
                }
            }

            // A kernel mismatch is not a systemd startup problem. Only a
            // separately authenticated, root-owned recovery entrypoint may
            // rebuild the driver. Never unload an active Astral module.
            let kernel = host_command("/usr/bin/uname", &["-r"]).output();
            let module = "/var/usrlocal/lib/astral-hwmon/astral-hwmon.ko";
            let magic = host_command("/usr/sbin/modinfo", &["-F", "vermagic", module]).output();
            if let (Ok(kernel), Ok(magic)) = (kernel, magic) {
                if kernel.status.success() && magic.status.success() {
                    let current = String::from_utf8_lossy(&kernel.stdout).trim().to_string();
                    let built_for = String::from_utf8_lossy(&magic.stdout)
                        .split_whitespace().next().unwrap_or("").to_string();
                    if !current.is_empty() && !built_for.is_empty() && current != built_for {
                        let loaded = host_command("/usr/bin/test", &["-d", "/sys/module/astral_hwmon"])
                            .status().is_ok_and(|s| s.success());
                        if loaded {
                            status(&result, "Kernel mismatch detected, but Astral is loaded. Automatic replacement refused to preserve active protection.", "error");
                            return;
                        }
                        let recovery = "/var/lib/ugc/astral-release/ugc-astral-kernel-recovery.py";
                        // Checking existence does not establish publisher trust;
                        // the privileged entrypoint independently checks root
                        // ownership, permissions and pinned source hashes.
                        if !host_file_exists(recovery) {
                            status(&result, "Kernel mismatch detected. The authenticated recovery component has not been installed; no repair attempted.", "error");
                            return;
                        }
                        confirm_action("Enable / Repair Startup", "The installed driver targets an older kernel. Request administrator authorization to rebuild it from the pinned Astral source? This will not unload an active driver.", move || {
                            host_background_action(result.clone(), "Kernel recovery", "/usr/bin/pkexec", vec!["/usr/bin/python3".into(), recovery.into(), "recover".into()]);
                        });
                        return;
                    }
                }
            }

            if all_ready {
                status(
                    &result,
                    "Startup protection already active and enabled. No repair needed.",
                    "success",
                );
                return;
            }

            confirm_action("Enable / Repair Startup", "This enables and starts the existing Astral services. If the guard detects a real CRITICAL condition, the independent shutdown protection may activate. Continue?", move || {
                host_background_action(result.clone(), "Startup repair", "/usr/bin/pkexec", vec!["/usr/bin/systemctl".into(), "enable".into(), "--now".into(), "astral-hwmon.service".into(), "astral-guard.timer".into()]);
            });
        });
    }
    {
        let result = manager_status.clone();
        recovery_check.connect_clicked(move |_| {
            match recovery_triage() {
                Ok((color, message)) => status(&result, &format!("Recovery check: {message}"), color),
                Err(error) => status(&result, &format!("Recovery check unavailable: {error}"), "error"),
            }
        });
    }
    {
        let result = manager_status.clone();
        install_preflight.connect_clicked(move |_| {
            match installation_preflight() {
                Ok((color, message)) => status(&result, &format!("Installation preflight: {message}"), color),
                Err(error) => status(&result, &format!("Installation preflight BLOCKED: {error}"), "error"),
            }
        });
    }
    // Testing86: preview only. Do not execute pkexec, flatpak-spawn or any installer.
    // A GUI preflight result cannot authorize a privileged operation.
    {
        let result = manager_status.clone();
        install_preview.connect_clicked(move |_| {
            match installation_preflight() {
                Ok((_, message)) => status(
                    &result,
                    &format!(
                        "Fresh-install preview ONLY — {message} \
                         Proposed future request: flatpak-spawn --host /usr/bin/pkexec \
                         /var/lib/ugc/astral-release/ugc-astral-deploy.py install-fresh. \
                         No installer was launched. The trusted helper is not deployed."
                    ),
                    "warning",
                ),
                Err(error) => status(
                    &result,
                    &format!("Fresh-install preview BLOCKED: {error}. No installer was launched."),
                    "error",
                ),
            }
        });
    }
    // Prepared fresh-host GUI path. The button stays disabled until an isolated
    // clean-host installation validates the RPM, polkit and service activation.
    // The root-owned installer must independently enforce all safety checks.
    {
        let result = manager_status.clone();
        reinstall.connect_clicked(move |_| {
            let eligible = match installation_preflight() {
                Ok(("warning", message)) if message.starts_with("FRESH INSTALL CANDIDATE:") => true,
                Ok((_, message)) => {
                    status(&result, &format!("Fresh installation blocked: {message}"), "error");
                    false
                }
                Err(error) => {
                    status(&result, &format!("Fresh installation blocked: {error}"), "error");
                    false
                }
            };
            if !eligible { return; }
            if !host_command("/usr/bin/test", &["-x", "/var/lib/ugc/astral-release/ugc-astral-deploy.py"])
                .status().is_ok_and(|s| s.success()) {
                status(&result, "Trusted host installer not installed. No changes made.", "error");
                return;
            }
            let result = result.clone();
            confirm_action("Install Astral Guard on this clean host?",
                "Requires administrator authorization. Only a fresh, compatible system is supported. Existing protection will never be replaced. The privileged installer performs independent checks.",
                move || {
                    // Run outside the GTK event handler to avoid freezing the GUI
                    // while the administrator authorization dialog is displayed.
                    status(&result, "Requesting administrator authorization for fresh installation…", "warning");
                    let result = result.clone();
                    let (tx, rx) = mpsc::channel();
                    thread::spawn(move || {
                        let output = host_command("/usr/bin/pkexec", &[
                            "/var/lib/ugc/astral-release/ugc-astral-deploy.py", "install-fresh"
                        ]).output();
                        let message = match output {
                            Ok(o) if o.status.success() => ("success", format!("Fresh installation completed: {}", String::from_utf8_lossy(&o.stdout).trim())),
                            Ok(o) => ("error", format!("Fresh installation failed or was cancelled ({}). Check for partial protection before retrying. {}", o.status, String::from_utf8_lossy(&o.stderr).trim())),
                            Err(e) => ("error", format!("Cannot launch trusted installer: {e}")),
                        };
                        let _ = tx.send(message);
                    });
                    glib::timeout_add_local(Duration::from_millis(150), move || {
                        match rx.try_recv() {
                            Ok((color, message)) => {
                                status(&result, &message, color);
                                glib::ControlFlow::Break
                            }
                            Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                            Err(mpsc::TryRecvError::Disconnected) => {
                                status(&result, "Installer status unavailable; inspect host before retrying.", "error");
                                glib::ControlFlow::Break
                            }
                        }
                    });
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
                    status(&result, "Desktop notifier repair in progress…", "warning");
                    let (tx, rx) = mpsc::channel();
                    thread::spawn(move || {
                        // Host shell expands its own HOME; never trust Flatpak HOME here.
                        let answer = host_command("/usr/bin/bash", &["-c",
                            "exec /usr/bin/bash \"$HOME/.local/share/ugc/astral-notifier-repair.sh\""])
                            .output().map(|output| (output.status.success(), format!("{}{}",
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
            let command = r#"sound="$HOME/.local/share/astral-guard-manager/siren.wav"
[ -f "$sound" ] || { echo "Siren file missing" >&2; exit 1; }
for player in pw-play paplay aplay; do
    if command -v "$player" >/dev/null 2>&1; then exec "$player" "$sound"; fi
done
echo "No supported audio player found" >&2
exit 1"#;
            host_background_action(
                result.clone(),
                "Siren test (no shutdown)",
                "/usr/bin/bash",
                vec!["-c".into(), command.into()],
            );
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
    let power_note = gtk::Label::new(Some(
        "Connector estimate = sum of six measured input currents × their voltages. This is not total GPU board power, and is not ASUS Cable Mileage.",
    ));
    power_note.set_wrap(true);
    power_note.set_xalign(0.0);
    power_note.add_css_class("dim-label");
    root.append(&power_note);
    let limits_note = gtk::Label::new(Some(
        "Display thresholds: current warning ≥ 9.2 A, critical ≥ 9.5 A per input; voltage warning < 11.4 V, critical < 11.0 V. These values are display-only; the independent guard uses its own configuration.",
    ));
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
    let check_note = gtk::Label::new(Some(
        "Configuration checks update every 5 seconds. ENABLED, CONFIGURED, and WIRED are not proof that emergency shutdown will work.",
    ));
    check_note.set_wrap(true);
    check_note.set_xalign(0.0);
    check_note.add_css_class("dim-label");
    root.append(&check_note);
    let disclaimer = gtk::Label::new(Some(
        "These checks do not prove that a critical-event shutdown will execute. A real critical-event shutdown is not simulated here.",
    ));
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
                            let display = match *name {
                                "Kernel driver loaded" => "LOADED",
                                "Guard executable installed" => "INSTALLED",
                                "Periodic guard timer" => "ACTIVE",
                                "Driver starts at boot" => "ENABLED",
                                "Sensor interface available" => "AVAILABLE",
                                "Emergency shutdown action" => "CONFIGURED",
                                "Critical-only trigger" => "WIRED",
                                "Kernel module matches boot" => "MATCHES",
                                _ => "VERIFIED",
                            };
                            status(label, display, "success");
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
