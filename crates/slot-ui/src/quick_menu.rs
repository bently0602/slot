use crate::draw::{Draw, TexId, OUT_H, OUT_W};
use crate::plate::{arrows_hint_face, centred_hints, hint_face, UndoFace, HINT_H, LEGEND_GAP};
use crate::power_menu::{MENU_H, MENU_INK, MENU_PAD, MENU_PX};
use crate::slot_chrome::{edge, opening};
use crate::text;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum QuickRow {
    FastForward,
    FastForwardSound,
    Screen,
    Rumble,
    DateTime,
    About,
    GbaShader,
    GbShader,
    ColourCorrection,
    Game,
    EjectSave,
    Turbo,
    Rewind,
    GbPalettes,
    CoverFlow,
    VideoOut,
    HdmiOutput,
    HdmiResolution,
    HdmiScale,
    HdmiMargin,
    Controllers,
    ControllerSelect,
    ControllerSwapAb,
    ControllerSwapXy,
    ControllerDeadzone,
    ControllerPause,
    ControllerReset,
    DockedLid,
    AudioOutput,
    DeviceInfo,
    ControllerPlayers,
    Music,
    MusicPlay,
    MusicPrevious,
    MusicNext,
    MusicShuffle,
    MusicRepeat,
    MusicStyle,
    MusicLid,
    MusicRescan,
    Bluetooth,
    BluetoothScan,
    BluetoothSelect,
    BluetoothConnect,
    BluetoothDisconnect,
    BluetoothForget,
    BluetoothStatus,
    Battery,
    BatteryStatus,
    SleepShutdown,
    ShutdownCharging,
}

impl QuickRow {
    pub const ALL: [QuickRow; 51] = [
        QuickRow::FastForward,
        QuickRow::FastForwardSound,
        QuickRow::Screen,
        QuickRow::Rumble,
        QuickRow::DateTime,
        QuickRow::About,
        QuickRow::GbaShader,
        QuickRow::GbShader,
        QuickRow::ColourCorrection,
        QuickRow::Game,
        QuickRow::EjectSave,
        QuickRow::Turbo,
        QuickRow::Rewind,
        QuickRow::GbPalettes,
        QuickRow::CoverFlow,
        QuickRow::VideoOut,
        QuickRow::HdmiOutput,
        QuickRow::HdmiResolution,
        QuickRow::HdmiScale,
        QuickRow::HdmiMargin,
        QuickRow::Controllers,
        QuickRow::ControllerSelect,
        QuickRow::ControllerSwapAb,
        QuickRow::ControllerSwapXy,
        QuickRow::ControllerDeadzone,
        QuickRow::ControllerPause,
        QuickRow::ControllerReset,
        QuickRow::DockedLid,
        QuickRow::AudioOutput,
        QuickRow::DeviceInfo,
        QuickRow::ControllerPlayers,
        QuickRow::Music,
        QuickRow::MusicPlay,
        QuickRow::MusicPrevious,
        QuickRow::MusicNext,
        QuickRow::MusicShuffle,
        QuickRow::MusicRepeat,
        QuickRow::MusicStyle,
        QuickRow::MusicLid,
        QuickRow::MusicRescan,
        QuickRow::Bluetooth,
        QuickRow::BluetoothScan,
        QuickRow::BluetoothSelect,
        QuickRow::BluetoothConnect,
        QuickRow::BluetoothDisconnect,
        QuickRow::BluetoothForget,
        QuickRow::BluetoothStatus,
        QuickRow::Battery,
        QuickRow::BatteryStatus,
        QuickRow::SleepShutdown,
        QuickRow::ShutdownCharging,
    ];

    pub const MAIN: [QuickRow; 10] = [
        QuickRow::Screen,
        QuickRow::VideoOut,
        QuickRow::Controllers,
        QuickRow::Bluetooth,
        QuickRow::Game,
        QuickRow::Music,
        QuickRow::Battery,
        QuickRow::DateTime,
        QuickRow::DeviceInfo,
        QuickRow::About,
    ];

    pub const SCREEN: [QuickRow; 5] = [
        QuickRow::GbaShader,
        QuickRow::GbShader,
        QuickRow::ColourCorrection,
        QuickRow::GbPalettes,
        QuickRow::CoverFlow,
    ];

