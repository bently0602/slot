use std::path::Path;

use slot_store::ini;

pub const RUN_AHEAD_FILE: &str = "Config/run_ahead.txt";

pub const RUN_AHEAD_MAX: u8 = 2;

pub fn run_ahead_for(root: &Path, stem: &str) -> u8 {
    ini::value(root, RUN_AHEAD_FILE, stem)
        .and_then(|v| v.parse::<u8>().ok())
        .filter(|frames| *frames <= RUN_AHEAD_MAX)
        .unwrap_or(0)
}

pub fn write_run_ahead(root: &Path, stem: &str, frames: u8) -> std::io::Result<()> {
    ini::write(root, RUN_AHEAD_FILE, stem, &frames.to_string())
}

pub fn label(frames: u8) -> &'static str {
    match frames {
        0 => "Run Ahead: Off",
        1 => "Run Ahead: 1 Frame",
        _ => "Run Ahead: 2 Frames",
    }
}
