use std::ffi::{c_char, c_int, c_uint, c_void, CStr, CString};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use libloading::Library;

use super::ring::Ring;
use super::route::{routed_devices, AudioRoute, PlaybackDevice};
use super::silence::Silence;
use super::sink::{AudioError, AudioSink};

const SND_PCM_STREAM_PLAYBACK: c_int = 0;
const SND_PCM_FORMAT_S16_LE: c_int = 2;
const SND_PCM_ACCESS_RW_INTERLEAVED: c_int = 3;
const CHANNELS: c_uint = 2;

const LATENCY_US: c_uint = 40_000;

const PERIOD_FRAMES: usize = 512;

const RELEASE_AFTER: Duration = Duration::from_secs(3);

const RETRY_AFTER: Duration = Duration::from_secs(2);
const SND_PCM_NONBLOCK: c_int = 1;

type PcmOpen = unsafe extern "C" fn(*mut *mut c_void, *const c_char, c_int, c_int) -> c_int;
type PcmSetParams =
    unsafe extern "C" fn(*mut c_void, c_int, c_int, c_uint, c_uint, c_int, c_uint) -> c_int;
type PcmWritei = unsafe extern "C" fn(*mut c_void, *const c_void, u64) -> i64;
type PcmWait = unsafe extern "C" fn(*mut c_void, c_int) -> c_int;
type PcmRecover = unsafe extern "C" fn(*mut c_void, c_int, c_int) -> c_int;
type PcmDrop = unsafe extern "C" fn(*mut c_void) -> c_int;
type PcmClose = unsafe extern "C" fn(*mut c_void) -> c_int;
type PcmInfoMalloc = unsafe extern "C" fn(*mut *mut c_void) -> c_int;
type PcmInfoFree = unsafe extern "C" fn(*mut c_void);
type PcmInfo = unsafe extern "C" fn(*mut c_void, *mut c_void) -> c_int;
type PcmInfoGetCard = unsafe extern "C" fn(*const c_void) -> c_int;
type StrError = unsafe extern "C" fn(c_int) -> *const c_char;

struct Alsa {
    open: PcmOpen,
    set_params: PcmSetParams,
    writei: PcmWritei,
    wait: PcmWait,
    recover: PcmRecover,
    drop: PcmDrop,
    close: PcmClose,
    info_malloc: PcmInfoMalloc,
    info_free: PcmInfoFree,
    info: PcmInfo,
    info_get_card: PcmInfoGetCard,
    strerror: StrError,
    _lib: Library,
}

impl Alsa {
    fn load() -> Result<Self, AudioError> {
        let lib = unsafe { Library::new("libasound.so.2") }
            .or_else(|_| unsafe { Library::new("libasound.so") })
            .map_err(|e| AudioError::Device(format!("libasound: {e}")))?;
        macro_rules! get {
            ($name:literal) => {
                *lib.get(concat!($name, "\0").as_bytes())
                    .map_err(|e| AudioError::Device(format!("{}: {e}", $name)))?
            };
        }
        unsafe {
            Ok(Alsa {
                open: get!("snd_pcm_open"),
                set_params: get!("snd_pcm_set_params"),
                writei: get!("snd_pcm_writei"),
                wait: get!("snd_pcm_wait"),
                recover: get!("snd_pcm_recover"),
                drop: get!("snd_pcm_drop"),
                close: get!("snd_pcm_close"),
                info_malloc: get!("snd_pcm_info_malloc"),
                info_free: get!("snd_pcm_info_free"),
                info: get!("snd_pcm_info"),
                info_get_card: get!("snd_pcm_info_get_card"),
                strerror: get!("snd_strerror"),
                _lib: lib,
            })
        }
    }

    fn message(&self, err: c_int) -> String {
        let text = unsafe { (self.strerror)(err) };
        match text.is_null() {
            true => format!("error {err}"),
            false => unsafe { CStr::from_ptr(text) }
                .to_string_lossy()
                .into_owned(),
        }
    }