    pub const MUSIC: [QuickRow; 8] = [
        QuickRow::MusicPlay,
        QuickRow::MusicPrevious,
        QuickRow::MusicNext,
        QuickRow::MusicShuffle,
        QuickRow::MusicRepeat,
        QuickRow::MusicStyle,
        QuickRow::MusicLid,
        QuickRow::MusicRescan,
    ];

    pub const GAME: [QuickRow; 6] = [
        QuickRow::FastForward,
        QuickRow::FastForwardSound,
        QuickRow::Rewind,
        QuickRow::Turbo,
        QuickRow::Rumble,
        QuickRow::EjectSave,
    ];

    pub fn index(self) -> usize {
        self as usize
    }

    pub const VIDEO: [QuickRow; 6] = [
        QuickRow::HdmiOutput,
        QuickRow::HdmiResolution,
        QuickRow::HdmiScale,
        QuickRow::HdmiMargin,
        QuickRow::DockedLid,
        QuickRow::AudioOutput,
    ];

    pub const CONTROLLERS: [QuickRow; 7] = [
        QuickRow::ControllerPlayers,
        QuickRow::ControllerSelect,
        QuickRow::ControllerSwapAb,
        QuickRow::ControllerSwapXy,
        QuickRow::ControllerDeadzone,
        QuickRow::ControllerPause,
        QuickRow::ControllerReset,
    ];

