//! BaseOS Bluetooth headphones. Blocking commands run only on a worker thread.
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Headphones {
    pub address: [u8; 6],
    pub name: String,
    pub paired: bool,
    pub connected: bool,
}

pub fn address(text: &str) -> Option<[u8; 6]> {
    if text.len() != 17 {
        return None;
    }
    let parts: Vec<_> = text.split(':').collect();
    if parts.len() != 6 || parts.iter().any(|p| p.len() != 2) {
        return None;
    }
    let mut result = [0; 6];
    for (byte, part) in result.iter_mut().zip(parts) {
        *byte = u8::from_str_radix(part, 16).ok()?;
    }
    Some(result)
}

pub fn format_address(address: [u8; 6]) -> String {
    address.map(|b| format!("{b:02X}")).join(":")
}

// BlueZ versions vary in their ANSI colours and interactive prompt prefixes.
fn clean(text: &str) -> String {
    let mut result = String::new();
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            if chars.next() == Some('[') {
                for c in chars.by_ref() {
                    if ('@'..='~').contains(&c) {
                        break;
                    }
                }
            }
        } else if !c.is_control() || c == '\n' || c == '\t' {
            result.push(c);
        }
    }
    result
}

pub fn parse_devices(text: &str) -> Vec<Headphones> {
    let mut devices = Vec::new();
    for line in clean(text).lines() {
        let Some((_, rest)) = line.split_once("Device ") else {
            continue;
        };
        let Some((mac, name)) = rest.split_once(' ') else {
            continue;
        };
        let Some(address) = address(mac) else {
            continue;
        };
        if !devices.iter().any(|d: &Headphones| d.address == address) {
            devices.push(Headphones {
                address,
                name: name.trim().chars().take(64).collect(),
                paired: false,
                connected: false,
            });
        }
    }
    devices
}

pub fn parse_info(mut device: Headphones, text: &str) -> Option<Headphones> {
    device.paired = false;
    device.connected = false;
    let text = clean(text);
    // Audio Sink is the remote A2DP playback service, not a controller or mic.
    let audio = text.to_ascii_lowercase();
    if !audio.contains("0000110b-0000-1000-8000-00805f9b34fb")
        && !audio.contains("icon: audio-headset")
        && !audio.contains("icon: audio-headphones")
    {
        return None;
    }
    for line in text.lines().map(str::trim) {
        if let Some(name) = line.strip_prefix("Alias: ") {
            device.name = name.chars().take(64).collect();
        }
        if line == "Paired: yes" || line == "Bonded: yes" {
            device.paired = true;
        }
        if line == "Connected: yes" {
            device.connected = true;
        }
    }
    Some(device)
}

#[derive(Clone, Copy, Debug)]
pub enum Operation {
    Scan,
    Connect,
    Disconnect,
    Forget,
    Refresh,
}

struct ResultState {
    devices: Vec<Headphones>,
    message: String,
    forgotten: bool,
}

pub struct Bluetooth {
    root: Option<PathBuf>,
    pub devices: Vec<Headphones>,
    pub selected: Option<[u8; 6]>,
    // Browsing another headset must not move audio away from the current one.
    pub active: Option<[u8; 6]>,
    pub status: String,
    pending: Option<(
        Operation,
        Option<[u8; 6]>,
        Receiver<Result<ResultState, String>>,
    )>,
    next_poll: Instant,
}

impl Default for Bluetooth {
    fn default() -> Self {
        Self {
            root: None,
            devices: Vec::new(),
            selected: None,
            active: None,
            status: "Scan to find headphones".into(),
            pending: None,
            next_poll: Instant::now(),
        }
    }
}

impl Bluetooth {
    pub fn boot(root: &Path) -> Self {
        let selected = std::fs::read_to_string(root.join("Config/bluetooth.txt"))
            .ok()
            .and_then(|s| address(s.trim()));
        let mut bt = Self {
            root: Some(root.into()),
            selected,
            ..Self::default()
        };
        if let Some(mac) = selected {
            bt.devices.push(Headphones {
                address: mac,
                name: format_address(mac),
                paired: true,
                connected: false,
            });
            #[cfg(all(feature = "device", not(feature = "host"), target_os = "linux"))]
            bt.request(Operation::Connect);
        }
        bt
    }

