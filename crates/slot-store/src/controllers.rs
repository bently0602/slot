use slot_input::{ControllerConfig, ControllerProfile, STICK_DEADZONES};
use std::path::Path;

fn encode(value: &str) -> String {
    value
        .as_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn decode(value: &str) -> Option<String> {
    if !value.len().is_multiple_of(2) || !value.is_ascii() {
        return None;
    }
    let bytes: Option<Vec<_>> = (0..value.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&value[i..i + 2], 16).ok())
        .collect();
    String::from_utf8(bytes?).ok()
}

pub fn read_controller_config(root: &Path) -> ControllerConfig {
    let mut config = ControllerConfig::default();
    let text = std::fs::read_to_string(root.join("Config/controllers.conf")).unwrap_or_default();
    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match key {
            "preferred" => config.preferred = decode(value).filter(|s| !s.is_empty()),
            "pause_disconnect" => match value {
                "0" => config.pause_disconnect = false,
                "1" => config.pause_disconnect = true,
                _ => {}
            },
            "profile" => {
                let fields: Vec<_> = value.split(',').collect();
                if fields.len() != 5
                    || !["0", "1"].contains(&fields[2])
                    || !["0", "1"].contains(&fields[3])
                {
                    continue;
                }
                let (Some(id), Some(name), Ok(deadzone)) = (
                    decode(fields[0]),
                    decode(fields[1]),
                    fields[4].parse::<u8>(),
                ) else {
                    continue;
                };
                if id.is_empty() || !STICK_DEADZONES.contains(&deadzone) {
                    continue;
                }
                config.profiles.retain(|p| p.id != id);
                config.profiles.push(ControllerProfile {
                    id,
                    name,
                    swap_ab: fields[2] == "1",
                    swap_xy: fields[3] == "1",
                    deadzone,
                });
            }
            _ => {}
        }
    }
    config
}

pub fn write_controller_config(root: &Path, config: &ControllerConfig) -> std::io::Result<()> {
    std::fs::create_dir_all(root.join("Config"))?;
    let mut text = format!(
        "preferred={}\npause_disconnect={}\n",
        encode(config.preferred.as_deref().unwrap_or("")),
        config.pause_disconnect as u8
    );
    for profile in &config.profiles {
        text.push_str(&format!(
            "profile={},{},{},{},{}\n",
            encode(&profile.id),
            encode(&profile.name),
            profile.swap_ab as u8,
            profile.swap_xy as u8,
            profile.deadzone
        ));
    }
    crate::atomic_write(&root.join("Config/controllers.conf"), text.as_bytes())
}
