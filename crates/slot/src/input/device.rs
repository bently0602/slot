use std::ffi::{c_int, c_ulong, c_void};
use std::fs::File;
use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;
use std::time::Instant;

use slot_input::{
    Btn, ControllerConfig, ControllerInfo, ControllerStatus, InputSource, Millis, RawEvent,
};

use super::controller::{AxisRange, Controller};
use super::controller_hub::ControllerHub;
use super::evdev::{
    controller_identity, decode, device_name, is_controller, pick_devices, to_raw, xinput_layout,
    Ev, Hat, EVENT_BYTES, EV_ABS, EV_KEY, EV_SW, EV_SYN,
};
use super::mux::InputMux;
use super::trace;

const DEV: &str = "/dev/input";
const SYS: &str = "/sys/class/input";
const PSY: &str = "/sys/class/power_supply";

const LID_POLL_MS: Millis = 150;
const EXTERNAL: u64 = u64::MAX;

#[derive(Default)]
struct Shared {
    pending: Vec<RawEvent>,
    config: ControllerConfig,
    status: ControllerStatus,
}

pub struct DeviceInput {
    pending: Arc<Mutex<Shared>>,
    status: ControllerStatus,
    hall: Option<PathBuf>,
    lid_shut: Option<bool>,
    next_lid_poll: Millis,
    stopped: Arc<AtomicBool>,
    reader: Option<JoinHandle<()>>,
}

impl DeviceInput {
    pub fn open(root: &Path) -> Self {
        DeviceInput::open_in(Path::new(DEV), Path::new(SYS), root, trace::enabled())
    }

    pub fn open_in(dev: &Path, sys: &Path, root: &Path, trace: bool) -> Self {
        let trace = trace.then(|| Trace::start(root, dev, sys)).flatten();
        DeviceInput::open_traced(dev, sys, trace, slot_store::read_controller_config(root))
    }

    fn open_traced(
        dev: &Path,
        sys: &Path,
        trace: Option<Arc<Trace>>,
        config: ControllerConfig,
    ) -> Self {
        let pending = Arc::new(Mutex::new(Shared {
            config: config.clone(),
            ..Shared::default()
        }));
        let stopped = Arc::new(AtomicBool::new(false));
        let mut hub = ControllerHub::default();
        hub.configure(&config);
        let mut worker = Worker {
            dev: dev.to_path_buf(),
            sys: sys.to_path_buf(),
            nodes: Vec::new(),
            mux: InputMux::default(),
            hub,
            config,
            next_id: 0,
            trace,
        };
        worker.scan(&pending);
        let queue = pending.clone();
        let stop = stopped.clone();
        let reader = std::thread::Builder::new()
            .name("slot-input".into())
            .spawn(move || worker.run(&queue, &stop))
            .map_err(|e| eprintln!("slot: input thread: {e}"))
            .ok();
        DeviceInput {
            pending,
            status: ControllerStatus::default(),
            hall: find_hall(Path::new(PSY)),
            lid_shut: None,
            next_lid_poll: 0,
            stopped,
            reader,
        }
    }
}

impl Drop for DeviceInput {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Relaxed);
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}

const POLL_MS: u32 = 10;

fn quicken_poll(sys: &Path, node: &Path) {
    let Some(name) = node.file_name() else {
        return;
    };
    let attr = sys.join(name).join("device/poll");
    let Some(now) = std::fs::read_to_string(&attr)
        .ok()
        .and_then(|s| s.trim().parse::<u32>().ok())
    else {
        return;
    };
    if now > POLL_MS {
        match std::fs::write(&attr, POLL_MS.to_string()) {
            Ok(()) => eprintln!(
                "slot: input {}: poll {now} ms -> {POLL_MS} ms",
                node.display()
            ),
            Err(e) => eprintln!("slot: input {}: poll: {e}", node.display()),
        }
    }
}

#[repr(C)]
struct PollFd {
    fd: c_int,
    events: i16,
    revents: i16,
}

unsafe extern "C" {
    fn poll(fds: *mut PollFd, count: c_ulong, timeout: c_int) -> c_int;
    fn ioctl(fd: c_int, request: c_ulong, arg: *mut c_void) -> c_int;
}

fn query<T>(file: &File, request: c_ulong, value: &mut T) -> bool {
    unsafe { ioctl(file.as_raw_fd(), request, (value as *mut T).cast()) >= 0 }
}

