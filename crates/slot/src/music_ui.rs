//! Album navigation, text faces, and bounded background artwork loading.
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::mpsc;

use slot_gfx::{Compositor, Draw, TexId};
use slot_input::{Action, Btn};
use slot_store::{Album, Artwork, MusicSettings, VisualizerStyle};

use crate::music::{Analysis, Command, Snapshot};

pub const PANEL_W: u32 = 560;
pub const PANEL_H: u32 = 356;
const COVER: f32 = 280.0;

#[derive(Default)]
pub struct Browser {
    pub albums: Vec<Album>,
    pub revision: u64,
    pub index: usize,
    pub track: usize,
    pub flipped: bool,
    pub fullscreen: bool,
    pub scanning: bool,
    pub rescan: bool,
    pub settings: MusicSettings,
    pub snapshot: Snapshot,
    pub commands: Vec<Command>,
    pub flip: f32,
    pub scroll: f32,
    ride: f32,
    velocity: f32,
    held: Option<(i32, u64)>,
    pub covers: HashMap<usize, TexId>,
    pub panel: Option<TexId>,
    pub heading: Option<TexId>,
    pub footer: Option<TexId>,
    pub notice: Option<TexId>,
    pub notice_until: u64,
}
impl Browser {
    pub fn selected(&self) -> Option<&Album> {
        self.albums.get(self.index)
    }
    pub fn replace(&mut self, albums: Vec<Album>) {
        let path = self.selected().map(|a| a.path.clone());
        let track = self
            .selected()
            .and_then(|a| a.tracks.get(self.track))
            .map(|t| t.path.clone());
        self.albums = albums;
        self.revision = self.revision.wrapping_add(1);
        self.index = path
            .and_then(|p| self.albums.iter().position(|a| a.path == p))
            .unwrap_or(0);
        self.track = track
            .and_then(|p| self.selected()?.tracks.iter().position(|t| t.path == p))
            .unwrap_or(0);
        self.scroll = self.index as f32;
        self.ride = self.scroll;
        self.velocity = 0.0;
        self.scanning = false;
        self.covers.clear();
    }
    pub fn show_playing(&mut self) -> bool {
        let Some(album) = &self.snapshot.album else {
            return false;
        };
        let Some(index) = self.albums.iter().position(|a| a.path == album.path) else {
            return false;
        };
        self.index = index;
        self.track = album
            .tracks
            .get(self.snapshot.index)
            .and_then(|t| {
                self.albums[index]
                    .tracks
                    .iter()
                    .position(|found| found.path == t.path)
            })
            .unwrap_or(0);
        self.scroll = index as f32;
        self.ride = self.scroll;
        self.flipped = true;
        self.fullscreen = false;
        self.held = None;
        true
    }
    fn select_track(&mut self) {
        if let Some(album) = self.selected().cloned() {
            self.commands.push(Command::Play {
                album,
                index: self.track,
                position: 0.0,
                playing: true,
            });
        }
    }
    pub fn toggle(&mut self) {
        if self.snapshot.album.is_some() {
            self.commands.push(Command::Toggle);
        } else {
            self.track = 0;
            self.select_track();
        }
    }
    fn step(&mut self, by: i32) {
        if self.albums.is_empty() {
            return;
        }
        self.index = (self.index as i32 + by).rem_euclid(self.albums.len() as i32) as usize;
        self.ride += by as f32;
        self.track = 0;
    }
    fn jump(&mut self, by: i32) {
        let Some(album) = self.selected() else { return };
        let letter = slot_store::initial(&album.artist);
        let n = self.albums.len();
        for step in 1..n {
            let i = (self.index as i32 + by * step as i32).rem_euclid(n as i32) as usize;
            if slot_store::initial(&self.albums[i].artist) != letter {
                self.index = i;
                self.ride = i as f32;
                self.track = 0;
                break;
            }
        }
    }
    /// Shelf changes and Menu are handled by App; all emulator actions are swallowed.
    pub fn input(&mut self, action: Action, now: u64) {
        match action {
            Action::GbaDown(Btn::Start) => self.toggle(),
            Action::GbaDown(Btn::X) => {
                if self.snapshot.album.is_some() {
                    self.fullscreen = !self.fullscreen;
                    self.held = None;
                }
            }
            Action::GbaDown(Btn::L2) => {
                if self.snapshot.album.is_some() {
                    self.commands.push(Command::Seek(-10.0));
                }
            }
            Action::GbaDown(Btn::R2) => {
                if self.snapshot.album.is_some() {
                    self.commands.push(Command::Seek(10.0));
                }
            }
            Action::GbaDown(Btn::B) => {
                if self.fullscreen {
                    self.fullscreen = false;
                } else {
                    self.flipped = false;
                }
            }
            Action::GbaDown(Btn::L1) if self.flipped || self.fullscreen => {
                self.commands.push(Command::Previous)
            }
            Action::GbaDown(Btn::R1) if self.flipped || self.fullscreen => {
                self.commands.push(Command::Next)
            }
            Action::GbaDown(Btn::Left | Btn::Right) if self.fullscreen => {
                self.settings.style = match self.settings.style {
                    VisualizerStyle::Bars => VisualizerStyle::Waveform,
                    VisualizerStyle::Waveform => VisualizerStyle::Bars,
                };
                self.commands.push(Command::Settings(self.settings.clone()));
            }
            Action::GbaDown(Btn::Up) if self.flipped && !self.fullscreen => {
                if let Some(a) = self.selected() {
                    self.track = (self.track + a.tracks.len() - 1) % a.tracks.len();
                }
            }
            Action::GbaDown(Btn::Down) if self.flipped && !self.fullscreen => {
                if let Some(a) = self.selected() {
                    self.track = (self.track + 1) % a.tracks.len();
                }
            }
            Action::GbaDown(Btn::A) | Action::Insert if self.flipped && !self.fullscreen => {
                self.select_track()
            }
            Action::GbaDown(Btn::A) | Action::Insert if !self.fullscreen => {
                if self.selected().is_some() {
                    self.flipped = true;
                    self.held = None;
                }
            }
            Action::ShelfLeft | Action::GbaDown(Btn::Left) if !self.flipped && !self.fullscreen => {
                self.step(-1);
                self.held = Some((-1, now + 400));
            }
            Action::ShelfRight | Action::GbaDown(Btn::Right)
                if !self.flipped && !self.fullscreen =>
            {
                self.step(1);
                self.held = Some((1, now + 400));
            }
            Action::GbaUp(Btn::Left | Btn::Right) => self.held = None,
            Action::GbaDown(Btn::Up) if !self.fullscreen => self.jump(-1),
            Action::GbaDown(Btn::Down) if !self.fullscreen => self.jump(1),
            _ => {}
        }
    }
    pub fn release(&mut self) {
        self.held = None;
    }
    pub fn update(&mut self, dt: f32, now: u64) {
        if let Some((by, due)) = self.held {
            if now >= due {
                self.step(by);
                self.held = Some((by, now + 85));
            }
        }
        let dt = dt.min(0.04);
        self.velocity += (-32.0 * self.velocity - 256.0 * (self.scroll - self.ride)) * dt;
        self.scroll += self.velocity * dt;
        let target = if self.flipped { 1.0 } else { 0.0 };
        if self.flip < target {
            self.flip = (self.flip + dt / 0.32).min(target);
        } else {
            self.flip = (self.flip - dt / 0.32).max(target);
        }
    }
    pub fn visible_indices(&self) -> Vec<usize> {
        let mut slots = self.slots();
        slots.sort_by(|a, b| a.1.abs().total_cmp(&b.1.abs()));
        slots.into_iter().map(|(i, _)| i).collect()
    }
    fn slots(&self) -> Vec<(usize, f32)> {
        let n = self.albums.len();
        if n == 0 {
            return Vec::new();
        }
        let target = self.ride
            + (self.index as f32 - self.ride + n as f32 / 2.0).rem_euclid(n as f32)
            - n as f32 / 2.0;
        let mut out = Vec::new();
        for off in -4..=4 {
            let i = (self.index as i32 + off).rem_euclid(n as i32) as usize;
            let offset = target + off as f32 - self.scroll;
            if let Some((_, old)) = out.iter_mut().find(|(found, _)| *found == i) {
                if offset.abs() < f32::abs(*old) {
                    *old = offset;
                }
            } else {
                out.push((i, offset));
            }
        }
        out.sort_by(|a, b| b.1.abs().total_cmp(&a.1.abs()));
        out
    }
    pub fn draw(&self, now: u64, out: &mut Vec<Draw>) {
        if self.fullscreen {
            out.push(rect(0.0, 0.0, 720.0, 480.0, [0.015, 0.02, 0.025, 1.0]));
            if let Some(tex) = self.heading {
                out.push(texture(tex, 80.0, 18.0, 560.0, 66.0));
            }
            draw_visualizer(
                &self.snapshot.analysis,
                self.settings.style,
                40.0,
                96.0,
                640.0,
                300.0,
                out,
            );
            draw_progress(&self.snapshot, 40.0, 411.0, 640.0, out);
        } else {
            let n = self.albums.len();
            if n > 0 {
                for (i, offset) in self.slots() {
                    let (space, scale, yaw) = slot_ui::coverflow_pose(offset, COVER);
                    let size = COVER * scale;
                    let alpha =
                        (1.0 - 0.22 * offset.abs().min(3.0)).max(0.25) * (1.0 - self.flip * 0.75);
                    if i == self.index && self.flip > 0.0 {
                        continue;
                    }
                    if let Some(tex) = self.covers.get(&i) {
                        out.push(Draw::Yawed {
                            x: 360.0 + space - size / 2.0,
                            y: 108.0 + (COVER - size) / 2.0,
                            w: size,
                            h: size,
                            tex: Some(*tex),
                            colour: [1.0, 1.0, 1.0, alpha],
                            yaw,
                        });
                    }
                }
                if self.flip > 0.0 {
                    let p = slot_ui::ease(self.flip);
                    let w = COVER + (PANEL_W as f32 - COVER) * p;
                    let h = COVER + (PANEL_H as f32 - COVER) * p;
                    let front = p < 0.5;
                    let tex = if front {
                        self.covers.get(&self.index).copied()
                    } else {
                        self.panel
                    };
                    let yaw = if front {
                        p * std::f32::consts::PI
                    } else {
                        (p - 1.0) * std::f32::consts::PI
                    };
                    if let Some(tex) = tex {
                        out.push(Draw::Yawed {
                            x: 360.0 - w / 2.0,
                            y: 238.0 - h / 2.0,
                            w,
                            h,
                            tex: Some(tex),
                            colour: [1.0; 4],
                            yaw,
                        });
                    }
                    if self.flip >= 0.999 {
                        draw_visualizer(
                            &self.snapshot.analysis,
                            self.settings.style,
                            100.0,
                            337.0,
                            520.0,
                            48.0,
                            out,
                        );
                        draw_progress(&self.snapshot, 100.0, 397.0, 520.0, out);
                    }
                } else if let Some(tex) = self.heading {
                    out.push(texture(tex, 80.0, 24.0, 560.0, 66.0));
                }
            } else if let Some(tex) = self.heading {
                out.push(texture(tex, 80.0, 190.0, 560.0, 66.0));
            }
        }
        if let Some(tex) = self.footer {
            out.push(texture(tex, 60.0, 434.0, 600.0, 32.0));
        }
        self.draw_notice(now, out);
    }
    fn draw_notice(&self, now: u64, out: &mut Vec<Draw>) {
        if now < self.notice_until {
            out.push(rect(80.0, 25.0, 560.0, 44.0, [0.12, 0.025, 0.025, 0.98]));
            if let Some(tex) = self.notice {
                out.push(texture(tex, 80.0, 25.0, 560.0, 44.0));
            }
        }
    }
}
fn rect(x: f32, y: f32, w: f32, h: f32, colour: [f32; 4]) -> Draw {
    Draw::Rect { x, y, w, h, colour }
}
fn texture(tex: TexId, x: f32, y: f32, w: f32, h: f32) -> Draw {
    Draw::Tex {
        x,
        y,
        w,
        h,
        tex,
        alpha: 1.0,
    }
}