    fn open_pcm(&self, rate: u32, route: AudioRoute) -> Result<(*mut c_void, bool), AudioError> {
        let cards = std::fs::read_to_string("/proc/asound/cards").unwrap_or_default();
        let pcms = std::fs::read_to_string("/proc/asound/pcm").unwrap_or_default();
        let devices = routed_devices(&cards, &pcms, route);
        self.open_devices(rate, route, &devices)
    }

    fn open_devices(
        &self,
        rate: u32,
        route: AudioRoute,
        devices: &[PlaybackDevice],
    ) -> Result<(*mut c_void, bool), AudioError> {
        let mut last = AudioError::NoDevice;
        for device in devices {
            let name = &device.name;
            let Ok(cname) = CString::new(name.as_str()) else {
                continue;
            };
            let mut pcm: *mut c_void = std::ptr::null_mut();
            let err = unsafe {
                (self.open)(
                    &mut pcm,
                    cname.as_ptr(),
                    SND_PCM_STREAM_PLAYBACK,
                    SND_PCM_NONBLOCK,
                )
            };
            if err < 0 || pcm.is_null() {
                last = AudioError::Device(format!("{name}: {}", self.message(err)));
                continue;
            }
            if let Some(codec_card) = device.codec_card {
                match self.pcm_card(pcm) {
                    Ok(card) if u32::try_from(card) == Ok(codec_card) => {}
                    card => {
                        unsafe { (self.close)(pcm) };
                        last = AudioError::Device(format!(
                            "{name}: default does not resolve to an onboard codec ({card:?})"
                        ));
                        eprintln!("slot: audio: {last}, trying another output");
                        continue;
                    }
                }
            }
            let err = unsafe {
                (self.set_params)(
                    pcm,
                    SND_PCM_FORMAT_S16_LE,
                    SND_PCM_ACCESS_RW_INTERLEAVED,
                    CHANNELS,
                    rate,
                    1,
                    LATENCY_US,
                )
            };
            if err < 0 {
                unsafe { (self.close)(pcm) };
                last = AudioError::Config(format!("{name}: {}", self.message(err)));
                continue;
            }
            eprintln!(
                "slot: audio {name} at {rate} Hz (requested {}, docked {})",
                route.output.name(),
                route.docked
            );
            return Ok((pcm, device.hdmi));
        }
        Err(last)
    }

    fn pcm_card(&self, pcm: *mut c_void) -> Result<c_int, AudioError> {
        let mut info = std::ptr::null_mut();
        let err = unsafe { (self.info_malloc)(&mut info) };
        if err < 0 || info.is_null() {
            return Err(AudioError::Device(format!(
                "PCM info: {}",
                self.message(err)
            )));
        }
        let err = unsafe { (self.info)(pcm, info) };
        let card = unsafe { (self.info_get_card)(info) };
        unsafe { (self.info_free)(info) };
        if err < 0 {
            Err(AudioError::Device(format!(
                "PCM info: {}",
                self.message(err)
            )))
        } else {
            Ok(card)
        }
    }
}

pub struct AlsaSink {
    ring: Arc<Ring>,
    device: Option<Device>,
    route: Arc<Mutex<AudioRoute>>,
    hdmi_active: Arc<AtomicBool>,
    bluetooth_active: Arc<AtomicBool>,
}

struct Device {
    stop: Arc<AtomicBool>,
    join: JoinHandle<()>,
}

impl AlsaSink {
    pub fn new() -> Self {
        AlsaSink {
            ring: Arc::new(Ring::new(0)),
            device: None,
            route: Arc::new(Mutex::new(AudioRoute::default())),
            hdmi_active: Arc::new(AtomicBool::new(false)),
            bluetooth_active: Arc::new(AtomicBool::new(false)),
        }
    }

    fn close(&mut self) {
        if let Some(d) = self.device.take() {
            d.stop.store(true, Ordering::Relaxed);
            let _ = d.join.join();
        }
        self.hdmi_active.store(false, Ordering::Relaxed);
        self.bluetooth_active.store(false, Ordering::Relaxed);
    }
}