struct Node {
    path: PathBuf,
    inode: u64,
    id: u64,
    file: File,
    controller: Option<Controller>,
    xinput: bool,
    deadzone: u8,
    hat: Hat,
    builtin: InputMux,
    dropped: bool,
}

impl Node {
    fn snapshot(&mut self) -> Vec<Btn> {
        let external = self.controller.is_some();
        self.controller = external.then(|| Controller::new(self.xinput));
        if let Some(controller) = &mut self.controller {
            controller.set_deadzone(self.deadzone);
        }
        self.hat = Hat::default();
        self.builtin = InputMux::default();
        let mut keys = [0u8; 96];
        if query(&self.file, 0x8060_4518, &mut keys) {
            // EVIOCGKEY(96)
            for code in 0..768u16 {
                if keys[code as usize / 8] & (1 << (code % 8)) != 0 {
                    self.feed(Ev {
                        kind: EV_KEY,
                        code,
                        value: 1,
                    });
                }
            }
        }
        // Use supported-key capabilities, not the current pressed state, to
        // distinguish old HID right-stick axes from analog-only triggers.
        let mut supported = [0u8; 96];
        query(&self.file, 0x8060_4521, &mut supported); // EVIOCGBIT(EV_KEY, 96)
        for code in [0u16, 1, 2, 5, 9, 10, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15] {
            let mut axis = [0i32; 6];
            if !query(&self.file, 0x8018_4540 + c_ulong::from(code), &mut axis) {
                continue;
            }
            if let Some(controller) = &mut self.controller {
                let digital = |key: usize| supported[key / 8] & (1 << (key % 8)) != 0;
                let trigger = match code {
                    2 | 10 if axis[1] >= 0 && !digital(0x138) => Some(Btn::L2),
                    5 | 9 if axis[1] >= 0 && !digital(0x139) => Some(Btn::R2),
                    0x12 => Some(Btn::R1),
                    0x13 => Some(Btn::L1),
                    0x14 => Some(Btn::R2),
                    0x15 => Some(Btn::L2),
                    _ => None,
                };
                if [0, 1].contains(&code) || trigger.is_some() {
                    controller.add_axis(
                        code,
                        AxisRange {
                            min: axis[1],
                            max: axis[2],
                            flat: axis[4],
                        },
                        trigger,
                    );
                }
            }
            self.feed(Ev {
                kind: EV_ABS,
                code,
                value: axis[0],
            });
        }
        let mut switches = [0u8; 8];
        if !external && query(&self.file, 0x8008_451b, &mut switches) && switches[0] & 1 != 0 {
            self.feed(Ev {
                kind: EV_SW,
                code: 0,
                value: 1,
            });
        }
        self.buttons()
    }

    fn feed(&mut self, ev: Ev) -> Vec<RawEvent> {
        if let Some(controller) = &mut self.controller {
            return controller.feed(ev);
        }
        let events = match ev.kind {
            EV_ABS => self.hat.feed(ev),
            _ => to_raw(ev).into_iter().collect(),
        };
        let mut out = Vec::new();
        for event in events {
            out.extend(self.builtin.feed(u64::from(ev.code), event));
        }
        out
    }

    fn buttons(&self) -> Vec<Btn> {
        match &self.controller {
            Some(c) => c.buttons(),
            None => self.builtin.buttons(),
        }
    }
}

struct Worker {
    dev: PathBuf,
    sys: PathBuf,
    nodes: Vec<Node>,
    mux: InputMux,
    hub: ControllerHub,
    config: ControllerConfig,
    next_id: u64,
    trace: Option<Arc<Trace>>,
}

fn enqueue(queue: &Mutex<Shared>, events: Vec<RawEvent>, status: ControllerStatus) {
    let mut shared = queue.lock().unwrap_or_else(|e| e.into_inner());
    shared.pending.extend(events);
    shared.status = status;
}

