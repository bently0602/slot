use slot_store::Platform;
use std::ffi::OsString;
use std::path::Path;

pub struct GameReset {
    pub root: std::path::PathBuf,
    pub cart: String,
    pub platform: Platform,
}

pub fn reset_target(args: &[OsString]) -> Option<(String, Platform)> {
    let at = args.iter().position(|arg| arg == "--reset-game")?;
    let platform = Platform::ALL.into_iter().find(|platform| {
        args.get(at + 1)
            .is_some_and(|arg| arg == platform.dir_name())
    })?;
    Some((args.get(at + 2)?.to_str()?.to_string(), platform))
}

/// Build the same invocation on native launches and BaseOS's explicit loader.
pub fn game_reset_command(
    executable: &Path,
    args: &[OsString],
    reset: &GameReset,
) -> std::process::Command {
    let mut command = std::process::Command::new(executable);
    let args = without_return_flags(args.iter().cloned());
    if executable
        .file_name()
        .is_some_and(|name| name.to_string_lossy().starts_with("ld-linux"))
    {
        command.args(args);
    } else {
        command.args(args.into_iter().skip(1));
    }
    command.args(["--reset-game", reset.platform.dir_name(), &reset.cart]);
    command.env("SLOT_ROOT", &reset.root);
    command.env_remove("SLOT_MUSIC_DISPLAY_RESUME");
    command
}

/// Replace the whole process: a wedged native core cannot safely be cancelled or
/// unloaded on another thread. Do not drop Session (which joins that worker),
/// flush a snapshot, or save RAM from the possibly damaged emulation state.
pub fn reset_game(reset: &GameReset) {
    let result: std::io::Result<()> = std::env::current_exe().and_then(|executable| {
        let args: Vec<_> = std::env::args_os().collect();
        let mut command = game_reset_command(&executable, &args, reset);
        eprintln!(
            "slot: resetting {} from the last on-disk game save",
            reset.cart
        );
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            Err(command.exec())
        }
        #[cfg(not(unix))]
        {
            command.spawn()?;
            std::process::exit(0);
        }
    });
    if let Err(error) = result {
        eprintln!("slot: game reset failed: {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reset_flags_are_consumed_before_later_app_or_display_restarts() {
        let args: Vec<OsString> = [
            "slot",
            "--reset-game",
            "GBA",
            "Emerald",
            "--return-device-info",
            "--device-info-sleep",
            "other",
        ]
        .into_iter()
        .map(OsString::from)
        .collect();
        assert_eq!(
            without_return_flags(args),
            [OsString::from("slot"), OsString::from("other")]
        );
    }
}

fn without_return_flags(args: impl IntoIterator<Item = OsString>) -> Vec<OsString> {
    let mut args = args.into_iter();
    let mut kept = Vec::new();
    while let Some(arg) = args.next() {
        if arg == "--reset-game" {
            args.next();
            args.next();
        } else if arg != "--return-device-info" && arg != "--device-info-sleep" {
            kept.push(arg);
        }
    }
    kept
}

/// Return-only flags apply to one boot, not subsequent app or display restarts.
pub fn restart_args() -> Vec<std::ffi::OsString> {
    without_return_flags(std::env::args_os())
}

/// Replace Slot after its window, audio and input have been dropped. The script
/// waits for Device Info and replaces itself with Slot even if the app fails.
#[cfg(unix)]
pub fn device_info(root: &Path) {
    use std::os::unix::process::CommandExt;

    let root = match root.canonicalize() {
        Ok(path) => path,
        Err(e) => {
            eprintln!("slot: Device Info root: {e}");
            return;
        }
    };
    let executable = match std::env::current_exe() {
        Ok(path) => path,
        Err(e) => {
            eprintln!("slot: Device Info executable: {e}");
            return;
        }
    };
    let mut command = std::process::Command::new("/bin/sh");
    command.arg(root.join("System/device-info.sh"));
    command.env("SLOT_ROOT", &root);
    let build = crate::build_info::Build::current();
    command.env("SLOT_VERSION", build.version);
    command.env("SLOT_BUILD", build.hash);
    // BaseOS invokes Slot through its loader. Preserve that invocation rather
    // than treating current_exe (the loader) as the frontend binary.
    command.arg(&executable);
    if executable
        .file_name()
        .is_some_and(|name| name.to_string_lossy().starts_with("ld-linux"))
    {
        command.args(restart_args());
    } else {
        command.args(restart_args().into_iter().skip(1));
    }
    let error = command.exec();
    eprintln!("slot: Device Info handoff: {error}");
}

#[cfg(not(unix))]
pub fn device_info(_root: &Path) {
    eprintln!("slot: Device Info requires Linux");
}
