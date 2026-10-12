use std::path::Path;

/// Return-only flags apply to one boot, not subsequent app or display restarts.
pub fn restart_args() -> Vec<std::ffi::OsString> {
    std::env::args_os()
        .filter(|arg| arg != "--return-device-info" && arg != "--device-info-sleep")
        .collect()
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