impl Default for AlsaSink {
    fn default() -> Self {
        AlsaSink::new()
    }
}

impl Drop for AlsaSink {
    fn drop(&mut self) {
        self.close();
    }
}

impl AudioSink for AlsaSink {
    fn open(&mut self, sample_rate: u32) -> Result<(), AudioError> {
        self.close();
        let ring = self.ring.clone();
        let route = self.route.clone();
        let hdmi_active = self.hdmi_active.clone();
        let bluetooth_active = self.bluetooth_active.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let (ready_tx, ready_rx) = mpsc::channel();
        let join = std::thread::Builder::new()
            .name("slot-audio".into())
            .spawn(move || {
                let initial = *route.lock().unwrap();
                match play(&ring, sample_rate, initial, hdmi_active, bluetooth_active) {
                    Ok(mut device) => {
                        let _ = ready_tx.send(Ok(()));
                        device.run(&ring, &flag, &route);
                    }
                    Err(e) => {
                        let _ = ready_tx.send(Err(e));
                    }
                }
            })
            .map_err(|e| AudioError::Device(e.to_string()))?;
        match ready_rx.recv() {
            Ok(Ok(())) => {
                self.device = Some(Device { stop, join });
                Ok(())
            }
            Ok(Err(e)) => {
                let _ = join.join();
                Err(e)
            }
            Err(_) => {
                let _ = join.join();
                Err(AudioError::Device("output thread stopped".into()))
            }
        }
    }

    fn ring(&self) -> Arc<Ring> {
        self.ring.clone()
    }

    fn configure_route(&mut self, route: AudioRoute) {
        *self.route.lock().unwrap() = route;
    }

    fn bluetooth_active(&self) -> bool {
        self.bluetooth_active.load(Ordering::Relaxed)
    }

    fn hdmi_active(&self) -> bool {
        self.hdmi_active.load(Ordering::Relaxed)
    }
}

struct Playback {
    alsa: Alsa,
    pcm: Option<*mut c_void>,
    rate: u32,
    failing: bool,
    route: AudioRoute,
    next_retry: Instant,
    hdmi_active: Arc<AtomicBool>,
    bluetooth_active: Arc<AtomicBool>,
}

fn play(
    ring: &Arc<Ring>,
    sample_rate: u32,
    route: AudioRoute,
    hdmi_active: Arc<AtomicBool>,
    bluetooth_active: Arc<AtomicBool>,
) -> Result<Playback, AudioError> {
    let alsa = Alsa::load()?;
    let mut output_error = None;
    let pcm = match alsa.open_pcm(sample_rate, route) {
        Ok((pcm, hdmi)) => {
            hdmi_active.store(hdmi, Ordering::Relaxed);
            bluetooth_active.store(route.wants_bluetooth(), Ordering::Relaxed);
            Some(pcm)
        }
        Err(e) => {
            eprintln!("slot: audio: {e}, retrying");
            output_error = Some(e.to_string());
            None
        }
    };
    ring.reopen(sample_rate);
    if let Some(error) = output_error {
        ring.report_output_error(error);
    }
    Ok(Playback {
        alsa,
        pcm,
        rate: sample_rate,
        failing: pcm.is_none(),
        route,
        next_retry: Instant::now() + RETRY_AFTER,
        hdmi_active,
        bluetooth_active,
    })
}

