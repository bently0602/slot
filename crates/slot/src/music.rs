//! Music decoding/transport never runs in an output callback or the UI thread.
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{mpsc, Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use rubato::{
    Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};
use slot_store::{Album, MusicResume, MusicSettings, Repeat};
use symphonia::core::{
    audio::SampleBuffer,
    codecs::{Decoder, DecoderOptions},
    errors::Error,
    formats::{FormatOptions, FormatReader, SeekMode, SeekTo},
    io::MediaSourceStream,
    meta::{Limit, MetadataOptions},
    probe::Hint,
    units::Time,
};

use crate::audio::Ring;

pub const MUSIC_HZ: u32 = 48_000;
pub const FFT_SIZE: usize = 2048;
pub const BANDS: usize = 32;
pub const WAVE_POINTS: usize = 192;

#[derive(Clone, Debug)]
pub struct Analysis {
    pub bars: [f32; BANDS],
    pub peaks: [f32; BANDS],
    pub waveform: [f32; WAVE_POINTS],
}

impl Default for Analysis {
    fn default() -> Self {
        Self {
            bars: [0.0; BANDS],
            peaks: [0.0; BANDS],
            waveform: [0.0; WAVE_POINTS],
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub album: Option<Arc<Album>>,
    pub index: usize,
    pub playing: bool,
    pub position: f64,
    pub duration: Option<f64>,
    pub settings: MusicSettings,
    pub analysis: Analysis,
    pub error: Option<String>,
    pub error_id: u64,
}
impl Snapshot {
    pub fn resume(&self) -> Option<MusicResume> {
        let album = self.album.as_ref()?;
        Some(MusicResume {
            album: album.path.clone(),
            track: album.tracks.get(self.index)?.path.clone(),
            position: self.position,
        })
    }
    pub fn title(&self) -> Option<&str> {
        Some(&self.album.as_ref()?.tracks.get(self.index)?.title)
    }
}

pub enum Command {
    Play {
        album: Album,
        index: usize,
        position: f64,
        playing: bool,
    },
    Toggle,
    Pause,
    Stop,
    Seek(f64),
    Previous,
    Next,
    Settings(MusicSettings),
    Visible(bool),
    Volume(u8),
    Error(String),
    Checkpoint,
    Shutdown,
    Forget,
}
struct Request {
    command: Command,
    done: Option<mpsc::Sender<()>>,
}
pub struct Player {
    tx: mpsc::Sender<Request>,
    state: Arc<Mutex<Snapshot>>,
    join: Option<JoinHandle<()>>,
}
impl Player {
    pub fn spawn(root: PathBuf, ring: Arc<Ring>) -> Self {
        let settings = MusicSettings::read(&root);
        let state = Arc::new(Mutex::new(Snapshot {
            settings,
            ..Snapshot::default()
        }));
        let (tx, rx) = mpsc::channel();
        let shared = state.clone();
        let join = std::thread::Builder::new()
            .name("slot-music".into())
            .spawn(move || Worker::new(root, ring, shared).run(rx))
            .expect("music worker");
        Self {
            tx,
            state,
            join: Some(join),
        }
    }
    pub fn send(&self, command: Command) {
        let _ = self.tx.send(Request {
            command,
            done: None,
        });
    }
    /// Acknowledgment is required before another producer acquires the audio ring.
    pub fn sync(&self, command: Command) {
        let (tx, rx) = mpsc::channel();
        if self
            .tx
            .send(Request {
                command,
                done: Some(tx),
            })
            .is_ok()
        {
            let _ = rx.recv();
        }
    }
    pub fn snapshot(&self) -> Snapshot {
        self.state.lock().unwrap().clone()
    }
}
impl Drop for Player {
    fn drop(&mut self) {
        self.sync(Command::Shutdown);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

/// Album order and history are independent of the currently browsed album.
pub struct Queue {
    order: Vec<usize>,
    at: usize,
    history: Vec<usize>,
    seed: u64,
}
impl Queue {
    pub fn new(n: usize, selected: usize, shuffle: bool) -> Self {
        let mut q = Self {
            order: (0..n).collect(),
            at: selected.min(n.saturating_sub(1)),
            history: Vec::new(),
            seed: 0x9e3779b97f4a7c15
                ^ std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos() as u64,
        };
        if shuffle {
            q.shuffle(selected);
        }
        q
    }
    fn shuffle(&mut self, selected: usize) {
        self.order.retain(|i| *i != selected);
        for i in (1..self.order.len()).rev() {
            self.seed ^= self.seed << 13;
            self.seed ^= self.seed >> 7;
            self.seed ^= self.seed << 17;
            let j = self.seed as usize % (i + 1);
            self.order.swap(i, j);
        }
        self.order.insert(0, selected);
        self.at = 0;
    }
    pub fn next(
        &mut self,
        n: usize,
        current: usize,
        repeat: Repeat,
        shuffle: bool,
        manual: bool,
    ) -> Option<usize> {
        if !manual && repeat == Repeat::Track {
            return Some(current);
        }
        if self.at + 1 < self.order.len() {
            self.history.push(current);
            self.at += 1;
            return Some(self.order[self.at]);
        }
        if repeat != Repeat::Album {
            return None;
        }
        self.history.push(current);
        self.order = (0..n).collect();
        self.at = 0;
        if shuffle {
            self.shuffle(current);
            if n > 1 {
                self.order.swap(0, 1);
            }
        }
        self.order.first().copied()
    }
    pub fn previous(&mut self, current: usize) -> usize {
        let target = self
            .history
            .pop()
            .unwrap_or_else(|| current.saturating_sub(1));
        self.at = self.order.iter().position(|i| *i == target).unwrap_or(0);
        target
    }
}

struct MusicDecoder {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn Decoder>,
    id: u32,
    rate: u32,
    output_rate: u32,
    duration: Option<f64>,
    discard: usize,
    resampler: Option<SincFixedIn<f32>>,
    channels: [VecDeque<f32>; 2],
    trim: usize,
    input_frames: usize,
    output_frames: usize,
    eof: bool,
}
impl MusicDecoder {
    fn open(path: &std::path::Path, output_rate: u32, position: f64) -> Result<Self, String> {
        let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
        let mut hint = Hint::new();
        if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
            hint.with_extension(ext);
        }
        let mut format = symphonia::default::get_probe()
            .format(
                &hint,
                MediaSourceStream::new(Box::new(file), Default::default()),
                &FormatOptions::default(),
                &MetadataOptions {
                    limit_metadata_bytes: Limit::Maximum(128 * 1024),
                    limit_visual_bytes: Limit::Maximum(0),
                },
            )
            .map_err(|e| e.to_string())?
            .format;
        let track = format.default_track().ok_or("No audio track")?;
        let id = track.id;
        let params = track.codec_params.clone();
        let rate = params.sample_rate.ok_or("Missing sample rate")?;
        let count = params.channels.ok_or("Missing channels")?.count();
        if count == 0 || count > 2 {
            return Err("Only mono/stereo music is supported".into());
        }
        let duration = params.n_frames.map(|n| n as f64 / rate as f64);
        let decoder = symphonia::default::get_codecs()
            .make(&params, &DecoderOptions::default())
            .map_err(|e| e.to_string())?;
        let mut discard = 0;
        if position > 0.0 {
            let seek = format
                .seek(
                    SeekMode::Accurate,
                    SeekTo::Time {
                        time: Time::from(position),
                        track_id: Some(id),
                    },
                )
                .map_err(|e| e.to_string())?;
            let actual = params
                .time_base
                .map(|tb| {
                    let t = tb.calc_time(seek.actual_ts);
                    t.seconds as f64 + t.frac
                })
                .unwrap_or(seek.actual_ts as f64 / rate as f64);
            discard = ((position - actual).max(0.0) * rate as f64).round() as usize;
        }
        let resampler = if rate != output_rate {
            Some(
                SincFixedIn::<f32>::new(
                    output_rate as f64 / rate as f64,
                    1.0,
                    SincInterpolationParameters {
                        sinc_len: 128,
                        f_cutoff: 0.95,
                        interpolation: SincInterpolationType::Linear,
                        oversampling_factor: 128,
                        window: WindowFunction::BlackmanHarris2,
                    },
                    1024,
                    2,
                )
                .map_err(|e| e.to_string())?,
            )
        } else {
            None
        };
        let trim = resampler.as_ref().map_or(0, Resampler::output_delay);
        Ok(Self {
            format,
            decoder,
            id,
            rate,
            output_rate,
            duration,
            discard,
            resampler,
            channels: Default::default(),
            trim,
            input_frames: 0,
            output_frames: 0,
            eof: false,
        })
    }
    fn chunk(&mut self) -> Result<Option<Vec<f32>>, String> {
        while self.channels[0].len() < 1024 && !self.eof {
            let packet = match self.format.next_packet() {
                Ok(p) => p,
                Err(Error::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                    self.eof = true;
                    break;
                }
                Err(e) => return Err(e.to_string()),
            };
            if packet.track_id() != self.id {
                continue;
            }
            let decoded = self.decoder.decode(&packet).map_err(|e| e.to_string())?;
            if decoded.spec().rate != self.rate {
                return Err("Sample rate changed within track".into());
            }
            let count = decoded.spec().channels.count();
            if count == 0 || count > 2 {
                return Err("Only mono/stereo music is supported".into());
            }
            let mut buffer = SampleBuffer::<f32>::new(decoded.capacity() as u64, *decoded.spec());
            buffer.copy_interleaved_ref(decoded);
            for frame in buffer.samples().chunks_exact(count) {
                if self.discard > 0 {
                    self.discard -= 1;
                    continue;
                }
                self.channels[0].push_back(frame[0]);
                self.channels[1].push_back(frame[count - 1]);
            }
        }
        let expected = ((self.input_frames + self.channels[0].len()) as f64
            * self.output_rate as f64
            / self.rate as f64)
            .ceil() as usize;
        if self.channels[0].is_empty()
            && (self.resampler.is_none() || self.output_frames >= expected)
        {
            return Ok(None);
        }
        let n = self.channels[0].len().min(1024);
        self.input_frames += n;
        let mut input = vec![Vec::with_capacity(1024), Vec::with_capacity(1024)];
        for (channel, out) in self.channels.iter_mut().zip(&mut input) {
            out.extend(channel.drain(..n));
            if self.resampler.is_some() {
                out.resize(1024, 0.0);
            }
        }
        let output = if let Some(r) = &mut self.resampler {
            r.process(&input, None).map_err(|e| e.to_string())?
        } else {
            input
        };
        // Flush the sinc filter's delay at EOF, and cut only the padded samples.
        let frames = output[0].len();
        let skip = self.trim.min(frames);
        self.trim -= skip;
        let emitted = if self.eof {
            (frames - skip).min(expected.saturating_sub(self.output_frames))
        } else {
            frames - skip
        };
        self.output_frames += emitted;
        let mut samples = Vec::with_capacity(emitted * 2);
        for (&left, &right) in output[0].iter().zip(&output[1]).skip(skip).take(emitted) {
            samples.push(left);
            samples.push(right);
        }
        Ok(Some(samples))
    }
}

pub struct Analyzer {
    fft: Arc<dyn realfft::RealToComplex<f32>>,
    input: Vec<f32>,
    output: Vec<realfft::num_complex::Complex32>,
    scratch: Vec<realfft::num_complex::Complex32>,
    window: VecDeque<f32>,
    since: usize,
    state: Analysis,
}
impl Default for Analyzer {
    fn default() -> Self {
        Self::new()
    }
}
impl Analyzer {
    pub fn new() -> Self {
        let fft = realfft::RealFftPlanner::<f32>::new().plan_fft_forward(FFT_SIZE);
        Self {
            input: fft.make_input_vec(),
            output: fft.make_output_vec(),
            scratch: fft.make_scratch_vec(),
            fft,
            window: VecDeque::with_capacity(FFT_SIZE),
            since: 0,
            state: Analysis::default(),
        }
    }
    pub fn feed(&mut self, stereo: &[f32], rate: u32) -> Option<Analysis> {
        for f in stereo.chunks_exact(2) {
            if self.window.len() == FFT_SIZE {
                self.window.pop_front();
            }
            self.window.push_back((f[0] + f[1]) * 0.5);
            self.since += 1;
        }
        if self.window.len() < FFT_SIZE || self.since < rate as usize / 30 {
            return None;
        }
        self.since %= (rate as usize / 30).max(1);
        for (i, v) in self.window.iter().enumerate() {
            self.input[i] =
                v * (0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / (FFT_SIZE - 1) as f32).cos());
        }
        self.fft
            .process_with_scratch(&mut self.input, &mut self.output, &mut self.scratch)
            .ok()?;
        let high = (rate as f32 / 2.0).min(20_000.0);
        for band in 0..BANDS {
            let low = 30.0 * (high / 30.0).powf(band as f32 / BANDS as f32);
            let upper = 30.0 * (high / 30.0).powf((band + 1) as f32 / BANDS as f32);
            let from = ((low * FFT_SIZE as f32 / rate as f32) as usize)
                .max(1)
                .min(self.output.len() - 1);
            let to = ((upper * FFT_SIZE as f32 / rate as f32).ceil() as usize)
                .max(from + 1)
                .min(self.output.len());
            let magnitude = self.output[from..to]
                .iter()
                .map(|v| v.norm() * 4.0 / FFT_SIZE as f32)
                .fold(0.0, f32::max);
            let level = ((20.0 * magnitude.max(1e-6).log10() + 60.0) / 60.0).clamp(0.0, 1.0);
            let previous = self.state.bars[band];
            self.state.bars[band] =
                previous + (level - previous) * if level > previous { 0.8 } else { 0.18 };
            self.state.peaks[band] = (self.state.peaks[band] - 0.015).max(self.state.bars[band]);
        }
        for i in 0..WAVE_POINTS {
            self.state.waveform[i] =
                self.window[i * (FFT_SIZE - 1) / (WAVE_POINTS - 1)].clamp(-1.0, 1.0);
        }
        Some(self.state.clone())
    }
}

struct Worker {
    root: PathBuf,
    ring: Arc<Ring>,
    shared: Arc<Mutex<Snapshot>>,
    state: Snapshot,
    decoder: Option<MusicDecoder>,
    queue: Option<Queue>,
    pending: Vec<f32>,
    offset: usize,
    base: f64,
    submitted: usize,
    eof: bool,
    visible: bool,
    volume: u8,
    failures: usize,
    analyzer: Analyzer,
    analyses: VecDeque<(f64, Analysis)>,
    saved: Instant,
    output_progress: Instant,
    heard_frames: usize,
    owns_audio: bool,
}
impl Worker {
    fn new(root: PathBuf, ring: Arc<Ring>, shared: Arc<Mutex<Snapshot>>) -> Self {
        let state = shared.lock().unwrap().clone();
        Self {
            root,
            ring,
            shared,
            state,
            decoder: None,
            queue: None,
            pending: Vec::new(),
            offset: 0,
            base: 0.0,
            submitted: 0,
            eof: false,
            visible: false,
            volume: 70,
            failures: 0,
            analyzer: Analyzer::new(),
            analyses: VecDeque::new(),
            saved: Instant::now(),
            output_progress: Instant::now(),
            heard_frames: 0,
            owns_audio: false,
        }
    }
    fn run(mut self, rx: mpsc::Receiver<Request>) {
        loop {
            self.position();
            match rx.recv_timeout(Duration::from_millis(if self.state.playing {
                2
            } else {
                30
            })) {
                Ok(req) => {
                    let shutdown = matches!(req.command, Command::Shutdown);
                    self.command(req.command);
                    self.publish();
                    if let Some(done) = req.done {
                        let _ = done.send(());
                    }
                    if shutdown {
                        break;
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
            if self.state.playing {
                self.pump();
            } else {
                for b in &mut self.state.analysis.bars {
                    *b *= 0.8;
                }
                for p in &mut self.state.analysis.peaks {
                    *p *= 0.9;
                }
                for v in &mut self.state.analysis.waveform {
                    *v *= 0.8;
                }
            }
            if self.state.playing && self.saved.elapsed() >= Duration::from_secs(15) {
                self.save();
            }
            self.publish();
        }
        self.save();
        if self.owns_audio {
            self.ring.clear();
            self.ring.set_idle(true);
        }
    }
    fn publish(&self) {
        *self.shared.lock().unwrap() = self.state.clone();
    }
    fn position(&mut self) {
        if self.decoder.is_some() {
            let heard = self.submitted.saturating_sub(self.ring.queued_frames());
            if heard > self.heard_frames {
                self.heard_frames = heard;
                self.output_progress = Instant::now();
            }
            self.state.position = self.base + heard as f64 / self.ring.sample_rate().max(1) as f64;
            if let Some(duration) = self.state.duration {
                self.state.position = self.state.position.min(duration);
            }
        }
        while self
            .analyses
            .front()
            .is_some_and(|(at, _)| *at <= self.state.position)
        {
            self.state.analysis = self.analyses.pop_front().unwrap().1;
        }
    }
    fn save(&mut self) {
        self.position();
        self.state.settings.resume = self
            .state
            .resume()
            .or_else(|| self.state.settings.resume.clone());
        if let Err(e) = self.state.settings.write(&self.root) {
            eprintln!("slot: music checkpoint: {e}");
        }
        self.saved = Instant::now();
    }
    fn clear(&mut self) {
        self.position();
        if self.owns_audio {
            self.ring.clear();
            self.ring.set_idle(true);
        }
        self.decoder = None;
        self.pending.clear();
        self.offset = 0;
        self.base = self.state.position;
        self.submitted = 0;
        self.heard_frames = 0;
        self.output_progress = Instant::now();
        self.eof = false;
        self.analyzer = Analyzer::new();
        self.analyses.clear();
        self.state.analysis = Analysis::default();
    }
    fn command(&mut self, command: Command) {
        match command {
            Command::Play {
                album,
                index,
                position,
                playing,
            } => {
                if index >= album.tracks.len() || !position.is_finite() {
                    self.error("Invalid music selection".into());
                    return;
                }
                self.owns_audio |= playing;
                self.clear();
                self.state.album = Some(Arc::new(album));
                self.state.index = index;
                self.state.position = position.max(0.0);
                self.state.playing = playing;
                self.failures = 0;
                let a = self.state.album.as_ref().unwrap();
                self.queue = Some(Queue::new(
                    a.tracks.len(),
                    index,
                    self.state.settings.shuffle,
                ));
                self.state.duration = a.tracks.get(index).and_then(|t| t.duration);
                if let Some(duration) = self.state.duration {
                    self.state.position = self.state.position.min(duration);
                }
                self.base = self.state.position;
                self.save();
            }
            Command::Toggle => {
                self.position();
                self.state.playing = !self.state.playing && self.state.album.is_some();
                let faded = self.state.analysis.clone();
                self.owns_audio |= self.state.playing;
                self.clear();
                if !self.state.playing {
                    self.state.analysis = faded;
                } else if self
                    .state
                    .duration
                    .is_some_and(|d| self.state.position >= d - 0.01)
                {
                    self.state.position = 0.0;
                    self.base = 0.0;
                }
                self.save();
            }
            Command::Pause | Command::Stop | Command::Checkpoint | Command::Shutdown => {
                let checkpoint = matches!(command, Command::Checkpoint);
                self.position();
                if !checkpoint {
                    self.state.playing = false;
                    let faded = self.state.analysis.clone();
                    self.clear();
                    self.state.analysis = faded;
                }
                self.save();
                if matches!(command, Command::Stop | Command::Shutdown) {
                    self.owns_audio = false;
                }
            }
            Command::Seek(delta) => {
                if self.state.album.is_some() {
                    self.position();
                    let pos = (self.state.position + delta)
                        .max(0.0)
                        .min(self.state.duration.unwrap_or(f64::MAX));
                    self.clear();
                    self.state.position = pos;
                    self.base = pos;
                    if self.state.duration.is_some_and(|duration| pos >= duration) {
                        self.state.playing = false;
                    }
                    self.save();
                }
            }
            Command::Previous => {
                self.position();
                let index = if self.state.position > 3.0 {
                    self.state.index
                } else {
                    self.queue
                        .as_mut()
                        .map_or(self.state.index, |q| q.previous(self.state.index))
                };
                self.change_track(index);
                self.failures = 0;
            }
            Command::Next => {
                self.failures = 0;
                self.advance(true);
            }
            Command::Settings(settings) => {
                if settings.shuffle != self.state.settings.shuffle {
                    if let Some(a) = &self.state.album {
                        self.queue = Some(Queue::new(
                            a.tracks.len(),
                            self.state.index,
                            settings.shuffle,
                        ));
                    }
                }
                let resume = self.state.settings.resume.take();
                self.state.settings = settings;
                self.state.settings.resume = resume;
                self.save();
            }
            Command::Visible(on) => {
                if self.visible != on {
                    self.analyzer = Analyzer::new();
                    self.analyses.clear();
                }
                self.visible = on;
            }
            Command::Volume(v) => self.volume = v,
            Command::Forget => {
                self.state.playing = false;
                self.clear();
                self.owns_audio = false;
                self.state.album = None;
                self.state.settings.resume = None;
                self.save();
            }
            Command::Error(e) => {
                self.state.playing = false;
                self.clear();
                self.error(e);
                self.save();
            }
        }
    }
    fn error(&mut self, error: String) {
        eprintln!("slot: music: {error}");
        self.state.error = Some(error);
        self.state.error_id += 1;
    }
    fn change_track(&mut self, index: usize) {
        self.owns_audio = true;
        self.clear();
        self.state.index = index;
        self.state.position = 0.0;
        self.base = 0.0;
        self.state.playing = true;
        self.state.duration = self
            .state
            .album
            .as_ref()
            .and_then(|a| a.tracks.get(index))
            .and_then(|t| t.duration);
        self.save();
    }
    fn advance(&mut self, manual: bool) {
        let next = self
            .state
            .album
            .as_ref()
            .zip(self.queue.as_mut())
            .and_then(|(a, q)| {
                q.next(
                    a.tracks.len(),
                    self.state.index,
                    self.state.settings.repeat,
                    self.state.settings.shuffle,
                    manual,
                )
            });
        if let Some(next) = next {
            self.change_track(next);
        } else {
            self.position();
            self.state.playing = false;
            self.clear();
            self.save();
        }
    }
    fn failed(&mut self, error: String) {
        self.error(error);
        self.failures += 1;
        let n = self.state.album.as_ref().map_or(0, |a| a.tracks.len());
        if self.failures >= n {
            self.state.playing = false;
            self.clear();
            self.save();
        } else {
            // Traverse the remaining queue once, regardless of repeat mode.
            let next = self.queue.as_mut().and_then(|q| {
                q.next(
                    n,
                    self.state.index,
                    Repeat::Off,
                    self.state.settings.shuffle,
                    true,
                )
            });
            if let Some(next) = next {
                self.change_track(next);
            } else {
                self.state.playing = false;
                self.clear();
                self.save();
            }
        }
    }
    fn pump(&mut self) {
        if let Some(error) = self.ring.take_output_error() {
            self.command(Command::Error(error));
            return;
        }
        if self.ring.queued_frames() > 0 && self.output_progress.elapsed() > Duration::from_secs(3)
        {
            self.command(Command::Error("Audio output stopped responding".into()));
            return;
        }
        if self.decoder.is_none() {
            let path = self
                .state
                .album
                .as_ref()
                .and_then(|a| a.tracks.get(self.state.index))
                .map(|t| self.root.join(&t.path));
            let Some(path) = path else {
                self.state.playing = false;
                return;
            };
            match MusicDecoder::open(&path, self.ring.sample_rate().max(1), self.state.position) {
                Ok(d) => {
                    self.state.duration = d.duration.or(self.state.duration);
                    self.decoder = Some(d);
                    self.base = self.state.position;
                    self.submitted = 0;
                    self.ring.set_idle(false);
                }
                Err(e) => {
                    self.failed(format!("{}: {e}", path.display()));
                    return;
                }
            }
        }
        if self.eof {
            if self.ring.queued_frames() == 0 {
                self.position();
                self.advance(false);
            }
            return;
        }
        if self.offset >= self.pending.len() {
            match self.decoder.as_mut().unwrap().chunk() {
                Ok(Some(samples)) => {
                    self.pending = samples;
                    self.offset = 0;
                }
                Ok(None) => {
                    self.eof = true;
                    self.ring.set_idle(true);
                    return;
                }
                Err(e) => {
                    self.failed(e);
                    return;
                }
            }
        }
        let n = (self.pending.len() - self.offset)
            .min(
                self.ring
                    .capacity_frames()
                    .saturating_sub(self.ring.queued_frames())
                    * 2,
            )
            .min(if self.visible { 2048 } else { usize::MAX });
        if n == 0 {
            return;
        }
        let samples = &self.pending[self.offset..self.offset + n];
        let gain = crate::audio::volume::gain(self.volume);
        let output: Vec<i16> = samples
            .iter()
            .map(|s| (s.clamp(-1.0, 1.0) * gain * 32767.0).round() as i16)
            .collect();
        let written = self.ring.write_available(&output);
        if written > 0 {
            self.failures = 0;
            if self.visible {
                if let Some(analysis) = self
                    .analyzer
                    .feed(&samples[..written], self.ring.sample_rate())
                {
                    self.analyses.push_back((
                        self.base
                            + (self.submitted + written / 2) as f64
                                / self.ring.sample_rate() as f64,
                        analysis,
                    ));
                    while self.analyses.len() > 8 {
                        self.analyses.pop_front();
                    }
                }
            }
            self.offset += written;
            self.submitted += written / 2;
        }
        self.position();
    }
}

#[cfg(test)]
mod decoder_tests {
    use super::MusicDecoder;

    #[test]
    fn sinc_resampler_flushes_tail_and_preserves_duration_after_seek() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/music/stereo960.flac");
        for (position, rate, expected) in [
            (0.0, 48_000, 48_000),
            (0.5, 48_000, 24_000),
            (0.0, 44_100, 44_100),
        ] {
            let mut decoder = MusicDecoder::open(&path, rate, position).unwrap();
            let mut frames = 0;
            while let Some(samples) = decoder.chunk().unwrap() {
                frames += samples.len() / 2;
            }
            assert_eq!(frames, expected, "at {position} seconds");
        }
    }
}
