//! RG SP / BaseOS sunxi disp2 setup. Run only before creating an EGL surface:
//! the vendor Mali driver captures framebuffer geometry at initialization.
use std::ffi::{c_int, c_ulong, c_void};
use std::fs::{self, File, OpenOptions};
use std::io;
use std::os::fd::AsRawFd;
use std::path::Path;
use std::thread::sleep;
use std::time::{Duration, Instant};

use slot_store::SlotState;

const DEBUG: &str = "/sys/kernel/debug/dispdbg";
const STATUS: &str = "/sys/class/disp/disp/attr/sys";
const BLANK: &str = "/sys/class/graphics/fb0/blank";

unsafe extern "C" {
    fn ioctl(fd: c_int, request: c_ulong, arg: *mut c_void) -> c_int;
}

fn control<T>(file: &File, request: c_ulong, arg: &mut T) -> io::Result<()> {
    let result = unsafe { ioctl(file.as_raw_fd(), request, (arg as *mut T).cast()) };
    if result < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

/// Linux fb_var_screeninfo, including the four reserved words (160 bytes).
#[repr(C)]
struct FramebufferInfo([u32; 40]);

impl Default for FramebufferInfo {
    fn default() -> Self {
        Self([0; 40])
    }
}

// sunxi_display2.h UAPI. Keep native C padding and 64-bit addresses/crops;
// fb0 uses channel 1, layer 0 on the H700 BSP.
#[repr(C)]
#[derive(Default, Clone, Copy)]
struct Size {
    width: u32,
    height: u32,
}
#[repr(C)]
#[derive(Default)]
struct Rect {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}
#[repr(C)]
#[derive(Default)]
struct Crop {
    x: i64,
    y: i64,
    width: i64,
    height: i64,
}
#[repr(C)]
#[derive(Default)]
struct Buffer {
    address: [u64; 3],
    size: [Size; 3],
    align: [u32; 3],
    format: i32,
    color_space: i32,
    right_address: [u32; 3],
    premultiply: u8,
    crop: Crop,
    flags: i32,
    scan: i32,
}
#[repr(C)]
#[derive(Default)]
struct LayerInfo {
    mode: i32,
    zorder: u8,
    alpha_mode: u8,
    alpha_value: u8,
    window: Rect,
    stereo: u8,
    stereo_mode: i32,
    buffer: Buffer,
    id: u32,
}
#[repr(C)]
#[derive(Default)]
struct Layer {
    info: LayerInfo,
    enabled: u8,
    channel: u32,
    id: u32,
}

pub fn cable_state(text: &str) -> bool {
    text.trim() == "1"
        || text.split_whitespace().any(|field| {
            field
                .split_once('=')
                .is_some_and(|(key, value)| key.eq_ignore_ascii_case("hdmi") && value == "1")
        })
}

pub fn connected() -> bool {
    [
        "/sys/class/extcon/hdmi/state",
        "/sys/class/extcon/hdmi/cable.0/state",
        "/sys/class/extcon/hdmi/cable.1/state",
    ]
    .iter()
    .any(|path| fs::read_to_string(path).is_ok_and(|s| cable_state(&s)))
}

/// Debounce a cable change before restarting graphics. The caller keeps the
/// change pending until the current game can be saved safely.
pub struct CableWatch {
    current: bool,
    seen: bool,
    since: Instant,
    next: Instant,
    automatic: bool,
    failures: u32,
}

const RETRY_FIRST: Duration = Duration::from_secs(5);
const RETRY_MAX: Duration = Duration::from_secs(60);

impl CableWatch {
    pub fn new(automatic: bool) -> Self {
        let current = connected();
        Self {
            current,
            seen: current,
            since: Instant::now(),
            next: Instant::now(),
            automatic,
            failures: 0,
        }
    }

    /// The save for a pending change failed. Wait longer before each retry so
    /// a full or failing card is not written to twice a second.
    pub fn defer(&mut self) -> Duration {
        self.defer_at(Instant::now())
    }

    fn defer_at(&mut self, now: Instant) -> Duration {
        let wait = RETRY_FIRST
            .saturating_mul(1 << self.failures.min(4))
            .min(RETRY_MAX);
        self.failures += 1;
        self.next = now + wait;
        wait
    }

    pub fn changed(&mut self) -> bool {
        let now = Instant::now();
        if !self.automatic || now < self.next {
            return false;
        }
        self.observe(connected(), now)
    }

    pub fn present(&self) -> bool {
        self.present_at(Instant::now())
    }

    fn present_at(&self, now: Instant) -> bool {
        if now.saturating_duration_since(self.since) >= Duration::from_millis(500) {
            self.seen
        } else {
            self.current
        }
    }

    fn observe(&mut self, cable: bool, now: Instant) -> bool {
        if !self.automatic || now < self.next {
            return false;
        }
        self.next = now + Duration::from_millis(500);
        if cable != self.seen {
            self.seen = cable;
            self.since = now;
        }
        if self.seen == self.current {
            self.failures = 0;
        }
        self.seen != self.current && now.duration_since(self.since) >= Duration::from_millis(500)
    }
}

fn active() -> io::Result<bool> {
    Ok(fs::read_to_string(STATUS)?.contains("hdmi output"))
}

pub fn switch_parameter(hdmi: bool, full_hd: bool) -> &'static str {
    match (hdmi, full_hd) {
        (false, _) => "1 0",
        (true, false) => "4 5 0 0 0x4 0x101 0 0 0 8",
        (true, true) => "4 10 0 0 0x4 0x101 0 0 0 8",
    }
}