impl Worker {
    fn scan(&mut self, queue: &Mutex<Shared>) {
        let paths = pick_devices(&self.dev, &self.sys);
        let mut at = 0;
        while at < self.nodes.len() {
            let node = &self.nodes[at];
            let present = paths.contains(&node.path)
                && std::fs::metadata(&node.path).is_ok_and(|m| m.ino() == node.inode);
            if present {
                at += 1;
            } else {
                self.remove(at, queue);
            }
        }
        for path in paths {
            if self.nodes.iter().any(|n| n.path == path) {
                continue;
            }
            quicken_poll(&self.sys, &path);
            let file = match std::fs::OpenOptions::new()
                .read(true)
                .custom_flags(0x800)
                .open(&path)
            {
                // O_NONBLOCK
                Ok(file) => file,
                Err(e) => {
                    eprintln!("slot: input {}: {e}", path.display());
                    continue;
                }
            };
            let inode = match file.metadata() {
                Ok(m) => m.ino(),
                Err(_) => continue,
            };
            self.next_id += 1;
            let external = is_controller(&self.sys, &path);
            let xinput = external && xinput_layout(&self.sys, &path);
            if external {
                let info = ControllerInfo {
                    id: controller_identity(&self.sys, &path),
                    name: device_name(&self.sys, &path),
                };
                let changes = self.hub.connect(self.next_id, info);
                let events = self.external_events(changes);
                enqueue(queue, events, self.hub.status());
            }
            eprintln!(
                "slot: input connected {} ({}, {})",
                path.display(),
                device_name(&self.sys, &path),
                if external { "gamepad" } else { "built-in" }
            );
            let mut node = Node {
                path,
                inode,
                id: self.next_id,
                file,
                controller: external.then(|| Controller::new(xinput)),
                xinput,
                deadzone: self.hub.deadzone(self.next_id),
                hat: Hat::default(),
                builtin: InputMux::default(),
                dropped: false,
            };
            let held = node.snapshot();
            let events = if external {
                let changes = self.hub.set(node.id, held);
                self.external_events(changes)
            } else {
                self.mux.set(node.id, held)
            };
            enqueue(queue, events, self.hub.status());
            self.nodes.push(node);
        }
    }

    fn remove(&mut self, at: usize, queue: &Mutex<Shared>) {
        let node = self.nodes.remove(at);
        eprintln!("slot: input disconnected {}", node.path.display());
        let events = if node.controller.is_some() {
            let changes = self.hub.disconnect(node.id);
            self.external_events(changes)
        } else {
            self.mux.set(node.id, Vec::new())
        };
        enqueue(queue, events, self.hub.status());
    }

    fn external_events(&mut self, changes: Vec<RawEvent>) -> Vec<RawEvent> {
        changes
            .into_iter()
            .flat_map(|event| self.mux.feed(EXTERNAL, event))
            .collect()
    }

    fn run(&mut self, queue: &Mutex<Shared>, stopped: &AtomicBool) {
        let mut scan_at = Instant::now() + Duration::from_millis(500);
        let mut buffer = [0u8; EVENT_BYTES * 32];
        while !stopped.load(Ordering::Relaxed) {
            let config = queue
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .config
                .clone();
            if config != self.config {
                let changes = self.hub.configure(&config);
                let events = self.external_events(changes);
                enqueue(queue, events, self.hub.status());
                self.config = config;
                for node in &mut self.nodes {
                    if node.controller.is_some() {
                        node.deadzone = self.hub.deadzone(node.id);
                        let changes = self.hub.set(node.id, node.snapshot());
                        let events = changes
                            .into_iter()
                            .flat_map(|e| self.mux.feed(EXTERNAL, e))
                            .collect();
                        enqueue(queue, events, self.hub.status());
                    }
                }
            }
            if Instant::now() >= scan_at {
                self.scan(queue);
                scan_at = Instant::now() + Duration::from_millis(500);
            }
            let mut fds: Vec<PollFd> = self
                .nodes
                .iter()
                .map(|n| PollFd {
                    fd: n.file.as_raw_fd(),
                    events: 1,
                    revents: 0,
                })
                .collect();
            let ready = unsafe { poll(fds.as_mut_ptr(), fds.len() as c_ulong, 20) };
            if ready <= 0 {
                continue;
            }
            // Reverse iteration keeps fd indices valid when a disconnected node is removed.
            for at in (0..fds.len()).rev() {
                if fds[at].revents == 0 {
                    continue;
                }
                if fds[at].revents & (8 | 16 | 32) != 0 {
                    self.remove(at, queue);
                    continue;
                }
                let node = &mut self.nodes[at];
                let read = match node.file.read(&mut buffer) {
                    Ok(0) => {
                        self.remove(at, queue);
                        continue;
                    }
                    Ok(n) => n,
                    Err(e)
                        if [
                            std::io::ErrorKind::WouldBlock,
                            std::io::ErrorKind::Interrupted,
                        ]
                        .contains(&e.kind()) =>
                    {
                        continue
                    }
                    Err(e) => {
                        eprintln!("slot: input {}: {e}", node.path.display());
                        self.remove(at, queue);
                        continue;
                    }
                };
                for ev in buffer[..read].chunks_exact(EVENT_BYTES).filter_map(decode) {
                    if let Some(trace) = &self.trace {
                        trace.event(
                            &node.path.display().to_string(),
                            ev,
                            node.controller.is_some(),
                            node.xinput,
                        );
                    }
                    if ev.kind == EV_SYN && ev.code == 3 {
                        node.dropped = true;
                        continue;
                    }
                    if node.dropped {
                        if ev.kind == EV_SYN && ev.code == 0 {
                            node.dropped = false;
                            let held = node.snapshot();
                            let events = if node.controller.is_some() {
                                self.hub
                                    .set(node.id, held)
                                    .into_iter()
                                    .flat_map(|e| self.mux.feed(EXTERNAL, e))
                                    .collect()
                            } else {
                                self.mux.set(node.id, held)
                            };
                            enqueue(queue, events, self.hub.status());
                        }
                        continue;
                    }
                    let changes = node.feed(ev);
                    let (source, changes) = if node.controller.is_some() {
                        (EXTERNAL, self.hub.set(node.id, node.buttons()))
                    } else {
                        (node.id, changes)
                    };
                    for event in changes {
                        if matches!(event, RawEvent::Down(_)) {
                            crate::latency::read();
                        }
                        enqueue(queue, self.mux.feed(source, event), self.hub.status());
                    }
                }
            }
        }
    }
}

