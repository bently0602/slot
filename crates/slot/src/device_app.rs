use std::path::PathBuf;
use std::time::{Duration, Instant};

use slot::frontend::Frontend;
use slot::input::DeviceInput;
use slot_gfx::{Compositor, FbdevSurface, Surface};
use slot_power::{trace_first_frame, DevicePlatform};

const CARD: &str = "/mnt/sdcard";

const MIN_FRAME: Duration = Duration::from_millis(12);

const STEP_TIMEOUT: Duration = Duration::from_millis(12);

const MARGIN: Duration = Duration::from_micros(1500);

struct Pacer {
    last_return: Option<Instant>,
    period: Duration,
    work: Duration,
    works: [Duration; WORK_WINDOW],
    next: usize,
}

const WORK_WINDOW: usize = 32;

impl Pacer {
    fn new() -> Self {
        Pacer {
            last_return: None,
            period: Duration::from_micros(16_760),
            work: Duration::from_millis(4),
            works: [Duration::from_millis(4); WORK_WINDOW],
            next: 0,
        }
    }

    fn wait(&self) {
        let Some(last) = self.last_return else {
            return;
        };
        let delay = self.period.saturating_sub(self.work + MARGIN);
        if let Some(left) = (last + delay).checked_duration_since(Instant::now()) {
            std::thread::sleep(left);
        }
    }

    fn swapped(&mut self, work: Duration, blocked: bool) -> bool {
        let now = Instant::now();
        let mut missed = false;
        if let Some(last) = self.last_return {
            let seen = now - last;
            missed = seen > self.period.mul_f64(1.5);
            if blocked && seen > Duration::from_millis(12) && seen < Duration::from_millis(22) {
                self.period = self.period.mul_f64(0.95) + seen.mul_f64(0.05);
            }
        }
        self.works[self.next] = work;
        self.next = (self.next + 1) % WORK_WINDOW;
        self.work = self.works.iter().copied().max().unwrap_or(work);
        self.last_return = Some(now);
        missed
    }
}

pub fn run() {
    if run_session() {
        // Restart the process to reset the vendor EGL framebuffer cache. BaseOS
        // launches through ld-linux explicitly, so retain that invocation too.
        use std::os::unix::process::CommandExt;
        let executable = match std::env::current_exe() {
            Ok(path) => path,
            Err(e) => {
                eprintln!("slot: HDMI restart executable: {e}");
                return;
            }
        };
        let mut command = std::process::Command::new(&executable);
        let args: Vec<_> = std::env::args_os().collect();
        if executable
            .file_name()
            .is_some_and(|name| name.to_string_lossy().starts_with("ld-linux"))
        {
            command.args(&args);
        } else {
            command.args(args.iter().skip(1));
        }
        eprintln!("slot: restarting display; current game saved");
        let error = command.exec();
        eprintln!("slot: HDMI restart: {error}");
    }
}

fn run_session() -> bool {
    slot::boot_time::mark("start");
    let _ = std::thread::Builder::new()
        .name("slot-fonts".into())
        .spawn(slot_ui::warm_fonts);
    let root = std::env::var_os("SLOT_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(CARD));
    let settings = slot_store::read_slot_state(&root);
    let mut cable = slot::hdmi::CableWatch::new(settings.hdmi_auto);
    let hdmi = match slot::hdmi::prepare(&settings) {
        Ok(size) => size,
        Err(e) => {
            eprintln!("slot: video output setup: {e}");
            return false;
        }
    };
    let route = slot::audio::AudioRoute {
        output: settings.audio_output,
        docked: hdmi.is_some(),
    };
    let open_audio = move || {
        let mut sink = slot::audio::open_sink();
        sink.configure_route(route);
        if let Err(e) = sink.open(slot::audio::GBA_HZ) {
            eprintln!("slot: audio: {e}");
        }
        sink
    };
    let audio = std::thread::Builder::new()
        .name("slot-audio-open".into())
        .spawn(open_audio);
    let mut surface = match hdmi.map_or_else(FbdevSurface::new, FbdevSurface::open) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("slot: {e}");
            return false;
        }
    };
    slot::boot_time::mark("surface");
    let mut compositor = match Compositor::new(&surface) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("slot: {e}");
            return false;
        }
    };
    slot::boot_time::mark("compositor");
    let platform = DevicePlatform::new(root.clone());
    eprintln!("slot: {}", platform.report());
    platform.trace_boot();
    slot::boot_time::mark("platform");
    let sink = match audio.map(|h| h.join()) {
        Ok(Ok(sink)) => sink,
        _ => open_audio(),
    };
    slot::boot_time::mark("audio");
    let mut frontend = Frontend::boot_with(Box::new(platform), sink);
    let mut docked = hdmi.is_some();
    frontend.set_docked(docked);
    slot::boot_time::mark("frontend");
    frontend.upload_boot_faces(&mut compositor);
    slot::boot_time::mark("faces");
    frontend.upload_bezel(&mut compositor, surface.window_size());
    slot::boot_time::mark("bezel");
    let mut input = DeviceInput::open(&root);
    slot::boot_time::mark("input");
    let card = root.clone();
    let _ = std::thread::Builder::new()
        .name("slot-bootlogo".into())
        .spawn(move || slot::bootlogo::refresh(&card));
    let mut drawn = false;
    frontend.drive_emulator();
    let mut pacer = Pacer::new();
    let (mut frames, mut missed, mut early) = (0u32, 0u32, 0u32);
    loop {
        pacer.wait();
        let began = Instant::now();
        frontend.advance(&mut input);
        if frontend.restarting() {
            frontend.restart();
        }
        if frontend.powering_off() {
            frontend.poweroff();
            return false;
        }
        let changed = cable.changed();
        let active = hdmi.is_some() && cable.present();
        if active != docked {
            docked = active;
            frontend.set_docked(docked);
        }
        if changed {
            match frontend.checkpoint_for_display() {
                Ok(true) => return true,
                Ok(false) => {}
                Err(e) => {
                    let wait = cable.defer();
                    eprintln!(
                        "slot: HDMI change deferred: {e}; retrying in {} s",
                        wait.as_secs()
                    );
                }
            }
        }
        frontend.step_emulator(pacer.period, STEP_TIMEOUT);
        compositor.set_hdmi_scaling(hdmi.map(|_| frontend.hdmi_scaling()));
        frontend.render(&mut compositor, surface.window_size());
        let swap = Instant::now();
        let work = swap - began;
        if let Err(e) = surface.swap() {
            eprintln!("slot: {e}");
            return false;
        }
        let swap_took = swap.elapsed();
        slot::latency::swapped(swap_took.as_secs_f64() * 1000.0);
        let dropped = pacer.swapped(work, swap_took > Duration::from_millis(1));
        if slot::latency::tracing() {
            frames += 1;
            missed += u32::from(dropped);
            early += u32::from(swap_took > pacer.period / 2);
            if frames == 600 {
                eprintln!(
                    "slot: pace: 600 frames, {missed} missed a latch, {early} drawn early, work {:.1} ms, period {:.2} ms",
                    pacer.work.as_secs_f64() * 1e3,
                    pacer.period.as_secs_f64() * 1e3
                );
                (frames, missed, early) = (0, 0, 0);
            }
        }
        if !drawn {
            drawn = true;
            slot::boot_time::mark("first frame");
            trace_first_frame();
        }
        if let Some(left) = MIN_FRAME.checked_sub(began.elapsed()) {
            std::thread::sleep(left);
        }
    }
}
