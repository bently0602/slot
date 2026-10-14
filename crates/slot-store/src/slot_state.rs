use std::path::{Path, PathBuf};

use crate::atomic::atomic_write;
use crate::gb_palette::GbPalette;
use crate::hdmi::HdmiScale;
use crate::platform::Platform;

pub const BRIGHTNESS_MAX: u8 = 9;
pub const BLUE_LIGHT_MAX: u8 = 9;
pub const VOLUME_MAX: u8 = 100;

pub const UTC_OFFSET_MIN: i16 = -720;
pub const UTC_OFFSET_MAX: i16 = 840;

pub const FF_SPEEDS: [u8; 4] = [2, 3, 4, 6];

pub const FF_SPEED_DEFAULT: u8 = 6;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SlotState {
    pub cart: Option<String>,
    pub cart_platform: Option<Platform>,
    pub brightness: u8,
    pub blue_light: u8,
    pub volume: u8,
    pub volume_hp: u8,
    pub muted: bool,
    pub muted_hp: bool,
    pub clock_set: bool,
    pub utc_offset_min: i16,
    pub rumble: bool,
    pub ff_speed: u8,
    pub ff_sound: bool,
    pub colour_correction: bool,
    pub shader_gba: Shader,
    pub shader_gb: Shader,
    pub shader_snes: Shader,
    pub shader_psx: Shader,
    pub eject_save: bool,
    pub turbo: bool,
    pub rewind: bool,
    pub gb_palettes: bool,
    pub coverflow: bool,
    pub gb_palette: GbPalette,
    pub hdmi_auto: bool,
    pub hdmi_1080p: bool,
    pub hdmi_scale: HdmiScale,
    pub hdmi_margin: u8,
    pub docked_lid: bool,
    pub audio_output: AudioOutput,
    pub sleep_shutdown: SleepShutdown,
    pub shutdown_charging: bool,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub enum SleepShutdown {
    Minute1,
    #[default]
    Minutes3,
    Minutes5,
    Minutes10,
    Minutes30,
    Never,
}

impl SleepShutdown {
    pub const ALL: [Self; 6] = [
        Self::Minute1,
        Self::Minutes3,
        Self::Minutes5,
        Self::Minutes10,
        Self::Minutes30,
        Self::Never,
    ];

    pub fn minutes(self) -> u8 {
        match self {
            Self::Minute1 => 1,
            Self::Minutes3 => 3,
            Self::Minutes5 => 5,
            Self::Minutes10 => 10,
            Self::Minutes30 => 30,
            Self::Never => 0,
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        let minutes: u8 = value.parse().ok()?;
        Self::ALL.into_iter().find(|s| s.minutes() == minutes)
    }

    pub fn step(self, right: bool) -> Self {
        let at = Self::ALL.iter().position(|s| *s == self).unwrap_or(1);
        Self::ALL[(at + if right { 1 } else { Self::ALL.len() - 1 }) % Self::ALL.len()]
    }

    pub fn duration(self) -> std::time::Duration {
        std::time::Duration::from_secs(u64::from(self.minutes()) * 60)
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub enum AudioOutput {
    #[default]
    Auto,
    Device,
    Hdmi,
}

impl AudioOutput {
    pub fn name(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Device => "device",
            Self::Hdmi => "hdmi",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        [Self::Auto, Self::Device, Self::Hdmi]
            .into_iter()
            .find(|v| v.name() == name)
    }

    pub fn step(self, right: bool) -> Self {
        let choices = [Self::Auto, Self::Device, Self::Hdmi];
        let at = self as usize;
        choices[(at + if right { 1 } else { 2 }) % choices.len()]
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Shader {
    Off,
    Lcd3x,
    Grid,
    Dot,
    Simpletex,
}

impl Shader {
    pub const ALL: [Shader; 5] = [
        Shader::Off,
        Shader::Lcd3x,
        Shader::Grid,
        Shader::Dot,
        Shader::Simpletex,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Shader::Off => "off",
            Shader::Lcd3x => "lcd3x",
            Shader::Grid => "grid",
            Shader::Dot => "dot",
            Shader::Simpletex => "simpletex",
        }
    }

    pub fn parse(name: &str) -> Option<Shader> {
        Self::ALL.into_iter().find(|s| s.name() == name)
    }

    pub fn step(self, choices: &[Shader], right: bool) -> Shader {
        let n = choices.len();
        let at = choices.iter().position(|&s| s == self).unwrap_or(0);
        choices[if right {
            (at + 1) % n
        } else {
            (at + n - 1) % n
        }]
    }
}

impl Default for SlotState {
    fn default() -> Self {
        SlotState {
            cart: None,
            cart_platform: None,
            brightness: 5,
            blue_light: 0,
            volume: 60,
            volume_hp: 60,
            muted: false,
            muted_hp: false,
            clock_set: false,
            utc_offset_min: 0,
            rumble: true,
            ff_speed: FF_SPEED_DEFAULT,
            ff_sound: false,
            colour_correction: false,
            shader_gba: Shader::Grid,
            shader_gb: Shader::Simpletex,
            shader_snes: Shader::Off,
            shader_psx: Shader::Off,
            eject_save: true,
            turbo: true,
            rewind: true,
            gb_palettes: false,
            coverflow: false,
            gb_palette: GbPalette::DEFAULT,
            hdmi_auto: true,
            hdmi_1080p: false,
            hdmi_scale: HdmiScale::Fit,
            hdmi_margin: 0,
            docked_lid: true,
            audio_output: AudioOutput::Auto,
            sleep_shutdown: SleepShutdown::default(),
            shutdown_charging: false,
        }
    }
}

fn state_path(root: &Path) -> PathBuf {
    root.join("Config").join("slot.state")
}

pub fn read_slot_state(root: &Path) -> SlotState {
    std::fs::read(state_path(root))
        .ok()
        .and_then(|b| String::from_utf8(b).ok())
        .and_then(|s| parse(&s))
        .unwrap_or_default()
}

pub fn write_slot_state(root: &Path, s: &SlotState) -> std::io::Result<()> {
    let mut text = format!(
        "cart={}\ncart_platform={}\nbrightness={}\nblue_light={}\nvolume={}\nvolume_hp={}\nmuted={}\nmuted_hp={}\nclock_set={}\nutc_offset_min={}\nrumble={}\nff_speed={}\nff_sound={}\ncolour_correction={}\nshader_gba={}\nshader_gb={}\neject_save={}\nturbo={}\nrewind={}\ngb_palettes={}\ncoverflow={}\ngb_palette={}\n",
        s.cart.as_deref().unwrap_or(""),
        s.cart_platform.map_or(String::new(), platform_key),
        s.brightness,
        s.blue_light,
        s.volume,
        s.volume_hp,
        s.muted as u8,
        s.muted_hp as u8,
        s.clock_set as u8,
        s.utc_offset_min,
        s.rumble as u8,
        s.ff_speed,
        s.ff_sound as u8,
        s.colour_correction as u8,
        s.shader_gba.name(),
        s.shader_gb.name(),
        s.eject_save as u8,
        s.turbo as u8,
        s.rewind as u8,
        s.gb_palettes as u8,
        s.coverflow as u8,
        s.gb_palette.core_name()
    );
    text.push_str(&format!(
        "shader_snes={}\nshader_psx={}\n",
        s.shader_snes.name(),
        s.shader_psx.name()
    ));
    text.push_str(&format!(
        "hdmi_auto={}\nhdmi_1080p={}\nhdmi_scale={}\nhdmi_margin={}\n",
        s.hdmi_auto as u8,
        s.hdmi_1080p as u8,
        s.hdmi_scale.name(),
        s.hdmi_margin
    ));
    text.push_str(&format!(
        "docked_lid={}\naudio_output={}\n",
        s.docked_lid as u8,
        s.audio_output.name()
    ));
    text.push_str(&format!(
        "sleep_shutdown={}\nshutdown_charging={}\n",
        s.sleep_shutdown.minutes(),
        s.shutdown_charging as u8
    ));
    atomic_write(&state_path(root), text.as_bytes())
}

fn parse(text: &str) -> Option<SlotState> {
    let mut cart = None;
    let mut cart_platform = None;
    let mut brightness = None;
    let mut blue_light = None;
    let mut volume = None;
    let mut volume_hp = None;
    let mut muted = None;
    let mut muted_hp = None;
    let mut clock_set = None;
    let mut utc_offset_min = None;
    let mut rumble = None;
    let mut ff_speed = None;
    let mut ff_sound = None;
    let mut colour_correction = None;
    let mut shader_gba = None;
    let mut shader_gb = None;
    let mut shader_snes = None;
    let mut shader_psx = None;
    let mut eject_save = None;
    let mut turbo = None;
    let mut rewind = None;
    let mut gb_palettes = None;
    let mut coverflow = None;
    let mut gb_palette = None;
    let mut hdmi_auto = None;
    let mut hdmi_1080p = None;
    let mut hdmi_scale = None;
    let mut hdmi_margin = None;
    let mut docked_lid = None;
    let mut audio_output = None;
    let mut sleep_shutdown = None;
    let mut shutdown_charging = None;
    for line in text.lines().filter(|l| !l.is_empty()) {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match key {
            "cart" => cart = Some(value.to_string()),
            "cart_platform" => cart_platform = platform_value(value),
            "brightness" => brightness = Some(level(value, BRIGHTNESS_MAX)?),
            "blue_light" => blue_light = Some(level(value, BLUE_LIGHT_MAX)?),
            "volume" => volume = Some(level(value, VOLUME_MAX)?),
            "volume_hp" => volume_hp = Some(level(value, VOLUME_MAX)?),
            "muted" => muted = Some(level(value, 1)? == 1),
            "muted_hp" => muted_hp = Some(level(value, 1)? == 1),
            "clock_set" => clock_set = Some(level(value, 1)? == 1),
            "utc_offset_min" => utc_offset_min = Some(offset(value)?),
            "rumble" => rumble = flag(value),
            "ff_speed" => ff_speed = ff_speed_value(value),
            "ff_sound" => ff_sound = flag(value),
            "colour_correction" => colour_correction = flag(value),
            "shader_gba" => shader_gba = Shader::parse(value),
            "shader_snes" => shader_snes = Shader::parse(value),
            "shader_psx" => shader_psx = Shader::parse(value),
            "eject_save" => eject_save = flag(value),
            "turbo" => turbo = flag(value),
            "rewind" => rewind = flag(value),
            "gb_palettes" => gb_palettes = flag(value),
            "coverflow" => coverflow = flag(value),
            "hdmi_auto" => hdmi_auto = flag(value),
            "docked_lid" => docked_lid = flag(value),
            "audio_output" => audio_output = AudioOutput::parse(value),
            "sleep_shutdown" => sleep_shutdown = SleepShutdown::parse(value),
            "shutdown_charging" => shutdown_charging = flag(value),
            "hdmi_1080p" => hdmi_1080p = flag(value),
            "hdmi_scale" => hdmi_scale = HdmiScale::parse(value),
            "hdmi_margin" => hdmi_margin = level(value, 10).filter(|n| [0, 5, 10].contains(n)),
            "gb_palette" => gb_palette = GbPalette::parse(value),
            "shader_gb" => shader_gb = Shader::parse(value),
            _ => {}
        }
    }
    let cart = cart?;
    let fallback = SlotState::default();
    Some(SlotState {
        cart: (!cart.is_empty()).then_some(cart),
        cart_platform,
        brightness: brightness?,
        blue_light: blue_light?,
        volume: volume?,
        volume_hp: volume_hp.or(volume)?,
        muted: muted?,
        muted_hp: muted_hp.or(muted)?,
        clock_set: clock_set?,
        utc_offset_min: utc_offset_min?,
        rumble: rumble.unwrap_or(fallback.rumble),
        ff_speed: ff_speed.unwrap_or(fallback.ff_speed),
        ff_sound: ff_sound.unwrap_or(fallback.ff_sound),
        colour_correction: colour_correction.unwrap_or(fallback.colour_correction),
        shader_gba: shader_gba.unwrap_or(fallback.shader_gba),
        shader_gb: shader_gb.unwrap_or(fallback.shader_gb),
        shader_snes: shader_snes.unwrap_or(fallback.shader_snes),
        shader_psx: shader_psx.unwrap_or(fallback.shader_psx),
        eject_save: eject_save.unwrap_or(fallback.eject_save),
        turbo: turbo.unwrap_or(fallback.turbo),
        rewind: rewind.unwrap_or(fallback.rewind),
        gb_palettes: gb_palettes.unwrap_or(fallback.gb_palettes),
        coverflow: coverflow.unwrap_or(fallback.coverflow),
        gb_palette: gb_palette.unwrap_or(fallback.gb_palette),
        hdmi_auto: hdmi_auto.unwrap_or(fallback.hdmi_auto),
        hdmi_1080p: hdmi_1080p.unwrap_or(fallback.hdmi_1080p),
        hdmi_scale: hdmi_scale.unwrap_or(fallback.hdmi_scale),
        hdmi_margin: hdmi_margin.unwrap_or(fallback.hdmi_margin),
        docked_lid: docked_lid.unwrap_or(fallback.docked_lid),
        audio_output: audio_output.unwrap_or(fallback.audio_output),
        sleep_shutdown: sleep_shutdown.unwrap_or(fallback.sleep_shutdown),
        shutdown_charging: shutdown_charging.unwrap_or(fallback.shutdown_charging),
    })
}

fn platform_key(platform: Platform) -> String {
    platform.dir_name().to_ascii_lowercase()
}

fn platform_value(value: &str) -> Option<Platform> {
    Platform::ALL
        .into_iter()
        .find(|p| value.eq_ignore_ascii_case(p.dir_name()))
}

fn offset(value: &str) -> Option<i16> {
    value
        .parse()
        .ok()
        .filter(|n| (UTC_OFFSET_MIN..=UTC_OFFSET_MAX).contains(n))
}

fn ff_speed_value(value: &str) -> Option<u8> {
    value.parse().ok().filter(|n| FF_SPEEDS.contains(n))
}

fn level(value: &str, max: u8) -> Option<u8> {
    value.parse().ok().filter(|n| *n <= max)
}

fn flag(value: &str) -> Option<bool> {
    level(value, 1).map(|n| n == 1)
}