    pub fn page(self) -> &'static [QuickRow] {
        match self {
            QuickRow::BatteryStatus | QuickRow::SleepShutdown | QuickRow::ShutdownCharging => {
                &QuickRow::BATTERY
            }
            QuickRow::BluetoothScan
            | QuickRow::BluetoothSelect
            | QuickRow::BluetoothConnect
            | QuickRow::BluetoothDisconnect
            | QuickRow::BluetoothForget
            | QuickRow::BluetoothStatus => &QuickRow::BLUETOOTH,
            QuickRow::MusicPlay
            | QuickRow::MusicPrevious
            | QuickRow::MusicNext
            | QuickRow::MusicShuffle
            | QuickRow::MusicRepeat
            | QuickRow::MusicStyle
            | QuickRow::MusicLid
            | QuickRow::MusicRescan => &QuickRow::MUSIC,
            QuickRow::ControllerPlayers
            | QuickRow::ControllerSelect
            | QuickRow::ControllerSwapAb
            | QuickRow::ControllerSwapXy
            | QuickRow::ControllerDeadzone
            | QuickRow::ControllerPause
            | QuickRow::ControllerReset => &QuickRow::CONTROLLERS,
            QuickRow::HdmiOutput
            | QuickRow::HdmiResolution
            | QuickRow::HdmiScale
            | QuickRow::HdmiMargin
            | QuickRow::DockedLid
            | QuickRow::AudioOutput => &QuickRow::VIDEO,
            QuickRow::GbaShader
            | QuickRow::GbShader
            | QuickRow::ColourCorrection
            | QuickRow::GbPalettes
            | QuickRow::CoverFlow => &QuickRow::SCREEN,
            QuickRow::FastForward
            | QuickRow::FastForwardSound
            | QuickRow::Rewind
            | QuickRow::Turbo
            | QuickRow::Rumble
            | QuickRow::EjectSave => &QuickRow::GAME,
            _ => &QuickRow::MAIN,
        }
    }

    pub fn parent(self) -> Option<QuickRow> {
        match self.page()[0] {
            QuickRow::BatteryStatus => Some(QuickRow::Battery),
            QuickRow::BluetoothScan => Some(QuickRow::Bluetooth),
            QuickRow::MusicPlay => Some(QuickRow::Music),
            QuickRow::GbaShader => Some(QuickRow::Screen),
            QuickRow::FastForward => Some(QuickRow::Game),
            QuickRow::HdmiOutput => Some(QuickRow::VideoOut),
            QuickRow::ControllerPlayers => Some(QuickRow::Controllers),
            _ => None,
        }
    }

    pub fn child(self) -> Option<QuickRow> {
        match self {
            QuickRow::Battery => Some(QuickRow::BatteryStatus),
            QuickRow::Bluetooth => Some(QuickRow::BluetoothScan),
            QuickRow::Music => Some(QuickRow::MUSIC[0]),
            QuickRow::Screen => Some(QuickRow::SCREEN[0]),
            QuickRow::Game => Some(QuickRow::GAME[0]),
            QuickRow::VideoOut => Some(QuickRow::VIDEO[0]),
            QuickRow::Controllers => Some(QuickRow::CONTROLLERS[0]),
            _ => None,
        }
    }

    pub fn position(self) -> usize {
        self.page().iter().position(|r| *r == self).unwrap_or(0)
    }

    pub fn label(self) -> &'static str {
        match self {
            QuickRow::Battery => "Battery",
            QuickRow::BatteryStatus => "Status",
            QuickRow::SleepShutdown => "Shutdown After Sleep",
            QuickRow::ShutdownCharging => "Shutdown While Charging",
            QuickRow::Bluetooth => "Bluetooth",
            QuickRow::BluetoothScan => "Scan for Headphones",
            QuickRow::BluetoothSelect => "Headphones",
            QuickRow::BluetoothConnect => "Connect",
            QuickRow::BluetoothDisconnect => "Disconnect",
            QuickRow::BluetoothForget => "Forget Headphones",
            QuickRow::BluetoothStatus => "Status",
            QuickRow::Music => "Music",
            QuickRow::MusicPlay => "Play / Pause",
            QuickRow::MusicPrevious => "Previous Track",
            QuickRow::MusicNext => "Next Track",
            QuickRow::MusicShuffle => "Shuffle",
            QuickRow::MusicRepeat => "Repeat",
            QuickRow::MusicStyle => "Visualizer",
            QuickRow::MusicLid => "Play With Lid Closed",
            QuickRow::MusicRescan => "Rescan Library",
            QuickRow::FastForward => "Fast Forward",
            QuickRow::FastForwardSound => "Fast Forward Sound",
            QuickRow::Screen => "Screen",
            QuickRow::Rumble => "Rumble",
            QuickRow::DateTime => "Date & Time",
            QuickRow::About => "About",
            QuickRow::DeviceInfo => "Device Info",
            QuickRow::GbaShader => "GBA Shader",
            QuickRow::GbShader => "GB / GBC Shader",
            QuickRow::ColourCorrection => "Color Correction",
            QuickRow::Game => "Gameplay",
            QuickRow::EjectSave => "Auto Save on Eject",
            QuickRow::Turbo => "Turbo Buttons",
            QuickRow::Rewind => "Rewind",
            QuickRow::GbPalettes => "GB Palettes",
            QuickRow::CoverFlow => "Cover Flow",
            QuickRow::VideoOut => "Video Out",
            QuickRow::HdmiOutput => "Output (restart)",
            QuickRow::HdmiResolution => "Resolution (restart)",
            QuickRow::HdmiScale => "HDMI Scaling",
            QuickRow::HdmiMargin => "HDMI Safe Area",
            QuickRow::Controllers => "Controllers",
            QuickRow::ControllerPlayers => "Players",
            QuickRow::ControllerSelect => "Controller",
            QuickRow::ControllerSwapAb => "Swap A / B",
            QuickRow::ControllerSwapXy => "Swap X / Y",
            QuickRow::ControllerDeadzone => "Stick Dead Zone",
            QuickRow::ControllerPause => "Pause if Lost",
            QuickRow::ControllerReset => "Reset Profile",
            QuickRow::DockedLid => "Keep Awake Docked",
            QuickRow::AudioOutput => "Audio Output",
        }
    }

    pub fn opens(self) -> bool {
        matches!(
            self,
            QuickRow::DateTime
                | QuickRow::Bluetooth
                | QuickRow::BluetoothScan
                | QuickRow::BluetoothConnect
                | QuickRow::BluetoothDisconnect
                | QuickRow::BluetoothForget
                | QuickRow::BluetoothStatus
                | QuickRow::Music
                | QuickRow::MusicPlay
                | QuickRow::MusicPrevious
                | QuickRow::MusicNext
                | QuickRow::MusicStyle
                | QuickRow::MusicRescan
                | QuickRow::About
                | QuickRow::DeviceInfo
                | QuickRow::Screen
                | QuickRow::Game
                | QuickRow::VideoOut
                | QuickRow::Controllers
                | QuickRow::ControllerPlayers
                | QuickRow::ControllerReset
                | QuickRow::Battery
                | QuickRow::BatteryStatus
        )
    }

    pub fn up(self) -> QuickRow {
        let page = self.page();
        page[(self.position() + page.len() - 1) % page.len()]
    }

    pub const BLUETOOTH: [QuickRow; 6] = [
        QuickRow::BluetoothScan,
        QuickRow::BluetoothSelect,
        QuickRow::BluetoothConnect,
        QuickRow::BluetoothDisconnect,
        QuickRow::BluetoothForget,
        QuickRow::BluetoothStatus,
    ];

    pub const BATTERY: [QuickRow; 3] = [
        QuickRow::BatteryStatus,
        QuickRow::SleepShutdown,
        QuickRow::ShutdownCharging,
    ];

    pub fn down(self) -> QuickRow {
        let page = self.page();
        page[(self.position() + 1) % page.len()]
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum QuickValue {
    Speed2,
    Speed3,
    Speed4,
    Speed6,
    On,
    Off,
    Lcd3x,
    Grid,
    Dot,
    Simpletex,
    Auto,
    Internal,
    P720,
    P1080,
    Fit,
    Integer,
    Stretch,
    Percent5,
    Percent10,
    Percent15,
    Percent20,
    Percent25,
    Percent30,
    Percent40,
    Device,
    Hdmi,
    Album,
    Track,
    Bars,
    Waveform,
    Playing,
    Paused,
    Minute1,
    Minutes3,
    Minutes5,
    Minutes10,
    Minutes30,
    Never,
}

impl QuickValue {
    pub const ALL: [QuickValue; 38] = [
        QuickValue::Speed2,
        QuickValue::Speed3,
        QuickValue::Speed4,
        QuickValue::Speed6,
        QuickValue::On,
        QuickValue::Off,
        QuickValue::Lcd3x,
        QuickValue::Grid,
        QuickValue::Dot,
        QuickValue::Simpletex,
        QuickValue::Auto,
        QuickValue::Internal,
        QuickValue::P720,
        QuickValue::P1080,
        QuickValue::Fit,
        QuickValue::Integer,
        QuickValue::Stretch,
        QuickValue::Percent5,
        QuickValue::Percent10,
        QuickValue::Percent15,
        QuickValue::Percent20,
        QuickValue::Percent25,
        QuickValue::Percent30,
        QuickValue::Percent40,
        QuickValue::Device,
        QuickValue::Hdmi,
        QuickValue::Album,
        QuickValue::Track,
        QuickValue::Bars,
        QuickValue::Waveform,
        QuickValue::Playing,
        QuickValue::Paused,
        QuickValue::Minute1,
        QuickValue::Minutes3,
        QuickValue::Minutes5,
        QuickValue::Minutes10,
        QuickValue::Minutes30,
        QuickValue::Never,
    ];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn text(self) -> &'static str {
        match self {
            QuickValue::Minute1 => "1 Minute",
            QuickValue::Minutes3 => "3 Minutes",
            QuickValue::Minutes5 => "5 Minutes",
            QuickValue::Minutes10 => "10 Minutes",
            QuickValue::Minutes30 => "30 Minutes",
            QuickValue::Never => "Never",
            QuickValue::Album => "Album",
            QuickValue::Track => "Track",
            QuickValue::Bars => "Bars",
            QuickValue::Waveform => "Waveform",
            QuickValue::Playing => "Playing",
            QuickValue::Paused => "Paused",
            QuickValue::Device => "Device",
            QuickValue::Hdmi => "HDMI",
            QuickValue::Speed2 => "2×",
            QuickValue::Speed3 => "3×",
            QuickValue::Speed4 => "4×",
            QuickValue::Speed6 => "6×",
            QuickValue::On => "On",
            QuickValue::Off => "Off",
            QuickValue::Lcd3x => "LCD3x",
            QuickValue::Grid => "Grid",
            QuickValue::Dot => "Dot",
            QuickValue::Simpletex => "Simpletex",
            QuickValue::Auto => "Auto",
            QuickValue::Internal => "Internal",
            QuickValue::P720 => "720p / 60",
            QuickValue::P1080 => "1080p / 60",
            QuickValue::Fit => "Fit",
            QuickValue::Integer => "Integer",
            QuickValue::Stretch => "Stretch",
            QuickValue::Percent5 => "5%",
            QuickValue::Percent10 => "10%",
            QuickValue::Percent15 => "15%",
            QuickValue::Percent20 => "20%",
            QuickValue::Percent25 => "25%",
            QuickValue::Percent30 => "30%",
            QuickValue::Percent40 => "40%",
        }
    }

    pub fn speed(frames: u8) -> Option<QuickValue> {
        match frames {
            2 => Some(QuickValue::Speed2),
            3 => Some(QuickValue::Speed3),
            4 => Some(QuickValue::Speed4),
            6 => Some(QuickValue::Speed6),
            _ => None,
        }
    }

    pub fn flag(on: bool) -> QuickValue {
        if on {
            QuickValue::On
        } else {
            QuickValue::Off
        }
    }
}