impl Playback {
    fn run(&mut self, ring: &Ring, stop: &AtomicBool, route: &Mutex<AudioRoute>) {
        let mut buf = vec![0i16; PERIOD_FRAMES * CHANNELS as usize];
        let span = Duration::from_secs_f64(PERIOD_FRAMES as f64 / self.rate as f64);
        let mut silence = Silence::new(RELEASE_AFTER);
        while !stop.load(Ordering::Relaxed) {
            let requested = *route.lock().unwrap();
            if requested != self.route {
                self.release();
                self.hdmi_active.store(false, Ordering::Relaxed);
                self.bluetooth_active.store(false, Ordering::Relaxed);
                self.route = requested;
                self.next_retry = Instant::now();
                self.failing = false;
            }
            ring.fill(&mut buf);
            let hold = silence.hear(&buf, span);
            match (self.pcm, hold) {
                (Some(_), false) => {
                    self.release();
                    eprintln!("slot: audio released after silence");
                }
                (None, true) if Instant::now() >= self.next_retry => {
                    self.next_retry = Instant::now() + RETRY_AFTER;
                    match self.alsa.open_pcm(self.rate, self.route) {
                        Ok((pcm, hdmi)) => {
                            self.hdmi_active.store(hdmi, Ordering::Relaxed);
                            self.bluetooth_active
                                .store(self.route.wants_bluetooth(), Ordering::Relaxed);
                            self.pcm = Some(pcm);
                            self.failing = false;
                        }
                        Err(e) => {
                            self.hdmi_active.store(false, Ordering::Relaxed);
                            self.bluetooth_active.store(false, Ordering::Relaxed);
                            ring.report_output_error(e.to_string());
                            if !self.failing {
                                eprintln!("slot: audio: {e}, retrying");
                                self.failing = true;
                            }
                        }
                    }
                }
                _ => {}
            }
            match self.pcm {
                Some(pcm) => {
                    if !self.write(pcm, &buf, stop) {
                        if !stop.load(Ordering::Relaxed) {
                            ring.report_output_error(
                                "Audio output write failed; see slot.log".into(),
                            );
                        }
                        self.release();
                        self.hdmi_active.store(false, Ordering::Relaxed);
                        self.bluetooth_active.store(false, Ordering::Relaxed);
                        self.next_retry = Instant::now() + RETRY_AFTER;
                        std::thread::sleep(span);
                    }
                }
                None => std::thread::sleep(span),
            }
        }
    }

    fn write(&self, pcm: *mut c_void, buf: &[i16], stop: &AtomicBool) -> bool {
        let mut written = 0;
        let deadline = Instant::now() + Duration::from_millis(250);
        while written < PERIOD_FRAMES {
            if stop.load(Ordering::Relaxed) {
                return false;
            }
            if Instant::now() >= deadline {
                eprintln!("slot: audio: PCM write stalled for 250 ms, reopening output");
                return false;
            }
            let at = written * CHANNELS as usize;
            let frames = unsafe {
                (self.alsa.writei)(
                    pcm,
                    buf[at..].as_ptr() as *const c_void,
                    (PERIOD_FRAMES - written) as u64,
                )
            };
            if frames == -11 || frames == 0 {
                // EAGAIN: nonblocking PCM has no room yet.
                // A bounded ALSA wait avoids waking every millisecond while
                // retaining prompt shutdown on a dock/display change.
                let ready = unsafe { (self.alsa.wait)(pcm, 10) };
                if ready < 0 && ready != -32 {
                    eprintln!(
                        "slot: audio: {}, reopening output",
                        self.alsa.message(ready)
                    );
                    return false;
                }
                continue;
            }
            if frames < 0 {
                // Only recover underruns here. Suspend recovery can wait
                // indefinitely and prevent a route switch or process restart.
                if frames != -32 {
                    // EPIPE
                    eprintln!(
                        "slot: audio: {}, reopening output",
                        self.alsa.message(frames as c_int)
                    );
                    return false;
                }
                let err = unsafe { (self.alsa.recover)(pcm, frames as c_int, 1) };
                if err < 0 {
                    eprintln!("slot: audio: {}", self.alsa.message(err));
                    return false;
                }
                continue;
            }
            written += frames as usize;
        }
        true
    }

    fn release(&mut self) {
        if let Some(pcm) = self.pcm.take() {
            unsafe {
                (self.alsa.drop)(pcm);
                (self.alsa.close)(pcm);
            }
        }
    }
}