    pub fn busy(&self) -> bool {
        self.pending.is_some()
    }

    pub fn text(&self) -> String {
        self.devices
            .iter()
            .find(|d| Some(d.address) == self.selected)
            .map(|d| d.name.chars().take(20).collect())
            .unwrap_or_else(|| "None found".into())
    }

    pub fn step(&mut self, right: bool) {
        if self.busy() || self.devices.is_empty() {
            return;
        }
        let at = self
            .devices
            .iter()
            .position(|d| Some(d.address) == self.selected)
            .unwrap_or(0);
        let count = self.devices.len();
        self.selected =
            Some(self.devices[(at + if right { 1 } else { count - 1 }) % count].address);
    }

    pub fn request(&mut self, op: Operation) {
        if self.busy() {
            return;
        }
        if !cfg!(all(
            feature = "device",
            not(feature = "host"),
            target_os = "linux"
        )) {
            self.status = "Pair in computer settings".into();
            return;
        }
        let Some(root) = self.root.clone() else {
            return;
        };
        let selected = if matches!(op, Operation::Refresh) {
            self.active
        } else {
            self.selected
        };
        if !matches!(op, Operation::Scan) && selected.is_none() {
            self.status = "Scan and select headphones".into();
            return;
        }
        if !matches!(op, Operation::Refresh) {
            self.status = match op {
                Operation::Scan => "Scanning (15 seconds)",
                Operation::Connect => "Connecting...",
                Operation::Disconnect => "Disconnecting...",
                Operation::Forget => "Forgetting...",
                Operation::Refresh => "Checking connection...",
            }
            .into();
        }
        let (tx, rx) = mpsc::channel();
        let known = self.devices.clone();
        match std::thread::Builder::new()
            .name("slot-bluetooth".into())
            .spawn(move || {
                let result = operate(&root, op, selected, known);
                let _ = tx.send(result);
            }) {
            Ok(_) => self.pending = Some((op, selected, rx)),
            Err(e) => self.status = format!("Bluetooth worker: {e}"),
        }
    }

    pub fn poll(&mut self) {
        if let Some((op, target, rx)) = &self.pending {
            let (op, target) = (*op, *target);
            let result = match rx.try_recv() {
                Ok(result) => result,
                Err(mpsc::TryRecvError::Empty) => return,
                Err(mpsc::TryRecvError::Disconnected) => Err("Bluetooth worker stopped".into()),
            };
            self.pending = None;
            self.next_poll = Instant::now() + Duration::from_secs(3);
            match result {
                Ok(result) => {
                    let active = if matches!(op, Operation::Connect) {
                        target
                    } else {
                        self.active
                    };
                    self.active = active.filter(|mac| {
                        result
                            .devices
                            .iter()
                            .any(|d| d.connected && d.address == *mac)
                    });
                    self.devices = result.devices;
                    if !self
                        .devices
                        .iter()
                        .any(|d| Some(d.address) == self.selected)
                    {
                        self.selected = self.devices.first().map(|d| d.address);
                    }
                    self.status = result.message;
                    if result.forgotten || matches!(op, Operation::Disconnect) {
                        // Explicit disconnect also disables reconnect at the next boot.
                        self.save(self.active);
                    } else if matches!(op, Operation::Connect) {
                        self.save(self.active);
                    }
                }
                Err(error) => {
                    eprintln!("slot: Bluetooth: {error}");
                    self.status = error;
                    if matches!(op, Operation::Refresh) {
                        self.active = None;
                    }
                }
            }
        } else if self.active.is_some() && Instant::now() >= self.next_poll {
            self.request(Operation::Refresh);
        }
    }

    fn save(&mut self, mac: Option<[u8; 6]>) {
        let Some(root) = &self.root else {
            return;
        };
        let path = root.join("Config/bluetooth.txt");
        let result = (|| -> std::io::Result<()> {
            std::fs::create_dir_all(path.parent().unwrap())?;
            let tmp = path.with_extension("tmp");
            std::fs::write(&tmp, mac.map(format_address).unwrap_or_default())?;
            std::fs::rename(tmp, path)
        })();
        if let Err(e) = result {
            self.status = "Could not save headphones".into();
            eprintln!("slot: Bluetooth settings: {e}");
        }
    }
}