pub const QUICK_PITCH: f32 = 52.0;
pub const QUICK_MAIN_PITCH: f32 = 38.0;
pub const QUICK_TOP: f32 = 24.0;
pub const QUICK_EDGE: f32 = 32.0;
const BAR_INSET: f32 = 4.0;
const TYPE_DROP: f32 = 4.0;
const CARET_GAP: f32 = 14.0;
const CARET_PX: f32 = 24.0;
const LEGEND_Y: f32 = 427.0;
pub const QUICK_SPLIT: f32 = LEGEND_Y - 2.0;
const DIM_INK: [u8; 3] = [0x9a, 0x9a, 0xa4];

pub fn quick_label_face(row: QuickRow) -> UndoFace {
    quick_text_face(row.label(), MENU_INK)
}

pub fn quick_value_face(text: &str, lit: bool) -> UndoFace {
    quick_text_face(text, if lit { MENU_INK } else { DIM_INK })
}

pub fn quick_bluetooth_value_face(label: &str, lit: bool, width: f32) -> UndoFace {
    let Some(font) = text::label_font() else {
        return UndoFace {
            rgba: Vec::new(),
            w: 0,
            h: 0,
        };
    };
    let layout = text::fit(font, label, width - 2.0 * MENU_PAD as f32, 1, MENU_PX, 18.0);
    let w = width as u32;
    let mut rgba = vec![0; (w * MENU_H * 4) as usize];
    text::draw_centred(
        &mut rgba,
        w,
        MENU_H,
        &layout,
        if lit { MENU_INK } else { DIM_INK },
    );
    UndoFace { rgba, w, h: MENU_H }
}