pub fn draw_visualizer(
    analysis: &Analysis,
    style: VisualizerStyle,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    out: &mut Vec<Draw>,
) {
    out.push(rect(x, y, w, h, [0.008, 0.012, 0.008, 1.0]));
    match style {
        VisualizerStyle::Bars => {
            let pitch = w / 32.0;
            let steps = if h > 100.0 { 32 } else { 12 };
            let unit = h / steps as f32;
            for (i, bar) in analysis.bars.iter().enumerate() {
                let count = (bar * steps as f32).round() as usize;
                for step in 0..count {
                    let fraction = step as f32 / steps as f32;
                    let colour = if fraction > 0.8 {
                        [1.0, 0.2, 0.07, 1.0]
                    } else if fraction > 0.6 {
                        [0.9, 0.85, 0.1, 1.0]
                    } else {
                        [0.1, 0.85, 0.18, 1.0]
                    };
                    out.push(rect(
                        x + i as f32 * pitch + 1.0,
                        y + h - (step + 1) as f32 * unit,
                        pitch - 2.0,
                        (unit - 1.0).max(1.0),
                        colour,
                    ));
                }
                out.push(rect(
                    x + i as f32 * pitch + 1.0,
                    y + h - analysis.peaks[i] * h,
                    pitch - 2.0,
                    1.0,
                    [0.8, 1.0, 0.7, 1.0],
                ));
            }
        }
        VisualizerStyle::Waveform => {
            let pitch = w / analysis.waveform.len() as f32;
            for (i, value) in analysis.waveform.iter().enumerate() {
                let previous = if i == 0 {
                    *value
                } else {
                    analysis.waveform[i - 1]
                };
                let a = y + h / 2.0 - *value * h * 0.45;
                let b = y + h / 2.0 - previous * h * 0.45;
                out.push(rect(
                    x + i as f32 * pitch,
                    a.min(b),
                    pitch.max(1.0),
                    (a - b).abs().max(1.0),
                    [0.15, 0.95, 0.28, 1.0],
                ));
            }
        }
    }
}
fn draw_progress(snapshot: &Snapshot, x: f32, y: f32, w: f32, out: &mut Vec<Draw>) {
    out.push(rect(x, y, w, 3.0, [0.18, 0.23, 0.19, 1.0]));
    let fraction = snapshot
        .duration
        .filter(|d| *d > 0.0)
        .map_or(0.0, |d| (snapshot.position / d).clamp(0.0, 1.0)) as f32;
    out.push(rect(x, y, w * fraction, 3.0, [0.2, 0.9, 0.3, 1.0]));
}
pub fn time(seconds: Option<f64>) -> String {
    seconds.map_or_else(
        || "--:--".into(),
        |s| format!("{}:{:02}", s.max(0.0) as u64 / 60, s.max(0.0) as u64 % 60),
    )
}