fn switch(hdmi: bool, full_hd: bool) -> io::Result<()> {
    for (file, value) in [
        ("name", "disp0"),
        ("command", "switch"),
        ("param", switch_parameter(hdmi, full_hd)),
        ("start", "1"),
    ] {
        fs::write(Path::new(DEBUG).join(file), value)?;
    }
    let deadline = Instant::now() + Duration::from_secs(3);
    while active()? != hdmi {
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "display attach timed out",
            ));
        }
        sleep(Duration::from_millis(50));
    }
    sleep(Duration::from_millis(150));
    Ok(())
}

fn resize(file: &File, size: (u32, u32)) -> io::Result<()> {
    let mut info = FramebufferInfo::default();
    control(file, 0x4600, &mut info)?;
    info.0[..4].fill(0);
    info.0[6] = 32;
    let _ = control(file, 0x4601, &mut info);
    sleep(Duration::from_millis(250));
    info.0[..7].copy_from_slice(&[size.0, size.1, size.0, size.1 * 2, 0, 0, 32]);
    info.0[21] = 128; // FB_ACTIVATE_NOW | FB_ACTIVATE_FORCE
    control(file, 0x4601, &mut info)
}

fn layer_geometry(file: &File, buffer: (u32, u32), output: (u32, u32)) -> io::Result<()> {
    for _ in 0..20 {
        let mut layer = Layer {
            channel: 1,
            ..Layer::default()
        };
        let mut args = [0usize, (&mut layer as *mut Layer) as usize, 1, 0];
        control(file, 0x48, &mut args)?; // DISP_LAYER_GET_CONFIG
        layer.enabled = 1;
        layer.info.buffer.size.fill(Size {
            width: buffer.0,
            height: buffer.1 * 2,
        });
        layer.info.buffer.crop = Crop {
            x: 0,
            y: 0,
            width: i64::from(buffer.0) << 32,
            height: i64::from(buffer.1) << 32,
        };
        layer.info.window = Rect {
            x: 0,
            y: 0,
            width: output.0,
            height: output.1,
        };
        control(file, 0x47, &mut args)?; // DISP_LAYER_SET_CONFIG
        sleep(Duration::from_millis(100));
        control(file, 0x48, &mut args)?;
        if layer.info.buffer.size[0].width == buffer.0
            && layer.info.buffer.size[0].height == buffer.1 * 2
            && layer.info.window.width == output.0
            && layer.info.window.height == output.1
            && layer.info.buffer.crop.y == 0
        {
            return Ok(());
        }
    }
    Err(io::Error::new(
        io::ErrorKind::TimedOut,
        "framebuffer layer geometry did not settle",
    ))
}

fn apply(fb: &File, disp: &File, hdmi: bool, full_hd: bool, panel: (u32, u32)) -> io::Result<()> {
    fs::write(BLANK, "4")?;
    let result: io::Result<()> = (|| {
        switch(hdmi, full_hd)?;
        let buffer = if hdmi { (1280, 720) } else { panel };
        let output = if hdmi && full_hd {
            (1920, 1080)
        } else {
            buffer
        };
        resize(fb, buffer)?;
        layer_geometry(disp, buffer, output)?;
        sleep(Duration::from_millis(250));
        Ok(())
    })();
    let unblank = fs::write(BLANK, "0");
    result?;
    unblank?;
    let buffer = if hdmi { (1280, 720) } else { panel };
    layer_geometry(
        disp,
        buffer,
        if hdmi && full_hd {
            (1920, 1080)
        } else {
            buffer
        },
    )
}