fn quick_text_face(label: &str, colour: [u8; 3]) -> UndoFace {
    let Some(font) = text::label_font() else {
        return UndoFace {
            rgba: Vec::new(),
            w: 0,
            h: 0,
        };
    };
    let layout = text::fit(font, label, OUT_W as f32, 1, MENU_PX, MENU_PX);
    let set = layout
        .lines
        .iter()
        .map(|l| text::line_width(font, l, layout.px, layout.tracking))
        .fold(0.0, f32::max);
    let w = set.ceil() as u32 + 2 * MENU_PAD;
    let mut rgba = vec![0u8; (w * MENU_H * 4) as usize];
    text::draw_centred(&mut rgba, w, MENU_H, &layout, colour);
    UndoFace { rgba, w, h: MENU_H }
}

pub fn quick_caret_face(right: bool) -> UndoFace {
    let glyph = if right { '\u{f0da}' } else { '\u{f0d9}' };
    let (Some(symbols), Some(label)) = (crate::icon::symbols_font(), text::label_font()) else {
        return UndoFace {
            rgba: Vec::new(),
            w: 0,
            h: 0,
        };
    };
    let (m, cov) = symbols.rasterize(glyph, CARET_PX);
    let (w, h) = (m.width as u32, MENU_H);
    let centre = match label.horizontal_line_metrics(MENU_PX) {
        Some(v) => {
            (h as f32 - v.new_line_size) / 2.0 + v.ascent
                - label.metrics('H', MENU_PX).height as f32 / 2.0
        }
        None => h as f32 / 2.0,
    };
    let top = (centre - m.height as f32 / 2.0).round() as i32;
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    for gy in 0..m.height {
        let y = top + gy as i32;
        if y < 0 || y >= h as i32 {
            continue;
        }
        for gx in 0..m.width {
            let at = ((y as u32 * w + gx as u32) * 4) as usize;
            let a = cov[gy * m.width + gx];
            rgba[at..at + 4].copy_from_slice(&[MENU_INK[0], MENU_INK[1], MENU_INK[2], a]);
        }
    }
    UndoFace { rgba, w, h }
}