#[allow(clippy::too_many_arguments)] // Pixel buffer, rectangle and text style.
fn text(
    dst: &mut [u8],
    w: u32,
    h: u32,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    label: &str,
    px: f32,
    colour: [u8; 3],
) {
    let Some(font) = slot_ui::text::label_font() else {
        return;
    };
    let layout = slot_ui::text::fit(font, label, width as f32, 1, px, px - 2.0);
    let mut face = vec![0u8; (width * height * 4) as usize];
    slot_ui::text::draw_centred(&mut face, width, height, &layout, colour);
    for cy in 0..height.min(h.saturating_sub(y)) {
        for cx in 0..width.min(w.saturating_sub(x)) {
            let a = ((cy * width + cx) * 4) as usize;
            let b = (((cy + y) * w + cx + x) * 4) as usize;
            let alpha = face[a + 3] as u32;
            for c in 0..3 {
                dst[b + c] =
                    ((face[a + c] as u32 * alpha + dst[b + c] as u32 * (255 - alpha)) / 255) as u8;
            }
            dst[b + 3] = dst[b + 3].max(face[a + 3]);
        }
    }
}
fn fill(dst: &mut [u8], w: u32, x: u32, y: u32, width: u32, height: u32, colour: [u8; 4]) {
    for cy in y..y + height {
        for cx in x..x + width {
            let at = ((cy * w + cx) * 4) as usize;
            dst[at..at + 4].copy_from_slice(&colour);
        }
    }
}
pub fn panel_face(browser: &Browser) -> Vec<u8> {
    let mut rgba = vec![0; (PANEL_W * PANEL_H * 4) as usize];
    fill(
        &mut rgba,
        PANEL_W,
        0,
        0,
        PANEL_W,
        PANEL_H,
        [24, 28, 29, 255],
    );
    let Some(album) = browser.selected() else {
        return rgba;
    };
    text(
        &mut rgba,
        PANEL_W,
        PANEL_H,
        16,
        10,
        528,
        28,
        &album.title,
        22.0,
        [240, 236, 219],
    );
    text(
        &mut rgba,
        PANEL_W,
        PANEL_H,
        16,
        39,
        528,
        22,
        &album.artist,
        16.0,
        [163, 179, 163],
    );
    let start = browser.track.saturating_sub(5);
    for (row, (index, track)) in album
        .tracks
        .iter()
        .enumerate()
        .skip(start)
        .take(6)
        .enumerate()
    {
        let y = 72 + row as u32 * 31;
        if index == browser.track {
            fill(&mut rgba, PANEL_W, 12, y, 536, 30, [48, 67, 52, 255]);
        }
        let playing = browser.snapshot.album.as_ref().is_some_and(|a| {
            a.path == album.path
                && a.tracks
                    .get(browser.snapshot.index)
                    .is_some_and(|t| t.path == track.path)
        });
        text(
            &mut rgba,
            PANEL_W,
            PANEL_H,
            14,
            y,
            54,
            30,
            &format!(
                "{}{:02}",
                if playing { ">" } else { "" },
                track.number.unwrap_or(index as u32 + 1)
            ),
            16.0,
            [110, 225, 132],
        );
        text(
            &mut rgba,
            PANEL_W,
            PANEL_H,
            70,
            y,
            394,
            30,
            &track.title,
            17.0,
            [232, 235, 226],
        );
        text(
            &mut rgba,
            PANEL_W,
            PANEL_H,
            468,
            y,
            76,
            30,
            &time(track.duration),
            16.0,
            [171, 184, 174],
        );
    }
    text(
        &mut rgba,
        PANEL_W,
        PANEL_H,
        20,
        258,
        520,
        19,
        &format!(
            "{} / {}   {} / {}",
            browser.track + 1,
            album.tracks.len(),
            time(Some(browser.snapshot.position)),
            time(browser.snapshot.duration)
        ),
        13.0,
        [142, 171, 146],
    );
    rgba
}
fn label_face(title: &str, subtitle: &str, w: u32, h: u32) -> Vec<u8> {
    let mut rgba = vec![0; (w * h * 4) as usize];
    text(
        &mut rgba,
        w,
        h,
        0,
        0,
        w,
        h / 2,
        title,
        if h > 40 { 22.0 } else { 14.0 },
        [232, 236, 223],
    );
    text(
        &mut rgba,
        w,
        h,
        0,
        h / 2,
        w,
        h / 2,
        subtitle,
        if h > 40 { 15.0 } else { 12.0 },
        [162, 186, 165],
    );
    rgba
}