impl Drop for Playback {
    fn drop(&mut self) {
        self.release();
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    static CLOSED: AtomicUsize = AtomicUsize::new(0);
    static WRITES: AtomicUsize = AtomicUsize::new(0);
    static RECOVERIES: AtomicUsize = AtomicUsize::new(0);
    static DEFAULT_CARD: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);
    static INFO_FAILS: AtomicBool = AtomicBool::new(false);
    static HDMI_FAILS: AtomicBool = AtomicBool::new(false);
    static LAST_PCM: AtomicUsize = AtomicUsize::new(0);
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    unsafe extern "C" fn open(
        pcm: *mut *mut c_void,
        name: *const c_char,
        _: c_int,
        mode: c_int,
    ) -> c_int {
        assert_eq!(mode, SND_PCM_NONBLOCK);
        let name = unsafe { CStr::from_ptr(name) }.to_str().unwrap();
        if name == "hdmi-unavailable"
            || name.starts_with("bluealsa:")
            || (name == "plughw:2,0" && HDMI_FAILS.load(Ordering::Relaxed))
        {
            return -19;
        }
        unsafe {
            *pcm = match name {
                "hdmi-bad-rate" => 2usize as *mut c_void,
                "plug:default" | "default" => 4usize as *mut c_void,
                "plughw:2,0" => 5usize as *mut c_void,
                _ => std::ptr::dangling_mut::<c_void>(),
            };
        }
        0
    }

    unsafe extern "C" fn params(
        pcm: *mut c_void,
        _: c_int,
        _: c_int,
        channels: c_uint,
        rate: c_uint,
        resample: c_int,
        _: c_uint,
    ) -> c_int {
        assert_eq!(channels, 2);
        assert_eq!(rate, GBA_HZ);
        assert_eq!(resample, 1);
        LAST_PCM.store(pcm as usize, Ordering::Relaxed);
        if pcm as usize == 2 {
            -22
        } else {
            0
        }
    }

    unsafe extern "C" fn close(_: *mut c_void) -> c_int {
        CLOSED.fetch_add(1, Ordering::Relaxed);
        0
    }
    unsafe extern "C" fn drop_pcm(_: *mut c_void) -> c_int {
        0
    }
    unsafe extern "C" fn error(_: c_int) -> *const c_char {
        c"fake PCM error".as_ptr()
    }
    unsafe extern "C" fn unavailable(_: *mut c_void, _: *const c_void, _: u64) -> i64 {
        WRITES.fetch_add(1, Ordering::Relaxed);
        -19
    }
    unsafe extern "C" fn full(_: *mut c_void, _: *const c_void, _: u64) -> i64 {
        -11
    }
    unsafe extern "C" fn wait(_: *mut c_void, timeout: c_int) -> c_int {
        std::thread::sleep(Duration::from_millis(timeout as u64));
        0
    }
    unsafe extern "C" fn no_recover(_: *mut c_void, _: c_int, _: c_int) -> c_int {
        RECOVERIES.fetch_add(1, Ordering::Relaxed);
        -1
    }
    unsafe extern "C" fn info_malloc(info: *mut *mut c_void) -> c_int {
        unsafe { *info = Box::into_raw(Box::new(0 as c_int)).cast() };
        0
    }
    unsafe extern "C" fn info_free(info: *mut c_void) {
        unsafe { std::mem::drop(Box::from_raw(info.cast::<c_int>())) };
    }
    unsafe extern "C" fn info(_: *mut c_void, info: *mut c_void) -> c_int {
        unsafe { *info.cast::<c_int>() = DEFAULT_CARD.load(Ordering::Relaxed) };
        if INFO_FAILS.load(Ordering::Relaxed) {
            -5
        } else {
            0
        }
    }
    unsafe extern "C" fn info_get_card(info: *const c_void) -> c_int {
        unsafe { *info.cast::<c_int>() }
    }
    fn device(name: &str, hdmi: bool) -> PlaybackDevice {
        PlaybackDevice {
            name: name.into(),
            hdmi,
            codec_card: None,
        }
    }
    fn alsa(writei: PcmWritei) -> Alsa {
        Alsa {
            open,
            set_params: params,
            writei,
            wait,
            recover: no_recover,
            drop: drop_pcm,
            close,
            info_malloc,
            info_free,
            info,
            info_get_card,
            strerror: error,
            _lib: libloading::os::unix::Library::this().into(),
        }
    }
    fn playback(writei: PcmWritei) -> Playback {
        Playback {
            alsa: alsa(writei),
            pcm: None,
            rate: GBA_HZ,
            failing: false,
            route: AudioRoute::default(),
            next_retry: Instant::now(),
            hdmi_active: Arc::new(AtomicBool::new(false)),
            bluetooth_active: Arc::new(AtomicBool::new(false)),
        }
    }