pub fn quick_legend_faces() -> [UndoFace; 3] {
    [
        hint_face("B", "Back"),
        arrows_hint_face("Change"),
        hint_face("A", "Open"),
    ]
}

pub struct QuickMenuFaces {
    pub labels: Vec<(TexId, u32, u32)>,
    pub values: Vec<[(TexId, u32, u32); 2]>,
    pub carets: [(TexId, u32, u32); 2],
    pub legend: [(TexId, u32); 3],
}

pub struct QuickMenu<'a> {
    pub row: QuickRow,
    pub values: [Option<QuickValue>; QuickRow::ALL.len()],
    pub clock: Option<[(TexId, u32, u32); 2]>,
    pub controller: Option<[(TexId, u32, u32); 2]>,
    pub bluetooth: [Option<[(TexId, u32, u32); 2]>; 2],
    pub battery: Option<[(TexId, u32, u32); 2]>,
    pub faces: Option<&'a QuickMenuFaces>,
}

impl QuickMenu<'_> {
    pub fn draw(&self, out: &mut Vec<Draw>) {
        out.push(Draw::Rect {
            x: 0.0,
            y: 0.0,
            w: OUT_W as f32,
            h: OUT_H as f32,
            colour: opening(),
        });
        out.push(Draw::Rect {
            x: 0.0,
            y: row_top(self.row) + BAR_INSET,
            w: OUT_W as f32,
            h: row_pitch(self.row) - 2.0 * BAR_INSET,
            colour: edge(),
        });
        let Some(faces) = self.faces else {
            return;
        };
        let (right, pad) = (OUT_W as f32 - QUICK_EDGE, MENU_PAD as f32);
        for &row in self.row.page() {
            let y = row_top(row) + TYPE_DROP;
            let lit = row == self.row;
            if let Some(&(tex, w, h)) = faces.labels.get(row.index()) {
                push(out, tex, QUICK_EDGE - pad, y, w, h);
            }
            let value = match row {
                QuickRow::DateTime => self.clock.map(|c| c[lit as usize]),
                QuickRow::ControllerSelect => self.controller.map(|c| c[lit as usize]),
                QuickRow::BluetoothSelect => self.bluetooth[0].map(|c| c[lit as usize]),
                QuickRow::BluetoothStatus => self.bluetooth[1].map(|c| c[lit as usize]),
                QuickRow::BatteryStatus => self.battery.map(|c| c[lit as usize]),
                _ => self.values[row.index()]
                    .and_then(|v| faces.values.get(v.index()))
                    .map(|v| v[lit as usize]),
            };
            let Some((tex, w, h)) = value else {
                continue;
            };
            if !lit || row.opens() {
                push(out, tex, right + pad - w as f32, y, w, h);
                continue;
            }
            let [(left_tex, lw, lh), (right_tex, rw, rh)] = faces.carets;
            let rx = right - rw as f32;
            push(out, right_tex, rx, y, rw, rh);
            let vx = rx - CARET_GAP + pad - w as f32;
            push(out, tex, vx, y, w, h);
            push(out, left_tex, vx + pad - CARET_GAP - lw as f32, y, lw, lh);
        }
        let [back, change, open] = faces.legend;
        let other = if self.row.opens() { open } else { change };
        for (tex, w, x) in centred_hints(&[back, other], LEGEND_GAP) {
            push(out, tex, x, LEGEND_Y, w, HINT_H);
        }
    }
}

fn row_top(row: QuickRow) -> f32 {
    if row.page().len() >= 8 {
        return QUICK_TOP + row_pitch(row) * row.position() as f32;
    }
    let top = (OUT_H as f32 - QUICK_PITCH * row.page().len() as f32) / 2.0;
    top + QUICK_PITCH * row.position() as f32
}

fn row_pitch(row: QuickRow) -> f32 {
    if row.page().len() >= 8 {
        QUICK_MAIN_PITCH
    } else {
        QUICK_PITCH
    }
}

fn push(out: &mut Vec<Draw>, tex: TexId, x: f32, y: f32, w: u32, h: u32) {
    out.push(Draw::Tex {
        x: x.round(),
        y: y.round(),
        w: w as f32,
        h: h as f32,
        tex,
        alpha: 1.0,
    });
}
