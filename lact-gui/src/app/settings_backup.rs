// Unified GPU Control settings backup format.
//
// Version 1 stores GUI preferences and saved GPU profiles.
// Importing a backup must never automatically activate GPU profiles
// or modify the Astral Guard emergency shutdown service.

use lact_schema::config::Profile;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const BACKUP_FORMAT_VERSION: u32 = 1;

#[derive(Debug, Serialize, Deserialize)]
pub struct SettingsBackup {
    pub format_version: u32,
    pub application: String,
    pub ui_config: serde_json::Value,
    pub default_profile: Option<Profile>,
    pub profiles: BTreeMap<String, Profile>,
    pub active_profile: Option<String>,
    pub auto_switch_profiles: bool,
}

impl SettingsBackup {
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.format_version == BACKUP_FORMAT_VERSION,
            "Unsupported settings backup version: {}",
            self.format_version
        );

        anyhow::ensure!(
            self.application == "Unified GPU Control",
            "This file is not a Unified GPU Control settings backup"
        );

        anyhow::ensure!(
            self.ui_config.is_object(),
            "Invalid GUI preferences in backup"
        );

        let _: crate::config::UiConfig = serde_json::from_value(self.ui_config.clone())
            .map_err(|err| anyhow::anyhow!("Invalid GUI preferences: {err}"))?;

        for name in self.profiles.keys() {
            anyhow::ensure!(
                !name.trim().is_empty(),
                "Backup contains an empty GPU profile name"
            );
        }

        Ok(())
    }

    pub fn profile_count(&self) -> usize {
        self.profiles.len() + usize::from(self.default_profile.is_some())
    }
}

/// Collect GUI preferences and all saved GPU profiles without changing
/// the currently active GPU configuration.
pub async fn collect_backup(client: &lact_client::DaemonClient) -> anyhow::Result<SettingsBackup> {
    use anyhow::Context;

    let profiles_info = client
        .list_profiles(false)
        .await
        .context("Could not retrieve GPU profile list")?;

    let default_profile = client
        .get_profile(None)
        .await
        .context("Could not retrieve default GPU profile")?;

    let mut profiles = BTreeMap::new();

    for name in profiles_info.profiles.keys() {
        let profile = client
            .get_profile(Some(name.clone()))
            .await
            .with_context(|| format!("Could not retrieve GPU profile: {name}"))?
            .with_context(|| format!("GPU profile disappeared: {name}"))?;

        profiles.insert(name.clone(), profile);
    }

    let ui_config = serde_json::to_value(&*crate::CONFIG.read())
        .context("Could not serialize GUI preferences")?;

    let backup = SettingsBackup {
        format_version: BACKUP_FORMAT_VERSION,
        application: "Unified GPU Control".to_owned(),
        ui_config,
        default_profile,
        profiles,
        active_profile: profiles_info.current_profile,
        auto_switch_profiles: profiles_info.auto_switch,
    };

    backup.validate()?;

    Ok(backup)
}

/// Restore GUI preferences only. Never modify daemon GPU settings.
pub fn restore_ui_preferences(backup: &SettingsBackup) -> anyhow::Result<()> {
    use anyhow::Context;

    backup.validate()?;

    let restored: crate::config::UiConfig = serde_json::from_value(backup.ui_config.clone())
        .context("Could not decode GUI preferences")?;

    let mut current = crate::CONFIG.write();

    current.edit(|config| {
        config.theme = restored.theme;
        config.color_scheme = restored.color_scheme;
        config.stats_poll_interval_ms = restored.stats_poll_interval_ms;
        config.window_size = restored.window_size;
        config.language = restored.language;
        config.plots_time_period = restored.plots_time_period;
        config.plots_per_row = restored.plots_per_row;
        config.gpus = restored.gpus;
    });

    Ok(())
}

/// Restore named profiles without replacing existing profiles or
/// enabling automatic switching rules and hooks.
pub async fn restore_profiles_safely(
    client: &lact_client::DaemonClient,
    backup: &SettingsBackup,
) -> anyhow::Result<usize> {
    use anyhow::Context;
    use lact_schema::request::ProfileBase;

    backup.validate()?;

    let existing = client
        .list_profiles(false)
        .await
        .context("Could not inspect existing GPU profiles")?;

    let mut reserved: std::collections::HashSet<String> =
        existing.profiles.keys().cloned().collect();

    let mut restored = 0;

    let profiles = backup
        .default_profile
        .iter()
        .map(|profile| ("Default", profile))
        .chain(
            backup
                .profiles
                .iter()
                .map(|(name, profile)| (name.as_str(), profile)),
        );

    for (name, original) in profiles {
        let mut profile = original.clone();

        // Never enable imported automatic profile rules or hooks.
        profile.rule = None;
        profile.hooks = Default::default();

        let base_name = format!("Imported - {name}");
        let mut new_name = base_name.clone();
        let mut suffix = 2;

        while reserved.contains(&new_name) {
            new_name = format!("{base_name} ({suffix})");
            suffix += 1;
        }

        client
            .create_profile(new_name.clone(), ProfileBase::Provided(profile))
            .await
            .with_context(|| format!("Could not restore profile: {name}"))?;

        reserved.insert(new_name);
        restored += 1;
    }

    Ok(restored)
}