// Direct arguments, validated MACs and bounded stdout/stderr: names never become
// shell commands. Enforce our own deadline even on firmware without timeout(1).
fn run(mut command: Command, seconds: u64) -> Result<String, String> {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command.env("LC_ALL", "C").env("TERM", "dumb");
    let mut child = command
        .spawn()
        .map_err(|e| format!("Bluetooth tools unavailable: {e}"))?;
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let readers = [Box::new(stdout) as Box<dyn Read + Send>, Box::new(stderr)].map(|reader| {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            let _ = reader.take(131_072).read_to_end(&mut bytes);
            let _ = tx.send(bytes);
        });
        rx
    });
    let deadline = Instant::now() + Duration::from_secs(seconds);
    let status: Result<std::process::ExitStatus, String> = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(25)),
            _ => {
                kill_command(&mut child);
                let _ = child.wait();
                break Err("Bluetooth command timed out".into());
            }
        }
    };
    let mut output = Vec::new();
    for reader in readers {
        match reader.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            Ok(bytes) => output.extend(bytes),
            Err(_) => {
                kill_command(&mut child);
                return Err("Bluetooth command timed out".into());
            }
        }
    }
    let text = clean(&String::from_utf8_lossy(&output));
    if !status?.success()
        || text.contains("Failed to")
        || text.contains("No default controller")
        || text.contains("not available")
    {
        return Err(text
            .lines()
            .find(|l| {
                l.contains("Failed") || l.contains("not available") || l.contains("No default")
            })
            .or_else(|| text.lines().rev().find(|l| !l.trim().is_empty()))
            .unwrap_or("Bluetooth operation failed; see slot.log")
            .chars()
            .take(100)
            .collect());
    }
    Ok(text)
}

fn kill_command(child: &mut std::process::Child) {
    // Kill descendants too: a firmware helper must not outlive an expired job
    // while retaining our output pipes. Successful daemon startup is unaffected.
    #[cfg(unix)]
    unsafe {
        unsafe extern "C" {
            fn kill(pid: i32, signal: i32) -> i32;
        }
        kill(-(child.id() as i32), 9);
    }
    let _ = child.kill();
}

fn ctl(args: &[&str], seconds: u64) -> Result<String, String> {
    let mut command = Command::new("bluetoothctl");
    command.args(args);
    run(command, seconds + 2)
}

fn inventory() -> Result<Vec<Headphones>, String> {
    let devices = parse_devices(&ctl(&["devices"], 5)?);
    let mut headphones = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(30);
    for device in devices.into_iter().take(32) {
        if Instant::now() >= deadline {
            break;
        }
        let mac = format_address(device.address);
        if let Ok(info) = ctl(&["info", &mac], 5) {
            if let Some(device) = parse_info(device, &info) {
                headphones.push(device);
            }
        }
    }
    headphones.sort_by(|a, b| {
        b.connected
            .cmp(&a.connected)
            .then(b.paired.cmp(&a.paired))
            .then(a.name.cmp(&b.name))
    });
    Ok(headphones)
}