fn placeholder(album: &Album) -> Vec<u8> {
    let mut rgba = vec![0; 280 * 280 * 4];
    fill(&mut rgba, 280, 0, 0, 280, 280, [36, 58, 46, 255]);
    text(
        &mut rgba,
        280,
        280,
        14,
        92,
        252,
        58,
        &album.title,
        24.0,
        [227, 233, 215],
    );
    text(
        &mut rgba,
        280,
        280,
        14,
        155,
        252,
        32,
        &album.artist,
        16.0,
        [158, 202, 167],
    );
    rgba
}
fn read_limited(path: &Path, max: usize) -> Option<Vec<u8>> {
    use std::io::Read;
    let file = std::fs::File::open(path).ok()?;
    if file.metadata().ok()?.len() > max as u64 {
        return None;
    }
    let mut bytes = Vec::new();
    file.take(max as u64 + 1).read_to_end(&mut bytes).ok()?;
    (bytes.len() <= max).then_some(bytes)
}

fn art(root: &Path, album: &Album) -> Vec<u8> {
    let Some(source) = &album.artwork else {
        return placeholder(album);
    };
    let path = match source {
        Artwork::Local(p) | Artwork::Embedded(p) => p,
    };
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    album.path.hash(&mut hash);
    album.revision.hash(&mut hash);
    path.hash(&mut hash);
    if let Ok(meta) = std::fs::metadata(root.join(path)) {
        meta.len().hash(&mut hash);
        meta.modified().ok().hash(&mut hash);
    }
    let cache = root
        .join("System/Cache/Music")
        .join(format!("{:016x}.rgba", hash.finish()));
    if let Some(bytes) = read_limited(&cache, 280 * 280 * 4) {
        if bytes.len() == 280 * 280 * 4 {
            return bytes;
        }
    }
    let bytes = match source {
        Artwork::Local(p) => read_limited(&root.join(p), 20 * 1024 * 1024),
        Artwork::Embedded(p) => slot_store::music::embedded_art(root, p),
    };
    let image = bytes.filter(|b| b.len() <= 20 * 1024 * 1024).and_then(|b| {
        let mut reader = image::ImageReader::new(std::io::Cursor::new(b))
            .with_guessed_format()
            .ok()?;
        let mut limits = image::Limits::default();
        limits.max_alloc = Some(64 * 1024 * 1024);
        limits.max_image_width = Some(4096);
        limits.max_image_height = Some(4096);
        reader.limits(limits);
        reader.decode().ok()
    });
    let rgba = image.map_or_else(
        || placeholder(album),
        |img| {
            img.resize_to_fill(280, 280, image::imageops::FilterType::Triangle)
                .into_rgba8()
                .into_raw()
        },
    );
    if std::fs::create_dir_all(cache.parent().unwrap()).is_ok() {
        let _ = slot_store::atomic_write(&cache, &rgba);
    }
    rgba
}