struct Trace {
    out: Mutex<File>,
    began: Instant,
}

impl Trace {
    fn start(root: &Path, dev: &Path, sys: &Path) -> Option<Arc<Trace>> {
        let path = root.join(trace::TRACE_FILE);
        let mut file = match File::create(&path) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("slot: {}: {e}", path.display());
                return None;
            }
        };
        for line in trace::survey(dev, sys) {
            let _ = writeln!(file, "{line}");
        }
        let _ = writeln!(file, "--");
        let _ = file.flush();
        Some(Arc::new(Trace {
            out: Mutex::new(file),
            began: Instant::now(),
        }))
    }

    fn event(&self, node: &str, ev: Ev, controller: bool, xinput: bool) {
        if ev.kind == EV_SYN {
            return;
        }
        let line = trace::mapped_event_line(
            node,
            self.began.elapsed().as_millis(),
            ev,
            controller,
            xinput,
        );
        let mut out = self.out.lock().unwrap_or_else(|e| e.into_inner());
        let _ = writeln!(out, "{line}");
        let _ = out.flush();
    }
}

impl DeviceInput {
    fn read_lid(&self) -> Option<bool> {
        let raw = std::fs::read_to_string(self.hall.as_ref()?).ok()?;
        match raw.trim() {
            "0" => Some(true),
            "1" => Some(false),
            _ => None,
        }
    }
}

impl InputSource for DeviceInput {
    fn configure_controllers(&mut self, config: &ControllerConfig) {
        self.pending
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .config = config.clone();
    }

    fn controller_status(&self) -> ControllerStatus {
        self.status.clone()
    }

    fn poll(&mut self, now: Millis) -> Vec<RawEvent> {
        let mut shared = self.pending.lock().unwrap_or_else(|e| e.into_inner());
        let mut out = std::mem::take(&mut shared.pending);
        self.status = shared.status.clone();
        drop(shared);
        if self.hall.is_some() && now >= self.next_lid_poll {
            self.next_lid_poll = now + LID_POLL_MS;
            if let Some(shut) = self.read_lid() {
                if self.lid_shut != Some(shut) {
                    self.lid_shut = Some(shut);
                    out.push(if shut {
                        RawEvent::Down(Btn::Lid)
                    } else {
                        RawEvent::Up(Btn::Lid)
                    });
                }
            }
        }
        out
    }
}

fn find_hall(psy: &Path) -> Option<PathBuf> {
    let mut supplies: Vec<PathBuf> = std::fs::read_dir(psy)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .collect();
    supplies.sort();
    supplies
        .into_iter()
        .map(|d| d.join("hallkey"))
        .find(|p| p.is_file())
}