fn operate(
    root: &Path,
    op: Operation,
    selected: Option<[u8; 6]>,
    mut known: Vec<Headphones>,
) -> Result<ResultState, String> {
    if matches!(op, Operation::Refresh) {
        let mac = selected.ok_or("No headphones selected")?;
        let text = ctl(&["info", &format_address(mac)], 3)?;
        let device = Headphones {
            address: mac,
            name: format_address(mac),
            paired: false,
            connected: false,
        };
        let updated = parse_info(device, &text).ok_or("Headphones unavailable")?;
        let message = if updated.connected {
            "Connected"
        } else {
            "Connection lost"
        }
        .into();
        known.retain(|d| d.address != mac);
        known.push(updated);
        return Ok(ResultState {
            devices: known,
            message,
            forgotten: false,
        });
    }
    if matches!(op, Operation::Scan | Operation::Connect) {
        let script = root.join("System/bluetooth.sh");
        if !script.is_file() {
            return Err("Rebuild Bluetooth bundle".into());
        }
        let mut command = Command::new("/bin/sh");
        command.arg(script);
        run(command, 25)?;
        // BlueZ may take a moment to register the freshly attached radio.
        let mut ready = false;
        for _ in 0..5 {
            if ctl(&["show"], 3).is_ok() {
                ready = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(500));
        }
        if !ready {
            return Err("Bluetooth radio unavailable".into());
        }
        ctl(&["power", "on"], 5)?;
    }
    let mac = selected.map(format_address).unwrap_or_default();
    match op {
        Operation::Scan => {
            ctl(&["--timeout", "15", "scan", "on"], 18)?;
        }
        Operation::Connect => {
            let info = ctl(&["info", &mac], 5)?;
            let candidate = Headphones {
                address: selected.unwrap(),
                name: mac.clone(),
                paired: false,
                connected: false,
            };
            if parse_info(candidate, &info).is_none() {
                return Err("Device is not A2DP headphones".into());
            }
            if !info.contains("Paired: yes") && !info.contains("Bonded: yes") {
                ctl(
                    &[
                        "--agent",
                        "NoInputNoOutput",
                        "--timeout",
                        "30",
                        "pair",
                        &mac,
                    ],
                    32,
                )?;
            }
            ctl(&["trust", &mac], 5)?;
            ctl(&["--timeout", "15", "connect", &mac], 17)?;
        }
        Operation::Disconnect => {
            ctl(&["disconnect", &mac], 10)?;
        }
        Operation::Forget => {
            ctl(&["remove", &mac], 10)?;
        }
        Operation::Refresh => {}
    }
    let devices = if matches!(op, Operation::Scan) {
        inventory()?
    } else {
        // Operation completion depends on this device, never on a crowded radio's inventory.
        known.retain(|d| Some(d.address) != selected);
        if !matches!(op, Operation::Forget) {
            let info = ctl(&["info", &mac], 5)?;
            let device = Headphones {
                address: selected.unwrap(),
                name: mac,
                paired: false,
                connected: false,
            };
            known.push(parse_info(device, &info).ok_or("Device has no A2DP audio")?);
        }
        known
    };
    let connected = devices
        .iter()
        .any(|d| d.connected && Some(d.address) == selected);
    if matches!(op, Operation::Connect) && !connected {
        return Err("Headphones did not connect".into());
    }
    Ok(ResultState {
        message: match op {
            Operation::Scan if devices.is_empty() => "No A2DP headphones found",
            Operation::Scan => "Choose headphones, then Connect",
            Operation::Forget => "Pairing removed",
            Operation::Disconnect => "Disconnected",
            _ if connected => "Connected",
            _ => "Connection lost",
        }
        .into(),
        devices,
        forgotten: matches!(op, Operation::Forget),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAC: [u8; 6] = [0xAA, 0xBB, 0xCC, 0x11, 0x22, 0x33];
    fn device(connected: bool) -> Headphones {
        Headphones {
            address: MAC,
            name: "Headphones".into(),
            paired: true,
            connected,
        }
    }
    fn finish(bt: &mut Bluetooth, op: Operation, result: Result<ResultState, String>) {
        let (tx, rx) = mpsc::channel();
        tx.send(result).unwrap();
        bt.pending = Some((op, Some(MAC), rx));
        bt.poll();
    }

    #[test]
    fn addresses_reject_command_text_and_round_trip() {
        assert_eq!(address("aa:bb:cc:11:22:33"), Some(MAC));
        assert_eq!(format_address(MAC), "AA:BB:CC:11:22:33");
        for invalid in [
            "AA:BB:CC:11:22",
            "AA:BB:CC:11:22:GG",
            "AA:BB:CC:11:22:33;reboot",
            "$(reboot)",
            "ＡＡ:BB:CC:11:22:33",
        ] {
            assert_eq!(address(invalid), None);
        }
    }

    #[test]
    fn bluez_colour_prompts_and_duplicate_devices_are_parsed() {
        let devices = parse_devices("[bluetooth]# \x1b[0;94mDevice AA:BB:CC:11:22:33 My Headphones\x1b[0m\nDevice AA:BB:CC:11:22:33 Duplicate\nDevice invalid Ignore");
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].name, "My Headphones");
        let info = "Alias: My AirPods\nPaired: yes\nConnected: yes\nUUID: Audio Sink (0000110b-0000-1000-8000-00805f9b34fb)";
        let parsed = parse_info(devices[0].clone(), info).unwrap();
        assert!(parsed.paired && parsed.connected);
        assert_eq!(parsed.name, "My AirPods");
        assert!(parse_info(device(false), "Icon: input-gaming\nConnected: yes").is_none());
        assert!(parse_info(
            device(false),
            "UUID: Audio Source (0000110a-0000-1000-8000-00805f9b34fb)"
        )
        .is_none());
        // BlueZ can identify a headset before pairing completes service discovery.
        assert!(parse_info(device(false), "Icon: audio-headphones").is_some());
    }

    #[test]
    fn browsing_keeps_audio_on_the_connected_headphones_and_wraps() {
        let mut bt = Bluetooth::default();
        bt.devices = vec![
            device(true),
            Headphones {
                address: [1; 6],
                name: "Other".into(),
                paired: false,
                connected: false,
            },
        ];
        bt.selected = Some(MAC);
        bt.active = Some(MAC);
        bt.step(true);
        assert_eq!(bt.selected, Some([1; 6]));
        assert_eq!(bt.active, Some(MAC));
        bt.step(true);
        assert_eq!(bt.selected, Some(MAC));
        bt.step(false);
        assert_eq!(bt.selected, Some([1; 6]));
    }

    #[test]
    fn successful_connect_persists_and_explicit_disconnect_disables_boot_reconnect() {
        let root = tempfile::tempdir().unwrap();
        let mut bt = Bluetooth::boot(root.path());
        bt.selected = Some(MAC);
        finish(
            &mut bt,
            Operation::Connect,
            Ok(ResultState {
                devices: vec![device(true)],
                message: "Connected".into(),
                forgotten: false,
            }),
        );
        assert_eq!(bt.active, Some(MAC));
        assert_eq!(
            std::fs::read_to_string(root.path().join("Config/bluetooth.txt")).unwrap(),
            format_address(MAC)
        );
        finish(
            &mut bt,
            Operation::Disconnect,
            Ok(ResultState {
                devices: vec![device(false)],
                message: "Disconnected".into(),
                forgotten: false,
            }),
        );
        assert_eq!(bt.active, None);
        assert!(
            std::fs::read_to_string(root.path().join("Config/bluetooth.txt"))
                .unwrap()
                .is_empty()
        );
        assert_eq!(Bluetooth::boot(root.path()).selected, None);
    }

    #[test]
    fn failed_pairing_keeps_the_previous_route_but_failed_monitoring_clears_it() {
        let mut bt = Bluetooth::default();
        bt.active = Some(MAC);
        finish(&mut bt, Operation::Connect, Err("Pairing failed".into()));
        assert_eq!(bt.active, Some(MAC));
        assert!(!bt.busy());
        assert_eq!(bt.status, "Pairing failed");
        finish(&mut bt, Operation::Refresh, Err("Radio unavailable".into()));
        assert_eq!(bt.active, None);
    }

    #[test]
    fn polling_connection_does_not_rewrite_the_sd_card() {
        let root = tempfile::tempdir().unwrap();
        let mut bt = Bluetooth::boot(root.path());
        bt.active = Some(MAC);
        finish(
            &mut bt,
            Operation::Refresh,
            Ok(ResultState {
                devices: vec![device(true)],
                message: "Connected".into(),
                forgotten: false,
            }),
        );
        assert_eq!(bt.active, Some(MAC));
        assert!(!root.path().join("Config/bluetooth.txt").exists());
        finish(
            &mut bt,
            Operation::Refresh,
            Ok(ResultState {
                devices: vec![device(false)],
                message: "Connection lost".into(),
                forgotten: false,
            }),
        );
        assert_eq!(bt.active, None);
    }

    #[cfg(unix)]
    #[test]
    fn commands_detect_bluez_errors_even_with_success_exit_codes_and_bound_descendants() {
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "printf 'Failed to connect: org.bluez.Error.Failed\n'"]);
        assert!(run(command, 2).unwrap_err().contains("Failed to connect"));
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "sleep 20 & wait"]);
        let start = Instant::now();
        assert!(run(command, 1).unwrap_err().contains("timed out"));
        assert!(start.elapsed() < Duration::from_secs(3));
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "printf 'Connection successful\n'"]);
        assert_eq!(run(command, 2).unwrap().trim(), "Connection successful");
    }
}