/// Returns HDMI's logical framebuffer size, or None for the built-in panel.
/// On an HDMI setup error, restore the LCD before allowing EGL to start.
pub fn prepare(state: &SlotState) -> io::Result<Option<(u32, u32)>> {
    if !Path::new(DEBUG).join("start").exists() || !Path::new(STATUS).exists() {
        return Ok(None);
    }
    let was_hdmi = active()?;
    let hdmi = state.hdmi_auto && connected();
    if !was_hdmi && !hdmi {
        return Ok(None);
    }
    let open = |name| OpenOptions::new().read(true).write(true).open(name);
    let fb = open("/dev/fb0")?;
    let disp = open("/dev/disp")?;
    let mut info = FramebufferInfo::default();
    control(&fb, 0x4600, &mut info)?;
    let panel = if !was_hdmi {
        let size = (info.0[0], info.0[1]);
        let _ = fs::write("/run/slot-panel-size", format!("{},{}", size.0, size.1));
        size
    } else {
        fs::read_to_string("/run/slot-panel-size")
            .ok()
            .as_deref()
            .and_then(slot_gfx::panel_size)
            .or_else(|| {
                fs::read_to_string("/sys/class/graphics/fb0/modes")
                    .ok()
                    .as_deref()
                    .and_then(slot_gfx::panel_mode)
                    .filter(|&(w, h)| w <= 720 && h <= 720)
            })
            .unwrap_or((720, 480))
    };
    if let Err(error) = apply(&fb, &disp, hdmi, state.hdmi_1080p, panel) {
        eprintln!("slot: HDMI setup failed: {error}; restoring internal screen");
        apply(&fb, &disp, false, false, panel)?;
        return Ok(None);
    }
    eprintln!(
        "slot: video out: {}",
        if hdmi {
            if state.hdmi_1080p {
                "HDMI 1080p60 (720p framebuffer)"
            } else {
                "HDMI 720p60"
            }
        } else {
            "internal screen"
        }
    );
    Ok(hdmi.then_some((1280, 720)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stable_cable_change_stays_pending_but_a_bounce_or_internal_mode_does_not_restart() {
        let now = Instant::now();
        let mut watch = CableWatch {
            current: false,
            seen: false,
            since: now,
            next: now,
            automatic: true,
            failures: 0,
        };
        assert!(!watch.observe(true, now));
        assert!(!watch.present_at(now));
        assert!(!watch.observe(false, now + Duration::from_millis(500)));
        assert!(!watch.observe(true, now + Duration::from_millis(1000)));
        assert!(watch.observe(true, now + Duration::from_millis(1500)));
        assert!(watch.present_at(now + Duration::from_millis(1500)));
        assert!(watch.observe(true, now + Duration::from_millis(2000)));
        watch.automatic = false;
        assert!(!watch.observe(true, now + Duration::from_millis(2500)));
    }

    #[test]
    fn a_failed_save_backs_off_and_a_reverted_cable_starts_over() {
        let now = Instant::now();
        let mut watch = CableWatch {
            current: false,
            seen: true,
            since: now,
            next: now,
            automatic: true,
            failures: 0,
        };
        let mut at = now + Duration::from_millis(500);
        assert!(watch.observe(true, at));
        let mut waits = Vec::new();
        for _ in 0..6 {
            let wait = watch.defer_at(at);
            assert!(!watch.observe(true, at + wait - Duration::from_millis(1)));
            at += wait;
            assert!(watch.observe(true, at));
            waits.push(wait.as_secs());
        }
        assert_eq!(waits, [5, 10, 20, 40, 60, 60]);
        watch.defer_at(at);
        at += RETRY_MAX;
        assert!(!watch.observe(false, at));
        assert_eq!(watch.defer_at(at), RETRY_FIRST);
    }

    #[test]
    fn h700_uapi_layout_matches_the_kernel() {
        assert_eq!(std::mem::size_of::<FramebufferInfo>(), 160);
        assert_eq!(std::mem::size_of::<Buffer>(), 128);
        assert_eq!(std::mem::size_of::<Layer>(), 184);
        assert_eq!(std::mem::offset_of!(LayerInfo, buffer), 32);
        assert_eq!(std::mem::offset_of!(Buffer, crop), 88);
    }

    #[test]
    fn parses_both_extcon_state_formats_without_matching_other_cables() {
        for value in ["1\n", "HDMI=1\n", "USB=0 HDMI=1"] {
            assert!(cable_state(value));
        }
        for value in ["0\n", "HDMI=0\n", "USB=1\n", "", "HDMI=10"] {
            assert!(!cable_state(value));
        }
    }

    #[test]
    fn signal_modes_are_distinct_and_lcd_ignores_hdmi_resolution() {
        assert!(switch_parameter(true, false).starts_with("4 5 "));
        assert!(switch_parameter(true, true).starts_with("4 10 "));
        assert_eq!(switch_parameter(false, true), "1 0");
    }
}