pub struct Faces {
    tx: mpsc::SyncSender<(u64, usize, Album)>,
    rx: mpsc::Receiver<(u64, usize, Vec<u8>)>,
    generation: u64,
    requested: HashSet<usize>,
    slots: HashMap<usize, TexId>,
    spare: Vec<TexId>,
    panel: Option<TexId>,
    heading: Option<TexId>,
    footer: Option<TexId>,
    notice: Option<TexId>,
    panel_key: String,
    heading_key: String,
    footer_key: String,
    error_id: u64,
    library_key: u64,
}
impl Faces {
    pub fn new(root: PathBuf) -> Self {
        let (tx, requests) = mpsc::sync_channel::<(u64, usize, Album)>(9);
        let (results, rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("slot-music-art".into())
            .spawn(move || {
                for (generation, index, album) in requests {
                    if results
                        .send((generation, index, art(&root, &album)))
                        .is_err()
                    {
                        break;
                    }
                }
            })
            .expect("music art worker");
        Self {
            tx,
            rx,
            generation: 0,
            requested: HashSet::new(),
            slots: HashMap::new(),
            spare: Vec::new(),
            panel: None,
            heading: None,
            footer: None,
            notice: None,
            panel_key: String::new(),
            heading_key: String::new(),
            footer_key: String::new(),
            error_id: 0,
            library_key: 0,
        }
    }
    pub fn sync(
        &mut self,
        browser: &mut Browser,
        compositor: &mut Compositor,
        visible: bool,
        now: u64,
    ) {
        if self.library_key != browser.revision {
            self.generation += 1;
            self.library_key = browser.revision;
            self.requested.clear();
            self.spare.extend(self.slots.drain().map(|(_, tex)| tex));
            browser.covers.clear();
            self.panel_key.clear();
        }
        let wants = if visible {
            browser.visible_indices()
        } else {
            Vec::new()
        };
        for index in self.slots.keys().copied().collect::<Vec<_>>() {
            if !wants.contains(&index) {
                self.spare.push(self.slots.remove(&index).unwrap());
                self.requested.remove(&index);
            }
        }
        for index in &wants {
            if !self.requested.contains(index)
                && self
                    .tx
                    .try_send((self.generation, *index, browser.albums[*index].clone()))
                    .is_ok()
            {
                self.requested.insert(*index);
            }
        }
        while let Ok((generation, index, rgba)) = self.rx.try_recv() {
            if generation != self.generation {
                continue;
            }
            if !wants.contains(&index) {
                self.requested.remove(&index);
                continue;
            }
            let tex = self
                .spare
                .pop()
                .inspect(|&tex| {
                    compositor.update_texture(tex, 280, 280, &rgba);
                })
                .unwrap_or_else(|| compositor.create_texture(280, 280, &rgba));
            if let Some(old) = self.slots.insert(index, tex) {
                self.spare.push(old);
            }
        }
        browser.covers = self.slots.clone();
        let (title, subtitle) = if browser.fullscreen {
            (
                browser.snapshot.title().unwrap_or("Music").to_string(),
                browser
                    .snapshot
                    .album
                    .as_ref()
                    .map_or(String::new(), |a| a.artist.clone()),
            )
        } else {
            browser.selected().map_or_else(
                || {
                    (
                        if browser.scanning {
                            "Scanning Music"
                        } else {
                            "Music"
                        }
                        .into(),
                        "Add MP3 or FLAC albums to Games/Music".into(),
                    )
                },
                |a| (a.title.clone(), a.artist.clone()),
            )
        };
        let heading_key = format!("{title}\n{subtitle}");
        if self.heading.is_none() || heading_key != self.heading_key {
            upload(
                compositor,
                &mut self.heading,
                560,
                66,
                &label_face(&title, &subtitle, 560, 66),
            );
            self.heading_key = heading_key;
        }
        browser.heading = self.heading;
        if visible && browser.flip > 0.0 {
            let key = format!(
                "{:?}|{}|{}|{}|{:?}|{}",
                browser.selected().map(|a| &a.path),
                browser.track,
                browser.snapshot.index,
                browser.snapshot.position as u64,
                browser.snapshot.album.as_ref().map(|a| &a.path),
                browser.snapshot.playing
            );
            if self.panel.is_none() || key != self.panel_key {
                upload(
                    compositor,
                    &mut self.panel,
                    PANEL_W,
                    PANEL_H,
                    &panel_face(browser),
                );
                self.panel_key = key;
            }
            browser.panel = self.panel;
        }
        let title = browser.snapshot.title().map_or_else(
            || "A Tracks   L/R Shelves   Start Play".into(),
            |title| {
                format!(
                    "{}  {}  {} / {}",
                    if browser.snapshot.playing {
                        "Playing"
                    } else {
                        "Paused"
                    },
                    title,
                    time(Some(browser.snapshot.position)),
                    time(browser.snapshot.duration)
                )
            },
        );
        let hint = if browser.fullscreen {
            "Left/Right Style  B Back  Start Pause  L/R Tracks"
        } else if browser.flipped {
            "A Play  B Covers  Start Pause  X Visualizer  L2/R2 Seek"
        } else {
            "A Tracks  Start Play/Pause  X Visualizer  L/R Shelves"
        };
        let key = format!("{title}\n{hint}");
        if self.footer.is_none() || key != self.footer_key {
            upload(
                compositor,
                &mut self.footer,
                600,
                32,
                &label_face(&title, hint, 600, 32),
            );
            self.footer_key = key;
        }
        browser.footer = self.footer;
        if browser.snapshot.error_id != self.error_id {
            self.error_id = browser.snapshot.error_id;
            browser.notice_until = now + 5000;
            upload(
                compositor,
                &mut self.notice,
                560,
                44,
                &label_face(
                    "Music playback error",
                    browser.snapshot.error.as_deref().unwrap_or(""),
                    560,
                    44,
                ),
            );
        }
        browser.notice = self.notice;
    }
}
fn upload(compositor: &mut Compositor, slot: &mut Option<TexId>, w: u32, h: u32, rgba: &[u8]) {
    if let Some(tex) = *slot {
        compositor.update_texture(tex, w, h, rgba);
    } else {
        *slot = Some(compositor.create_texture(w, h, rgba));
    }
}

#[cfg(test)]
mod artwork_tests {
    use super::{art, placeholder, read_limited};

    #[test]
    fn changed_cover_invalidates_cache_and_corrupt_cover_uses_placeholder() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("Games/Music/Album");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(
            folder.join("track.mp3"),
            b"corrupt audio still defines an album",
        )
        .unwrap();
        let cover = folder.join("cover.png");
        image::RgbImage::from_pixel(16, 16, image::Rgb([200, 10, 20]))
            .save(&cover)
            .unwrap();
        let first = slot_store::music::scan(dir.path()).remove(0);
        let red = art(dir.path(), &first);
        assert_eq!(red, art(dir.path(), &first));
        image::RgbImage::from_pixel(32, 32, image::Rgb([10, 200, 20]))
            .save(&cover)
            .unwrap();
        let changed = slot_store::music::scan(dir.path()).remove(0);
        assert_ne!(first.revision, changed.revision);
        assert_ne!(red, art(dir.path(), &changed));
        std::fs::write(&cover, b"not an image").unwrap();
        let corrupt = slot_store::music::scan(dir.path()).remove(0);
        assert_eq!(art(dir.path(), &corrupt), placeholder(&corrupt));
        assert!(read_limited(&cover, 1).is_none());
    }
}