    use crate::audio::GBA_HZ;
    #[test]
    fn an_unavailable_or_incompatible_tv_pcm_falls_back_to_the_device_and_closes_failed_handles() {
        let _lock = TEST_LOCK.lock().unwrap();
        CLOSED.store(0, Ordering::Relaxed);
        let alsa = alsa(full);
        let devices = vec![
            device("hdmi-unavailable", true),
            device("hdmi-bad-rate", true),
            device("codec", false),
        ];
        let route = AudioRoute {
            output: slot_store::AudioOutput::Auto,
            docked: true,
            bluetooth: None,
        };
        let (pcm, hdmi) = alsa.open_devices(GBA_HZ, route, &devices).unwrap();
        assert!(!hdmi);
        assert_eq!(CLOSED.load(Ordering::Relaxed), 1);
        unsafe {
            (alsa.close)(pcm);
        }
        assert_eq!(CLOSED.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn unavailable_bluetooth_audio_does_not_open_a_speaker_fallback() {
        let _lock = TEST_LOCK.lock().unwrap();
        let devices = routed_devices(
            "0 [sndcodec]: sunxi-codec",
            "00-00: codec : playback 1",
            AudioRoute {
                output: slot_store::AudioOutput::Auto,
                docked: false,
                bluetooth: Some([1; 6]),
            },
        );
        assert_eq!(devices.len(), 1);
        assert!(alsa(full)
            .open_devices(
                GBA_HZ,
                AudioRoute {
                    bluetooth: Some([1; 6]),
                    ..Default::default()
                },
                &devices
            )
            .is_err());
    }

    #[test]
    fn handheld_uses_the_firmware_default_and_rejects_defaults_on_hdmi_or_usb_cards() {
        let _lock = TEST_LOCK.lock().unwrap();
        let cards = "0 [sndcodec]: sunxi-codec\n2 [sndhdmi]: HDMI\n1 [USB]: USB Audio CODEC\n";
        let pcms =
            "00-00: codec : playback 1\n02-00: HDMI : playback 1\n01-00: USB Audio : playback 1\n";
        let route = AudioRoute {
            output: slot_store::AudioOutput::Device,
            docked: true,
            bluetooth: None,
        };
        let devices = routed_devices(cards, pcms, route);
        let alsa = alsa(full);
        DEFAULT_CARD.store(0, Ordering::Relaxed);
        INFO_FAILS.store(false, Ordering::Relaxed);
        CLOSED.store(0, Ordering::Relaxed);
        let (pcm, hdmi) = alsa.open_devices(GBA_HZ, route, &devices).unwrap();
        assert_eq!(
            LAST_PCM.load(Ordering::Relaxed),
            4,
            "bypassed firmware mixer hooks"
        );
        assert!(!hdmi);
        unsafe { (alsa.close)(pcm) };
        for other_card in [2, 1, -1] {
            DEFAULT_CARD.store(other_card, Ordering::Relaxed);
            CLOSED.store(0, Ordering::Relaxed);
            let (pcm, hdmi) = alsa.open_devices(GBA_HZ, route, &devices).unwrap();
            assert_eq!(LAST_PCM.load(Ordering::Relaxed), 1);
            assert!(!hdmi);
            assert_eq!(
                CLOSED.load(Ordering::Relaxed),
                2,
                "rejected default handles leaked"
            );
            unsafe { (alsa.close)(pcm) };
        }
        DEFAULT_CARD.store(0, Ordering::Relaxed);
        INFO_FAILS.store(true, Ordering::Relaxed);
        let (pcm, _) = alsa.open_devices(GBA_HZ, route, &devices).unwrap();
        assert_eq!(
            LAST_PCM.load(Ordering::Relaxed),
            1,
            "unverified default was accepted"
        );
        unsafe { (alsa.close)(pcm) };
        INFO_FAILS.store(false, Ordering::Relaxed);
    }

    #[test]
    fn docked_auto_prefers_hdmi_and_unknown_inventory_keeps_legacy_defaults() {
        let _lock = TEST_LOCK.lock().unwrap();
        let route = AudioRoute {
            output: slot_store::AudioOutput::Auto,
            docked: true,
            bluetooth: None,
        };
        let devices = routed_devices(
            "",
            "00-00: codec : playback 1\n02-00: HDMI : playback 1",
            route,
        );
        let alsa = alsa(full);
        let (pcm, hdmi) = alsa.open_devices(GBA_HZ, route, &devices).unwrap();
        assert_eq!(LAST_PCM.load(Ordering::Relaxed), 5);
        assert!(hdmi);
        unsafe { (alsa.close)(pcm) };
        DEFAULT_CARD.store(0, Ordering::Relaxed);
        HDMI_FAILS.store(true, Ordering::Relaxed);
        let (pcm, hdmi) = alsa.open_devices(GBA_HZ, route, &devices).unwrap();
        assert_eq!(
            LAST_PCM.load(Ordering::Relaxed),
            4,
            "TV failure bypassed firmware mixer hooks"
        );
        assert!(!hdmi);
        unsafe { (alsa.close)(pcm) };
        HDMI_FAILS.store(false, Ordering::Relaxed);
        INFO_FAILS.store(true, Ordering::Relaxed);
        let devices = routed_devices("", "", AudioRoute::default());
        let (pcm, hdmi) = alsa
            .open_devices(GBA_HZ, AudioRoute::default(), &devices)
            .unwrap();
        assert_eq!(LAST_PCM.load(Ordering::Relaxed), 4);
        assert!(!hdmi);
        unsafe { (alsa.close)(pcm) };
        INFO_FAILS.store(false, Ordering::Relaxed);
    }

    #[test]
    fn a_full_pcm_is_interruptible_and_a_disconnected_pcm_does_not_try_suspend_recovery() {
        let _lock = TEST_LOCK.lock().unwrap();
        RECOVERIES.store(0, Ordering::Relaxed);
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let stopper = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(10));
            flag.store(true, Ordering::Relaxed);
        });
        let pcm = std::ptr::dangling_mut::<c_void>();
        let buf = [0i16; PERIOD_FRAMES * CHANNELS as usize];
        let start = Instant::now();
        assert!(!playback(full).write(pcm, &buf, &stop));
        assert!(start.elapsed() < Duration::from_secs(1));
        stopper.join().unwrap();
        assert!(!playback(unavailable).write(pcm, &buf, &AtomicBool::new(false)));
        assert_eq!(RECOVERIES.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn output_failure_keeps_the_worker_alive_and_a_route_change_retries_playback() {
        let _lock = TEST_LOCK.lock().unwrap();
        WRITES.store(0, Ordering::Relaxed);
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let route = Arc::new(Mutex::new(AudioRoute::default()));
        let requested = route.clone();
        let ring = Arc::new(Ring::new(1024));
        let join = std::thread::spawn(move || {
            let mut player = playback(unavailable);
            player.pcm = Some(std::ptr::dangling_mut::<c_void>());
            player.run(&ring, &flag, &requested);
        });
        let deadline = Instant::now() + Duration::from_secs(1);
        while WRITES.load(Ordering::Relaxed) == 0 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(1));
        }
        route.lock().unwrap().docked = true;
        while WRITES.load(Ordering::Relaxed) < 2 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(1));
        }
        let alive = !join.is_finished();
        stop.store(true, Ordering::Relaxed);
        join.join().unwrap();
        assert!(alive, "a PCM failure killed the output worker");
        assert!(
            WRITES.load(Ordering::Relaxed) >= 2,
            "changing the route did not reopen output"
        );
    }
}
