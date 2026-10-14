use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use slot_gfx::{OUT_H, OUT_W};
use slot_input::{
    Action, Btn, ControllerConfig, ControllerInfo, ControllerProfile, ControllerStatus,
    MUTE_CHORD_MS, STICK_DEADZONES,
};
use slot_power::{Battery, Charge, LedState, LidPolicy, Power};
use slot_retro::LinkChannel;
use slot_store::{
    format_stamp, read_slot_state, scan, write_slot_state, Cart, Core, GbPalette, Platform, Shader,
    SlotState, StateEntry, StateRing, Theme, BLUE_LIGHT_MAX, BRIGHTNESS_MAX, FF_SPEEDS, RING_MAX,
    SAVE_SLOTS, VOLUME_MAX,
};
use slot_ui::{
    board_from, board_zoom, draw_backdrop, draw_empty_slot, draw_footer, draw_slot_name,
    draw_sticker, ease, grown, lid_at, lid_from, lift_of, on_board, shelf_cart_at, ClockPicker,
    Draw, FfState, GbShell, Hud, HudKind, Icon, LinkBadge, Millis, Placed, Polaroids, PowerChoice,
    PsxTextures, QuickMenu, QuickMenuFaces, QuickRow, QuickValue, Refusal, Shelf, SlotChrome,
    TexId, Toast, BOARD_W, BOARD_X, CART_W, CHIP_H, CHIP_U, CHIP_V, CHIP_W, HINT_EDGE, HINT_H,
    HOP_LIFT, SHADOW_H, SHADOW_W, SOCKET_H, SOCKET_U, SOCKET_V, SOCKET_W, TURN_PAD,
};

use crate::audio::Sfx;
use crate::core_picker::{Chip, CorePicker, Outcome, Press};
use crate::link_kind::{link_carried, link_kind, serial_option, LinkKind};
use crate::link_radio::{radio_jobs, LinkRole, RadioJob, RadioJobs};
use crate::link_screen::LinkSprites;
use crate::link_start::{link_port, LinkFail, LinkProgress, LinkStarter, LinkStep};
use crate::persist::{self, Snapshot};
use crate::run_ahead::{self, RUN_AHEAD_MAX};
use crate::video_mode::{self, VideoMode};

pub const INSERT_S: f32 = 0.73;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Playback {
    Rewind,
    FastForward,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ShelfKind {
    Console(Platform),
    Music,
}
const INSERT_HOLD_S: f32 = 0.28;

pub const SEATED_AT: f32 = INSERT_S - INSERT_HOLD_S;

pub const EJECT_S: f32 = SEATED_AT;

pub const PSX_SEATED_AT: f32 = 1.5;

fn seated_at(platform: Platform) -> f32 {
    if platform == Platform::Psx {
        PSX_SEATED_AT
    } else {
        SEATED_AT
    }
}

const EJECT_HOLD_S: f32 = 0.35;

const POWER_ON_S: f32 = 0.22;

const POWER_OFF_S: f32 = 0.16;

const VOLUME_STEP: u8 = 5;

const AUTOSAVE_MS: Millis = 60_000;

const BATTERY_CRITICAL: u8 = 5;
const BATTERY_SAVE: u8 = 10;
const BATTERY_RECOVERED: u8 = 15;
const BATTERY_RETRY_MS: Millis = 30_000;

const BATTERY_POLL_MS: Millis = 10_000;

const CHARGE_POLL_MS: Millis = 1_000;
const HEADPHONES_POLL_MS: Millis = 500;

const BATTERY_LOW: u8 = 20;

pub const UNDO_GRACE_MS: Millis = 30_000;

pub const LINK_LOST_MS: Millis = 2000;

const PLAY_HOLD_MS: Millis = 500;

const SHUTDOWN_SHOW_MS: Millis = 250;

const POWER_MENU_PITCH: f32 = 44.0;
const POWER_MENU_BAR_INSET: f32 = 4.0;
const CORE_PICKER_RECEDE: f32 = 0.26;

const SLOT_NAME_IN_MS: Millis = 200;
const SLOT_NAME_HOLD_MS: Millis = 1200;
const SLOT_NAME_OUT_MS: Millis = 800;
const SLOT_NAME_ALPHA: f32 = 0.4;
const CORE_PICKER_DIM: f32 = 0.614;
const CORE_LEGEND_Y: f32 = 386.0;
const LINK_TEXT_Y: f32 = 44.0;
const LINK_LEGEND_Y: f32 = 422.0;
const LINK_LEGEND_GAP: f32 = 40.0;
const LID_SHADOW_W: f32 = 168.0;
const LID_SHADOW_H: f32 = 18.0;
const LID_SHADOW_DROP: f32 = 29.0;
const LID_SHADOW_ALPHA: f32 = 0.8;
const FACES_WAIT_MS: Millis = 1500;

const ALERT_HOLD: f32 = 0.45;
const ALERT_GONE: f32 = 0.9;

const CLOCK_FLOOR: i64 = 1_577_836_800;

pub enum PendingUndo {
    SaveSlot {
        slot: usize,
        prior: Option<Vec<u8>>,
        prior_modified: Option<SystemTime>,
    },
    Save {
        stamp: String,
        evicted: Option<(String, Vec<u8>, Vec<u8>)>,
    },
    Load {
        prior: Vec<u8>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StateSlotMode {
    Save,
    Load,
}

struct LinkSession {
    client_id: u16,
    lost_at: Option<Millis>,
}

struct LinkStarting {
    starter: LinkStarter,
    client_id: u16,
}

struct Reload {
    stem: String,
    role: LinkRow,
    cancelled: bool,
    from: LinkKind,
    from_serial: &'static str,
    fallback: bool,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum GameMenu {
    Pick(LinkRow),
    Working {
        role: LinkRow,
        step: LinkStep,
        since: Millis,
    },
    Linked {
        role: LinkRow,
        worked: Millis,
        since: Millis,
        opened: bool,
    },
    Failed {
        role: LinkRow,
        fail: LinkFail,
        worked: Millis,
        since: Millis,
    },
    Unplug {
        role: LinkRow,
        since: Millis,
    },
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum LinkRow {
    Host,
    Join,
}

impl LinkRow {
    pub const ALL: [LinkRow; 2] = [LinkRow::Host, LinkRow::Join];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn text(self) -> &'static str {
        match self {
            LinkRow::Host => "Host",
            LinkRow::Join => "Join",
        }
    }

    pub fn role(self) -> LinkRole {
        match self {
            LinkRow::Host => LinkRole::Host,
            LinkRow::Join => LinkRole::Join,
        }
    }

    pub fn client_id(self) -> u16 {
        match self {
            LinkRow::Host => 0,
            LinkRow::Join => 1,
        }
    }

    pub fn other(self) -> LinkRow {
        match self {
            LinkRow::Host => LinkRow::Join,
            LinkRow::Join => LinkRow::Host,
        }
    }

    pub fn from_client_id(id: u16) -> LinkRow {
        if id == 0 {
            LinkRow::Host
        } else {
            LinkRow::Join
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum LinkLegend {
    Cancel,
    Mode,
    Swap,
    Link,
    Ok,
    Back,
    EndLink,
}

impl LinkLegend {
    pub const ALL: [LinkLegend; 7] = [
        LinkLegend::Cancel,
        LinkLegend::Mode,
        LinkLegend::Swap,
        LinkLegend::Link,
        LinkLegend::Ok,
        LinkLegend::Back,
        LinkLegend::EndLink,
    ];

    pub fn index(self) -> usize {
        self as usize
    }
}

pub const LINKED_HOLD_MS: Millis = 1000;

pub const UNPLUG_HOLD_MS: Millis = 420;

#[derive(Debug)]
pub enum Phase {
    SetClock {
        picker: ClockPicker,
        seed: i64,
        from_menu: bool,
    },
    Shelf,
    QuickMenu {
        row: QuickRow,
    },
    /// Each controller choosing the player it plays as.
    Players,
    Inserting {
        cart: String,
        t: f32,
        core_ready: bool,
        resumed: bool,
        clean: bool,
    },
    Playing {
        cart: String,
    },
    Ejecting {
        cart: String,
        t: f32,
    },
    Polaroids {
        cart: String,
    },
    About,
    Doze {
        cart: Option<String>,
    },
}

pub struct App {
    pub bluetooth: crate::bluetooth::Bluetooth,
    bluetooth_audio: bool,
    quick_bluetooth_faces: [Option<[(TexId, u32, u32); 2]>; 2],
    phase: Phase,
    shelves: Vec<(Platform, Shelf)>,
    shelf_at: usize,
    pub shelf_kind: ShelfKind,
    pub music: crate::music_ui::Browser,
    play_held: Option<Millis>,
    refusal: Option<Refusal>,
    refused_from: Option<f32>,
    alert_face: Option<TexId>,
    psx_textures: Option<PsxTextures>,
    shutdown_faces: Vec<(TexId, u32, u32)>,
    power_menu: Option<usize>,
    power_menu_faces: Vec<(TexId, u32, u32)>,
    core_picker: Option<CorePicker>,
    core_board_face: Option<TexId>,
    core_lid_face: Option<TexId>,
    core_faces_stem: Option<String>,
    core_socket_faces: Vec<TexId>,
    core_chip_faces: Vec<TexId>,
    core_blank_chip_face: Option<TexId>,
    core_chip_shadow_face: Option<TexId>,
    core_legend_faces: Vec<(TexId, u32)>,
    game_menu: Option<GameMenu>,
    disc_menu: Option<usize>,
    state_menu: Option<usize>,
    game_reset_requested: bool,
    playback: Option<Playback>,
    players_open: bool,
    state_slot_menu: Option<(StateSlotMode, usize)>,
    state_slot_metadata: [Option<std::fs::Metadata>; SAVE_SLOTS],
    state_slot_faces: Vec<(TexId, u32, u32)>,
    state_slot_undo_face: Option<(TexId, u32, u32)>,
    state_faces: Vec<(TexId, u32, u32)>,
    run_ahead_faces: Vec<(TexId, u32, u32)>,
    run_ahead: u8,
    discs: (usize, usize, bool),
    disc_change: Option<usize>,
    disc_faces: Vec<(TexId, u32, u32)>,
    disc_title: Option<(TexId, u32, u32)>,
    link_sprites: Option<LinkSprites>,
    link_hardware: LinkKind,
    last_role: LinkRow,
    link_choices: HashMap<String, LinkKind>,
    link_loaded: Option<&'static str>,
    link_reload: Option<(String, &'static str)>,
    reload: Option<Reload>,
    link_menu_faces: Vec<(TexId, u32, u32)>,
    link_linked_face: Option<(TexId, u32, u32)>,
    link_step_faces: Vec<(TexId, u32, u32)>,
    link_fail_faces: Vec<(TexId, u32, u32)>,
    link_legend_faces: Vec<(TexId, u32)>,
    starting: Option<LinkStarting>,
    link_transport: Option<(u16, Box<dyn LinkChannel>)>,
    restarting: bool,
    device_info_requested: bool,
    act_at: Millis,
    root: Option<PathBuf>,
    state: SlotState,
    controller_config: ControllerConfig,
    controller_status: ControllerStatus,
    last_controller: Option<ControllerInfo>,
    vol_before: Vec<(u8, bool, Millis)>,
    snapshot: Option<Box<dyn Snapshot>>,
    core: Core,
    colour_pending: Option<bool>,
    palette_live: bool,
    palette_pending: Option<GbPalette>,
    link_player: Option<u8>,
    platform: Platform,
    named_core: bool,
    video_mode: VideoMode,
    link: Option<LinkSession>,
    sfx: Option<Sfx>,
    polaroids: Option<Polaroids>,
    pending: Option<(PendingUndo, Millis)>,
    legend_faces: Vec<TexId>,
    undo_face: Option<TexId>,
    clock_faces: Option<(TexId, TexId)>,
    quick_menu_faces: Option<QuickMenuFaces>,
    quick_clock_faces: Option<[(TexId, u32, u32); 2]>,
    quick_controller_faces: Option<[(TexId, u32, u32); 2]>,
    sticker_face: Option<TexId>,
    players_face: Option<TexId>,
    wallpaper: Option<TexId>,
    battery_percent: slot_ui::Printed,
    bolt: Option<TexId>,
    shelf_platform: slot_ui::Printed,
    shelf_switch_hints: [slot_ui::Printed; 2],
    name_pending: bool,
    slot_letter: Option<char>,
    shelf_named: Option<Millis>,
    shelf_clock: slot_ui::Printed,
    hud: Hud,
    screen: f32,
    lift: f32,
    game_ready: bool,
    clock: f64,
    power: Option<Power>,
    screen_off: bool,
    dozed_at: Millis,
    docked: bool,
    hdmi_audio: bool,
    lid_closed: bool,
    lid_sleeping: bool,
    sleep_inserting: Option<Phase>,
    woke_on_press: bool,
    autosave_at: Millis,
    pending_save: Option<std::thread::JoinHandle<()>>,
    battery_at: Millis,
    charge_at: Millis,
    headphones: bool,
    headphones_at: Millis,
    battery: Option<Battery>,
    battery_warned: bool,
    battery_saved: Option<(Platform, Core, String)>,
    battery_retry_at: Millis,
    quick_battery_faces: Option<[(TexId, u32, u32); 2]>,
    last_led: Option<LedState>,
    powering_off: bool,
    radio: Box<dyn RadioJobs>,
    state_writer: crate::state_writer::StateWriter,
}

const FACE_AHEAD: i32 = 8;
const FACE_KEEP: i32 = 12;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FaceWant {
    pub shelf: usize,
    pub index: usize,
    pub on_screen: bool,
}

fn shelves_of(carts: Vec<Cart>) -> Vec<(Platform, Shelf)> {
    let mut rows: Vec<(Platform, Vec<Cart>)> =
        Platform::ALL.iter().map(|p| (*p, Vec::new())).collect();
    for cart in carts {
        if let Some((_, row)) = rows.iter_mut().find(|(p, _)| *p == cart.platform) {
            row.push(cart);
        }
    }
    rows.into_iter()
        .map(|(platform, carts)| (platform, Shelf::new(carts)))
        .collect()
}

impl App {
    pub fn new(carts: Vec<Cart>) -> Self {
        let shelves = shelves_of(carts);
        let shelf_at = shelves
            .iter()
            .position(|(_, s)| !s.carts.is_empty())
            .unwrap_or(0);
        App {
            bluetooth: crate::bluetooth::Bluetooth::default(),
            bluetooth_audio: false,
            quick_bluetooth_faces: [None; 2],
            radio: radio_jobs(),
            state_writer: crate::state_writer::StateWriter::spawn(),
            phase: Phase::Shelf,
            shelves,
            shelf_at,
            shelf_kind: ShelfKind::Console(Platform::ALL[shelf_at]),
            music: crate::music_ui::Browser::default(),
            play_held: None,
            refusal: None,
            refused_from: None,
            alert_face: None,
            psx_textures: None,
            shutdown_faces: Vec::new(),
            power_menu: None,
            power_menu_faces: Vec::new(),
            core_picker: None,
            core_board_face: None,
            core_lid_face: None,
            core_faces_stem: None,
            core_socket_faces: Vec::new(),
            core_chip_faces: Vec::new(),
            core_blank_chip_face: None,
            core_chip_shadow_face: None,
            core_legend_faces: Vec::new(),
            game_menu: None,
            disc_menu: None,
            state_menu: None,
            game_reset_requested: false,
            playback: None,
            players_open: false,
            state_slot_menu: None,
            state_slot_metadata: std::array::from_fn(|_| None),
            state_slot_faces: Vec::new(),
            state_slot_undo_face: None,
            state_faces: Vec::new(),
            run_ahead_faces: Vec::new(),
            run_ahead: 0,
            discs: (0, 0, false),
            disc_change: None,
            disc_faces: Vec::new(),
            disc_title: None,
            link_sprites: None,
            link_hardware: LinkKind::Cable,
            last_role: LinkRow::Host,
            link_choices: HashMap::new(),
            link_loaded: None,
            link_reload: None,
            reload: None,
            link_menu_faces: Vec::new(),
            link_linked_face: None,
            link_step_faces: Vec::new(),
            link_fail_faces: Vec::new(),
            link_legend_faces: Vec::new(),
            starting: None,
            link_transport: None,
            restarting: false,
            device_info_requested: false,
            act_at: 0,
            root: None,
            state: SlotState::default(),
            controller_config: ControllerConfig::default(),
            controller_status: ControllerStatus::default(),
            last_controller: None,
            vol_before: Vec::new(),
            snapshot: None,
            core: Core::default(),
            colour_pending: None,
            palette_live: false,
            palette_pending: None,
            link_player: None,
            platform: Platform::default(),
            named_core: false,
            video_mode: VideoMode::default(),
            link: None,
            sfx: None,
            polaroids: None,
            pending: None,
            legend_faces: Vec::new(),
            undo_face: None,
            clock_faces: None,
            quick_menu_faces: None,
            quick_clock_faces: None,
            quick_controller_faces: None,
            sticker_face: None,
            players_face: None,
            wallpaper: None,
            battery_percent: slot_ui::Printed::default(),
            bolt: None,
            shelf_platform: slot_ui::Printed::default(),
            shelf_switch_hints: [slot_ui::Printed::default(); 2],
            name_pending: false,
            slot_letter: None,
            shelf_named: None,
            shelf_clock: slot_ui::Printed::default(),
            hud: Hud::new(),
            screen: 0.0,
            lift: 1.0,
            game_ready: false,
            clock: 0.0,
            power: None,
            screen_off: false,
            dozed_at: 0,
            docked: false,
            hdmi_audio: false,
            lid_closed: false,
            lid_sleeping: false,
            sleep_inserting: None,
            woke_on_press: false,
            autosave_at: AUTOSAVE_MS,
            pending_save: None,
            battery_at: BATTERY_POLL_MS,
            charge_at: CHARGE_POLL_MS,
            headphones: false,
            headphones_at: 0,
            battery: None,
            battery_warned: false,
            battery_saved: None,
            battery_retry_at: 0,
            quick_battery_faces: None,
            last_led: None,
            powering_off: false,
        }
    }

    pub fn boot(root: &Path) -> Self {
        crate::root::ensure(root);
        slot_ui::set_theme(Theme::read(root));
        crate::boot_time::mark("root and theme");
        let carts = scan(root).unwrap_or_default();
        crate::boot_time::mark("scan");
        let mut app = App::new(carts);
        app.root = Some(root.to_path_buf());
        app.bluetooth = crate::bluetooth::Bluetooth::boot(root);
        app.state = read_slot_state(root);
        app.controller_config = slot_store::read_controller_config(root);
        app.sync_shelf_settings();
        let args: Vec<_> = std::env::args_os().collect();
        if let Some((cart, platform)) = crate::system_app::reset_target(&args) {
            app.start_game_clean(&cart, platform);
        } else if args.iter().any(|arg| arg == "--return-device-info") {
            // Skip automatic cartridge resume after the system app exits.
            app.return_to_device_info_menu();
        } else if app.state.clock_set {
            app.start();
        } else {
            app.phase = clock_screen(system_secs(), 0, false);
        }
        app
    }

    /// Start the selected cartridge without restoring a potentially broken state.
    pub fn start_game_clean(&mut self, cart: &str, platform: Platform) {
        self.phase = Phase::Shelf;
        let Some((at, i)) = self.seat_of(cart, Some(platform)) else {
            return;
        };
        self.shelf_at = at;
        self.shelf_kind = ShelfKind::Console(platform);
        self.shelf_mut().select(i);
        self.insert(true);
    }

    fn start(&mut self) {
        let seated = if self.single_cart() {
            Some((self.shelf_at, 0))
        } else {
            let stem = self.state.cart.clone();
            let platform = self.state.cart_platform;
            stem.and_then(|stem| self.seat_of(&stem, platform))
        };
        self.phase = Phase::Shelf;
        match seated {
            Some((at, i)) => {
                self.shelf_at = at;
                self.shelf_kind = ShelfKind::Console(self.shelves[at].0);
                self.shelf_mut().select(i);
                self.insert(false);
                let insert_s = seated_at(self.shelves[self.shelf_at].0) + INSERT_HOLD_S;
                if let Phase::Inserting { resumed, t, .. } = &mut self.phase {
                    *resumed = true;
                    *t = insert_s;
                }
            }
            None => {
                self.state.cart = None;
                self.state.cart_platform = None;
                self.name_pending = true;
            }
        }
    }

    fn shelf(&self) -> &Shelf {
        &self.shelves[self.shelf_at].1
    }

    fn shelf_mut(&mut self) -> &mut Shelf {
        &mut self.shelves[self.shelf_at].1
    }

    fn sync_shelf_settings(&mut self) {
        for (_, shelf) in &mut self.shelves {
            shelf.set_coverflow(self.state.coverflow);
        }
    }

    fn switch_shelf(&mut self, by: i32) {
        let mut choices: Vec<ShelfKind> = self
            .shelves
            .iter()
            .filter(|(_, s)| !s.carts.is_empty())
            .map(|(p, _)| ShelfKind::Console(*p))
            .collect();
        choices.push(ShelfKind::Music);
        let at = choices
            .iter()
            .position(|k| *k == self.shelf_kind)
            .unwrap_or(0);
        let kind = choices[(at as i32 + by).rem_euclid(choices.len() as i32) as usize];
        self.shelf_mut().release_hold();
        self.music.release();
        self.play_held = None;
        self.shelf_kind = kind;
        if let ShelfKind::Console(platform) = kind {
            self.shelf_at = self
                .shelves
                .iter()
                .position(|(p, _)| *p == platform)
                .unwrap_or(0);
        }
        self.shelf_platform = slot_ui::Printed::default();
        self.slot_letter = None;
        self.shelf_named = None;
        self.name_pending = true;
    }

    fn jump_letter(&mut self, dir: i32) {
        let before = self.shelf().index;
        match dir > 0 {
            true => self.shelf_mut().jump_next_letter(),
            false => self.shelf_mut().jump_prev_letter(),
        }
        if self.shelf().index == before {
            return;
        }
        let Some(letter) = self.selected_stem().map(slot_store::initial) else {
            return;
        };
        if self.slot_letter != Some(letter) {
            self.shelf_platform = slot_ui::Printed::default();
        }
        self.slot_letter = Some(letter);
        self.shelf_named = None;
        self.name_pending = true;
    }

    fn slot_name_alpha(&self) -> f32 {
        if self.slot_letter.is_none() {
            return SLOT_NAME_ALPHA;
        }
        let Some(at) = self.shelf_named else {
            return 0.0;
        };
        let t = self.now().saturating_sub(at);
        let level = if t < SLOT_NAME_IN_MS {
            t as f32 / SLOT_NAME_IN_MS as f32
        } else if t < SLOT_NAME_IN_MS + SLOT_NAME_HOLD_MS {
            1.0
        } else {
            let out = t - SLOT_NAME_IN_MS - SLOT_NAME_HOLD_MS;
            1.0 - (out as f32 / SLOT_NAME_OUT_MS as f32).min(1.0)
        };
        SLOT_NAME_ALPHA * ease(level)
    }

    fn next_shelf(&self, by: i32) -> Option<usize> {
        let n = self.shelves.len() as i32;
        (1..n)
            .map(|step| (self.shelf_at as i32 + by * step).rem_euclid(n) as usize)
            .find(|at| !self.shelves[*at].1.carts.is_empty())
    }

    fn seat_of(&self, stem: &str, platform: Option<Platform>) -> Option<(usize, usize)> {
        self.shelves
            .iter()
            .enumerate()
            .filter(|(_, (p, _))| platform.is_none_or(|want| *p == want))
            .find_map(|(at, (_, shelf))| {
                shelf
                    .carts
                    .iter()
                    .position(|c| c.stem == stem)
                    .map(|i| (at, i))
            })
    }

    pub fn confirm_clock(&mut self) {
        let Phase::SetClock {
            picker,
            seed,
            from_menu,
        } = &self.phase
        else {
            return;
        };
        let (moved, offset, from_menu) = (picker.secs() - *seed, picker.offset_min(), *from_menu);
        let utc = self.utc_secs() + moved;
        if let Some(power) = &mut self.power {
            power.set_clock(utc);
        }
        self.state.utc_offset_min = offset as i16;
        self.state.clock_set = true;
        self.persist();
        if from_menu {
            self.phase = Phase::QuickMenu {
                row: QuickRow::DateTime,
            };
        } else {
            self.start();
        }
    }

    pub fn root(&self) -> Option<&Path> {
        self.root.as_deref()
    }

    pub fn wall_secs(&self) -> i64 {
        self.utc_secs() + i64::from(self.state.utc_offset_min) * 60
    }

    fn utc_secs(&self) -> i64 {
        self.power.as_ref().map_or_else(system_secs, |p| p.now())
    }

    pub fn picker(&self) -> Option<&ClockPicker> {
        match &self.phase {
            Phase::SetClock { picker, .. } => Some(picker),
            _ => None,
        }
    }

    pub fn quick_menu(&self) -> Option<QuickRow> {
        match self.phase {
            Phase::QuickMenu { row } => Some(row),
            _ => None,
        }
    }

    pub fn prepare_music_display(&mut self) -> bool {
        if self.state.cart.is_some() || self.music.settings.resume.is_none() {
            return false;
        }
        // A sole game normally autostarts; a music display restart must stay on Music.
        self.phase = Phase::Shelf;
        self.shelf_kind = ShelfKind::Music;
        true
    }

    pub fn device_info_requested(&self) -> bool {
        self.device_info_requested
    }

    pub fn take_device_info_request(&mut self) -> Option<PathBuf> {
        if !std::mem::take(&mut self.device_info_requested) {
            return None;
        }
        self.root.clone()
    }

    pub fn return_to_device_info_menu(&mut self) {
        self.phase = Phase::QuickMenu {
            row: QuickRow::DeviceInfo,
        };
    }

    pub fn quick_value(&self, row: QuickRow) -> Option<QuickValue> {
        match row {
            QuickRow::Battery | QuickRow::BatteryStatus => None,
            QuickRow::SleepShutdown => Some(match self.state.sleep_shutdown {
                slot_store::SleepShutdown::Minute1 => QuickValue::Minute1,
                slot_store::SleepShutdown::Minutes3 => QuickValue::Minutes3,
                slot_store::SleepShutdown::Minutes5 => QuickValue::Minutes5,
                slot_store::SleepShutdown::Minutes10 => QuickValue::Minutes10,
                slot_store::SleepShutdown::Minutes30 => QuickValue::Minutes30,
                slot_store::SleepShutdown::Never => QuickValue::Never,
            }),
            QuickRow::ShutdownCharging => Some(QuickValue::flag(self.state.shutdown_charging)),
            QuickRow::Bluetooth
            | QuickRow::BluetoothScan
            | QuickRow::BluetoothSelect
            | QuickRow::BluetoothConnect
            | QuickRow::BluetoothDisconnect
            | QuickRow::BluetoothForget
            | QuickRow::BluetoothOff
            | QuickRow::BluetoothStatus => None,
            QuickRow::MusicPlay => Some(if self.music.snapshot.playing {
                QuickValue::Playing
            } else {
                QuickValue::Paused
            }),
            QuickRow::MusicShuffle => Some(QuickValue::flag(self.music.settings.shuffle)),
            QuickRow::MusicLid => Some(QuickValue::flag(self.music.settings.keep_playing_closed)),
            QuickRow::MusicRepeat => Some(match self.music.settings.repeat {
                slot_store::Repeat::Off => QuickValue::Off,
                slot_store::Repeat::Album => QuickValue::Album,
                slot_store::Repeat::Track => QuickValue::Track,
            }),
            QuickRow::MusicStyle => Some(match self.music.settings.style {
                slot_store::VisualizerStyle::Bars => QuickValue::Bars,
                slot_store::VisualizerStyle::Waveform => QuickValue::Waveform,
            }),
            QuickRow::Music
            | QuickRow::MusicPrevious
            | QuickRow::MusicNext
            | QuickRow::MusicRescan => None,
            QuickRow::ControllerSwapAb => self
                .controller_target()
                .map(|p| QuickValue::flag(p.swap_ab)),
            QuickRow::ControllerSwapXy => self
                .controller_target()
                .map(|p| QuickValue::flag(p.swap_xy)),
            QuickRow::ControllerDeadzone => self.controller_target().map(|p| match p.deadzone {
                10 => QuickValue::Percent10,
                15 => QuickValue::Percent15,
                20 => QuickValue::Percent20,
                30 => QuickValue::Percent30,
                40 => QuickValue::Percent40,
                _ => QuickValue::Percent25,
            }),
            QuickRow::ControllerPause => {
                Some(QuickValue::flag(self.controller_config.pause_disconnect))
            }
            QuickRow::Controllers
            | QuickRow::ControllerPlayers
            | QuickRow::ControllerSelect
            | QuickRow::ControllerReset => None,
            QuickRow::FastForward => QuickValue::speed(self.state.ff_speed),
            QuickRow::FastForwardSound => Some(QuickValue::flag(self.state.ff_sound)),
            QuickRow::ColourCorrection => Some(QuickValue::flag(self.state.colour_correction)),
            QuickRow::GbPalettes => Some(QuickValue::flag(self.state.gb_palettes)),
            QuickRow::CoverFlow => Some(QuickValue::flag(self.state.coverflow)),
            QuickRow::DockedLid => Some(QuickValue::flag(self.state.docked_lid)),
            QuickRow::AudioOutput => Some(match self.state.audio_output {
                slot_store::AudioOutput::Auto => QuickValue::Auto,
                slot_store::AudioOutput::Device => QuickValue::Device,
                slot_store::AudioOutput::Hdmi => QuickValue::Hdmi,
            }),
            QuickRow::HdmiOutput => Some(if self.state.hdmi_auto {
                QuickValue::Auto
            } else {
                QuickValue::Internal
            }),
            QuickRow::HdmiResolution => Some(if self.state.hdmi_1080p {
                QuickValue::P1080
            } else {
                QuickValue::P720
            }),
            QuickRow::HdmiScale => Some(match self.state.hdmi_scale {
                slot_store::HdmiScale::Fit => QuickValue::Fit,
                slot_store::HdmiScale::Integer => QuickValue::Integer,
                slot_store::HdmiScale::Stretch => QuickValue::Stretch,
            }),
            QuickRow::HdmiMargin => Some(match self.state.hdmi_margin {
                5 => QuickValue::Percent5,
                10 => QuickValue::Percent10,
                _ => QuickValue::Off,
            }),
            QuickRow::GbaShader => Some(shader_value(self.state.shader_gba)),
            QuickRow::GbShader => Some(shader_value(self.state.shader_gb)),
            QuickRow::SnesShader => Some(shader_value(self.state.shader_snes)),
            QuickRow::PsxShader => Some(shader_value(self.state.shader_psx)),
            QuickRow::Rumble => Some(QuickValue::flag(self.state.rumble)),
            QuickRow::EjectSave => Some(QuickValue::flag(self.state.eject_save)),
            QuickRow::Turbo => Some(QuickValue::flag(self.state.turbo)),
            QuickRow::Rewind => Some(QuickValue::flag(self.state.rewind)),
            QuickRow::DateTime
            | QuickRow::ScreenOff
            | QuickRow::DeviceInfo
            | QuickRow::About
            | QuickRow::Screen
            | QuickRow::Game
            | QuickRow::VideoOut => None,
        }
    }

    pub fn set_quick_menu_faces(&mut self, faces: QuickMenuFaces) {
        self.quick_menu_faces = Some(faces);
    }

    pub fn set_quick_clock_faces(&mut self, dim: (TexId, u32, u32), lit: (TexId, u32, u32)) {
        self.quick_clock_faces = Some([dim, lit]);
    }

    pub fn controller_config(&self) -> &ControllerConfig {
        &self.controller_config
    }

    pub fn set_controller_status(&mut self, status: ControllerStatus) {
        if let Some(info) = status
            .connected
            .iter()
            .find(|p| Some(&p.id) == status.active.as_ref())
        {
            self.last_controller = Some(info.clone());
        }
        self.controller_status = status;
    }

    fn controller_target(&self) -> Option<ControllerProfile> {
        let id = self
            .controller_config
            .preferred
            .as_ref()
            .or(self.controller_status.active.as_ref())
            .or(self.last_controller.as_ref().map(|p| &p.id))?;
        let mut profile = self.controller_config.profile(id);
        profile.id = id.clone();
        if let Some(info) = self
            .controller_status
            .connected
            .iter()
            .find(|p| p.id == *id)
            .or(self.last_controller.as_ref().filter(|p| p.id == *id))
        {
            profile.name = info.name.clone();
        }
        Some(profile)
    }

    pub fn controller_text(&self) -> String {
        let name = self.controller_target().map(|p| p.name).unwrap_or_default();
        let name: String = name.chars().take(18).collect();
        if self.controller_config.preferred.is_none() {
            if self.controller_status.active.is_none() {
                "Auto (no pad)".into()
            } else {
                format!("Auto: {name}")
            }
        } else if self.controller_status.active.is_none() {
            format!("{name} (lost)")
        } else {
            name
        }
    }

    pub fn set_quick_controller_faces(&mut self, faces: [(TexId, u32, u32); 2]) {
        self.quick_controller_faces = Some(faces);
    }

    pub fn set_quick_bluetooth_faces(&mut self, index: usize, faces: [(TexId, u32, u32); 2]) {
        self.quick_bluetooth_faces[index] = Some(faces);
    }

    pub fn set_quick_battery_faces(&mut self, faces: [(TexId, u32, u32); 2]) {
        self.quick_battery_faces = Some(faces);
    }

    pub fn battery_text(&self) -> String {
        let Some(b) = self.battery else {
            return "Unavailable".into();
        };
        let status = match b.charge {
            Charge::Charging => "Charging",
            Charge::Full => "Full",
            Charge::Discharging => "Discharging",
            Charge::Unknown => "Status Unknown",
        };
        format!("{}% / {status}", b.percent)
    }

    pub fn set_bluetooth_audio(&mut self, active: bool) {
        self.bluetooth_audio = active;
        self.hud.set_headphones(self.headphones_active());
    }

    fn save_controller_config(&self) {
        if let Some(root) = &self.root {
            if let Err(e) = slot_store::write_controller_config(root, &self.controller_config) {
                eprintln!("slot: controller settings: {e}");
            }
        }
    }

    fn change_controller_setting(&mut self, row: QuickRow, right: bool) {
        match row {
            QuickRow::ControllerSelect => {
                let mut choices = vec![None];
                for pad in &self.controller_status.connected {
                    let id = Some(pad.id.clone());
                    if !choices.contains(&id) {
                        choices.push(id);
                    }
                }
                if !choices.contains(&self.controller_config.preferred) {
                    choices.push(self.controller_config.preferred.clone());
                }
                let at = choices
                    .iter()
                    .position(|p| *p == self.controller_config.preferred)
                    .unwrap_or(0);
                let at = if right {
                    (at + 1) % choices.len()
                } else {
                    (at + choices.len() - 1) % choices.len()
                };
                self.controller_config.preferred = choices[at].clone();
                if let Some(profile) = self.controller_target() {
                    self.controller_config
                        .profiles
                        .retain(|p| p.id != profile.id);
                    self.controller_config.profiles.push(profile);
                }
            }
            QuickRow::ControllerPause => {
                self.controller_config.pause_disconnect = !self.controller_config.pause_disconnect
            }
            _ => {
                let Some(mut profile) = self.controller_target() else {
                    return;
                };
                match row {
                    QuickRow::ControllerSwapAb => profile.swap_ab = !profile.swap_ab,
                    QuickRow::ControllerSwapXy => profile.swap_xy = !profile.swap_xy,
                    QuickRow::ControllerDeadzone => {
                        let at = STICK_DEADZONES
                            .iter()
                            .position(|d| *d == profile.deadzone)
                            .unwrap_or(3);
                        profile.deadzone =
                            STICK_DEADZONES[if right { (at + 1) % 6 } else { (at + 5) % 6 }];
                    }
                    QuickRow::ControllerReset => {
                        profile = ControllerProfile {
                            id: profile.id,
                            name: profile.name,
                            ..ControllerProfile::default()
                        }
                    }
                    _ => return,
                }
                self.controller_config
                    .profiles
                    .retain(|p| p.id != profile.id);
                self.controller_config.profiles.push(profile);
            }
        }
        self.save_controller_config();
    }

    pub fn set_sticker_face(&mut self, face: TexId) {
        self.sticker_face = Some(face);
    }

    pub fn set_players_face(&mut self, face: TexId) {
        self.players_face = Some(face);
    }

    /// The handheld and each connected controller, with the player each plays as.
    pub fn controller_seats(&self) -> Vec<(String, u8)> {
        let status = &self.controller_status;
        let pads = status.connected.iter().zip(&status.players);
        std::iter::once(("Handheld".to_string(), status.handheld_player()))
            .chain(pads.map(|(pad, player)| (pad.name.clone(), *player)))
            .collect()
    }

    pub fn handheld_player(&self) -> u8 {
        self.controller_status.handheld_player()
    }

    /// A PlayStation takes a third and fourth player through a multitap.
    pub fn multitap_wanted(&self) -> bool {
        let status = &self.controller_status;
        status.handheld_player() > 2 || status.players.iter().any(|p| *p > 2)
    }

    pub fn platform(&self) -> Platform {
        self.platform
    }

    pub fn set_clock_faces(&mut self, line: TexId, hint: TexId) {
        self.clock_faces = Some((line, hint));
    }

    pub fn set_cart_shadow(&mut self, face: TexId) {
        for (_, shelf) in &mut self.shelves {
            shelf.set_shadow(face);
        }
    }

    pub fn set_gb_cart_shadows(&mut self, notched: TexId, rounded: TexId) {
        for (_, shelf) in &mut self.shelves {
            shelf.set_gb_shadow(GbShell::Notched, notched);
            shelf.set_gb_shadow(GbShell::Rounded, rounded);
        }
    }

    pub fn set_wallpaper(&mut self, face: TexId) {
        self.wallpaper = Some(face);
    }

    pub fn set_bolt_face(&mut self, bolt: TexId) {
        self.bolt = Some(bolt);
    }

    pub fn set_shelf_platform_face(&mut self, face: TexId, w: u32) {
        self.shelf_platform = slot_ui::Printed::new(face, w);
    }

    pub fn set_shelf_switch_hint_faces(&mut self, faces: [(TexId, u32); 2]) {
        self.shelf_switch_hints = faces.map(|(face, w)| slot_ui::Printed::new(face, w));
    }

    pub fn clear_shelf_platform(&mut self) {
        self.shelf_platform = slot_ui::Printed::default();
    }

    pub fn slot_text(&self) -> Option<String> {
        match self.slot_letter {
            Some(letter) => Some(letter.to_string()),
            None => self.shelf_platform_name().map(str::to_string),
        }
    }

    pub fn shelf_platform_name(&self) -> Option<&'static str> {
        if self.shelf_kind == ShelfKind::Music {
            return Some("Music");
        }
        (!self.shelf().carts.is_empty()).then(|| self.shelves[self.shelf_at].0.name())
    }

    pub fn set_battery_percent_face(&mut self, face: TexId, w: u32) {
        self.battery_percent = slot_ui::Printed::new(face, w);
    }

    pub fn set_shelf_clock_face(&mut self, face: TexId, w: u32) {
        self.shelf_clock = slot_ui::Printed::new(face, w);
    }

    pub fn phase(&self) -> &Phase {
        &self.phase
    }

    pub fn carts(&self) -> impl Iterator<Item = &Cart> {
        self.shelves
            .iter()
            .flat_map(|(_, shelf)| shelf.carts.iter())
    }

    pub fn seated_cart(&self) -> Option<&Cart> {
        let stem = match &self.phase {
            Phase::Inserting { cart, .. }
            | Phase::Playing { cart }
            | Phase::Ejecting { cart, .. }
            | Phase::Polaroids { cart } => cart,
            Phase::Doze { cart: Some(cart) } => cart,
            _ => return None,
        };
        self.shelf().carts.iter().find(|c| c.stem == *stem)
    }

    pub fn single_cart(&self) -> bool {
        self.carts().count() == 1
    }

    pub fn set_faces(&mut self, faces: Vec<TexId>) {
        let mut faces = faces.into_iter();
        for (_, shelf) in &mut self.shelves {
            let n = shelf.carts.len();
            shelf.set_faces(faces.by_ref().take(n).collect());
        }
    }

    pub fn face_wants(&self) -> Vec<FaceWant> {
        let here = self.shelf();
        let screen = here.on_screen();
        let mut wants: Vec<(usize, usize)> = screen.iter().map(|i| (self.shelf_at, *i)).collect();
        for at in self.neighbour_shelves() {
            wants.extend(self.shelves[at].1.on_screen().into_iter().map(|i| (at, i)));
        }
        wants.extend(
            here.around(FACE_AHEAD)
                .into_iter()
                .filter(|i| !screen.contains(i))
                .map(|i| (self.shelf_at, i)),
        );
        wants
            .into_iter()
            .filter(|(at, i)| self.shelves[*at].1.face(*i).is_none())
            .map(|(shelf, index)| FaceWant {
                shelf,
                index,
                on_screen: shelf == self.shelf_at && screen.contains(&index),
            })
            .collect()
    }

    pub fn face_cart(&self, shelf: usize, index: usize) -> Option<&Cart> {
        self.shelves.get(shelf)?.1.carts.get(index)
    }

    pub fn set_cart_face(&mut self, shelf: usize, index: usize, tex: TexId) -> Option<TexId> {
        self.shelves.get_mut(shelf)?.1.set_face(index, tex)
    }

    pub fn shed_faces(&mut self) -> Vec<TexId> {
        let neighbours = self.neighbour_shelves();
        let mut shed = Vec::new();
        for (at, (_, shelf)) in self.shelves.iter_mut().enumerate() {
            let keep = if at == self.shelf_at {
                let mut keep = shelf.around(FACE_KEEP);
                for i in shelf.on_screen() {
                    if !keep.contains(&i) {
                        keep.push(i);
                    }
                }
                keep
            } else if neighbours.contains(&at) {
                shelf.on_screen()
            } else {
                Vec::new()
            };
            for i in 0..shelf.carts.len() {
                if !keep.contains(&i) {
                    shed.extend(shelf.take_face(i));
                }
            }
        }
        shed
    }

    fn neighbour_shelves(&self) -> Vec<usize> {
        let mut out = Vec::new();
        for by in [1, -1] {
            if let Some(at) = self.next_shelf(by) {
                if !out.contains(&at) {
                    out.push(at);
                }
            }
        }
        out
    }

    pub fn set_snapshot(&mut self, snapshot: Box<dyn Snapshot>) {
        self.snapshot = Some(snapshot);
    }

    pub fn set_core(&mut self, core: Core) {
        self.core = core;
    }

    pub fn set_platform(&mut self, platform: Platform) {
        self.platform = platform;
    }

    pub fn set_named_core(&mut self, named: bool) {
        self.named_core = named;
    }

    pub fn set_video_mode(&mut self, mode: VideoMode) {
        self.video_mode = mode;
    }

    pub fn source_rect(&self) -> [f32; 4] {
        video_mode::source_rect(self.platform, self.video_mode)
    }

    pub fn set_run_ahead(&mut self, frames: u8) {
        self.run_ahead = frames.min(RUN_AHEAD_MAX);
    }

    pub fn run_ahead(&self) -> u8 {
        self.run_ahead
    }

    fn step_run_ahead(&mut self, right: bool) {
        let options = RUN_AHEAD_MAX + 1;
        let step = if right { 1 } else { RUN_AHEAD_MAX };
        self.run_ahead = (self.run_ahead + step) % options;
        let (Some(root), Phase::Playing { cart }) = (self.root.clone(), &self.phase) else {
            return;
        };
        if let Err(e) = run_ahead::write_run_ahead(&root, cart, self.run_ahead) {
            eprintln!("slot: run ahead: could not write run_ahead.txt: {e}");
        }
    }

    fn slot_owns_the_shoulders(&self) -> bool {
        matches!(self.phase, Phase::Playing { .. })
            && matches!(self.platform, Platform::Gb | Platform::Gbc)
    }

    pub fn taken_buttons(&self) -> &'static [Btn] {
        if self.slot_owns_the_shoulders() {
            &[Btn::L1, Btn::R1]
        } else {
            &[]
        }
    }

    pub fn takes_from_the_game(&self, action: Action) -> bool {
        match action {
            Action::GbaDown(btn) | Action::GbaUp(btn) => self.taken_buttons().contains(&btn),
            _ => false,
        }
    }

    fn set_picture(&mut self, mode: VideoMode) {
        if self.video_mode == mode {
            return;
        }
        self.video_mode = mode;
        let (Some(root), Phase::Playing { cart }) = (self.root.clone(), &self.phase) else {
            return;
        };
        if let Err(e) = video_mode::write_video_mode(&root, cart, mode) {
            eprintln!("slot: video: could not write video_mode.txt: {e}");
        }
    }

    pub fn set_link_loaded(&mut self, serial: &'static str) {
        self.link_loaded = Some(serial);
    }

    pub fn link_mode(&self, stem: &str) -> (LinkKind, &'static str) {
        let Some((cart, auto)) = self.auto_link(stem) else {
            return (LinkKind::Cable, "auto");
        };
        let chosen = self.link_choices.get(stem).copied().unwrap_or(auto);
        (chosen, serial_option(chosen, auto, &cart.code, &cart.title))
    }

    pub fn link_active(&self) -> bool {
        self.link.is_some()
    }

    pub fn begin_link(&mut self, client_id: u16) {
        self.stop_playback();
        self.link = Some(LinkSession {
            client_id,
            lost_at: None,
        });
        self.sync_link_badge();
    }

    pub fn link_client_id(&self) -> Option<u16> {
        self.link.as_ref().map(|s| s.client_id)
    }

    pub fn may_rewind(&self) -> bool {
        self.state.rewind && !self.link_active()
    }

    pub fn turbo(&self) -> bool {
        self.state.turbo
    }

    pub fn records_rewind(&self) -> bool {
        self.state.rewind
    }

    pub fn picture_rect(&self) -> [f32; 4] {
        if self.platform.multiplayer() {
            return slot_gfx::WHOLE_TEXTURE;
        }
        let (w, h) = self.platform.picture();
        let (sw, sh) = (slot_gfx::SRC_W as f32, slot_gfx::SRC_H as f32);
        let (x, y) = ((sw - w as f32) / 2.0, (sh - h as f32) / 2.0);
        [x / sw, y / sh, (x + w as f32) / sw, (y + h as f32) / sh]
    }

    pub fn may_load_state(&self) -> bool {
        !self.link_active()
    }

    pub fn may_fast_forward(&self) -> bool {
        !self.link_active()
    }

    pub fn end_link(&mut self) {
        self.link = None;
        self.link_player = None;
        self.sync_link_badge();
        self.radio.ask(RadioJob::Down);
        self.radio.ask(RadioJob::Cool);
    }

    pub fn peer_lost(&mut self) {
        if matches!(self.game_menu, Some(GameMenu::Linked { .. })) {
            self.game_menu = None;
        }
        let now = self.now();
        if let Some(session) = &mut self.link {
            session.lost_at.get_or_insert(now);
        }
        self.sync_link_badge();
    }

    pub fn bios_mismatch(&mut self) {
        if !self.link_active() {
            return;
        }
        let role = self
            .link_client_id()
            .map_or(self.last_role, LinkRow::from_client_id);
        self.end_link();
        self.hud.toast(Toast::BiosMismatch, self.now());
        self.unplug(role);
    }

    pub fn peer_ended(&mut self) {
        if !self.link_active() {
            return;
        }
        let role = self
            .link_client_id()
            .map_or(self.last_role, LinkRow::from_client_id);
        self.end_link();
        self.hud.toast(Toast::PeerEnded, self.now());
        self.unplug(role);
    }

    pub fn link_badge(&self) -> LinkBadge {
        self.hud.link()
    }

    pub fn set_link_badge_faces(&mut self, faces: Vec<TexId>) {
        self.hud.set_link_faces(faces);
    }

    fn sync_link_badge(&mut self) {
        let badge = match &self.link {
            None => LinkBadge::Off,
            Some(s) => match (s.client_id == 0, s.lost_at.is_some()) {
                (true, false) => LinkBadge::Hosting,
                (false, false) => LinkBadge::Joined,
                (true, true) => LinkBadge::HostingLost,
                (false, true) => LinkBadge::JoinedLost,
            },
        };
        self.hud.set_link(badge);
    }

    pub fn take_sfx(&mut self) -> Option<Sfx> {
        self.sfx.take()
    }

    pub fn set_power(&mut self, mut power: Power) {
        power.set_backlight(self.state.brightness);
        if self.screen_off {
            power.set_screen_off(true);
        }
        if self.lid_closed || matches!(self.phase, Phase::Doze { .. }) {
            power.on_close();
        }
        self.headphones = power.headphones();
        self.hud.set_headphones(self.headphones_active());
        let secs = power.now();
        if matches!(self.phase, Phase::SetClock { .. }) || secs < CLOCK_FLOOR {
            self.phase = clock_screen(secs, 0, false);
        }
        self.power = Some(power);
        self.battery_at = self.now();
        self.charge_at = self.now();
        self.last_led = None;
    }

    pub fn set_rumble(&mut self, strength: u16) {
        if let Some(power) = &mut self.power {
            power.set_rumble(strength);
        }
    }

    pub fn rumble_enabled(&self) -> bool {
        self.state.rumble
    }

    pub fn ff_speed(&self) -> u8 {
        self.state.ff_speed
    }

    pub fn ff_sound(&self) -> bool {
        self.state.ff_sound
    }

    pub fn screen_shader(&self) -> Shader {
        match self.platform {
            Platform::Gba => self.state.shader_gba,
            Platform::Gb | Platform::Gbc => self.state.shader_gb,
            Platform::Psx => self.state.shader_psx,
            Platform::Snes => self.state.shader_snes,
        }
    }

    pub fn colour_correction(&self) -> bool {
        self.state.colour_correction
    }

    pub fn powering_off(&self) -> bool {
        self.powering_off
    }

    pub fn ready_to_power_off(&self) -> bool {
        self.powering_off && self.now() >= self.act_at
    }

    pub fn ready_to_restart(&self) -> bool {
        self.restarting && self.now() >= self.act_at
    }

    pub fn shutting_down(&self) -> bool {
        self.powering_off || self.restarting
    }

    pub fn restarting(&self) -> bool {
        self.restarting
    }

    pub fn restart(&mut self) {
        self.settle_saves();
        if let Some(power) = &mut self.power {
            power.restart();
        }
    }

    pub fn power_menu(&self) -> Option<usize> {
        self.power_menu
    }

    pub fn core_picker(&self) -> Option<Core> {
        self.core_picker.map(|p| p.seat())
    }

    pub fn core_picker_chip(&self) -> Option<Chip> {
        self.core_picker.map(|p| p.chip(self.now()))
    }

    pub fn game_menu_open(&self) -> bool {
        self.game_menu.is_some() || self.disc_menu.is_some() || self.state_menu.is_some()
    }

    pub fn state_menu_row(&self) -> Option<usize> {
        self.state_menu
    }

    pub fn take_game_reset_request(&mut self) -> Option<crate::system_app::GameReset> {
        if !std::mem::take(&mut self.game_reset_requested) {
            return None;
        }
        Some(crate::system_app::GameReset {
            root: self.root.clone()?,
            cart: self.seated()?.to_string(),
            platform: self.platform,
        })
    }

    pub fn playback(&self) -> Option<Playback> {
        self.playback
    }

    pub fn stop_playback(&mut self) {
        self.playback = None;
    }

    /// The player screen is up, from the shelf's menu or over a held game.
    pub fn choosing_players(&self) -> bool {
        matches!(self.phase, Phase::Players) || (self.players_open && self.state_menu.is_some())
    }

    pub fn state_slot_menu(&self) -> Option<(StateSlotMode, usize)> {
        self.state_slot_menu
    }

    pub fn state_slot_labels(&self) -> Vec<String> {
        let Some((mode, _)) = self.state_slot_menu else {
            return Vec::new();
        };
        let mut labels: Vec<_> = self
            .state_slot_metadata
            .iter()
            .enumerate()
            .map(|(slot, metadata)| {
                let status = match metadata {
                    None => "Empty".into(),
                    Some(metadata) => metadata
                        .modified()
                        .ok()
                        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                        .and_then(|duration| i64::try_from(duration.as_secs()).ok())
                        .and_then(|secs| {
                            secs.checked_add(i64::from(self.state.utc_offset_min) * 60)
                        })
                        .map(|secs| {
                            let (year, month, day) =
                                slot_store::civil_from_days(secs.div_euclid(86_400));
                            format!("{year:04}-{month:02}-{day:02} {}", slot_ui::hhmm(secs))
                        })
                        .unwrap_or_else(|| "Date Unknown".into()),
                };
                format!("Slot {}: {status}", slot + 1)
            })
            .collect();
        if mode == StateSlotMode::Load {
            labels.push("Saved History".into());
        }
        labels.push("Back".into());
        labels
    }

    pub fn set_state_slot_faces(&mut self, faces: Vec<(TexId, u32, u32)>) {
        self.state_slot_faces = faces;
    }

    pub fn set_state_slot_undo_face(&mut self, face: Option<(TexId, u32, u32)>) {
        self.state_slot_undo_face = face;
    }
    pub fn set_state_menu_faces(
        &mut self,
        faces: Vec<(TexId, u32, u32)>,
        run_ahead: Vec<(TexId, u32, u32)>,
    ) {
        self.state_faces = faces;
        self.run_ahead_faces = run_ahead;
    }

    pub fn disc_menu_row(&self) -> Option<usize> {
        self.disc_menu
    }
    pub fn discs(&self) -> (usize, usize, bool) {
        self.discs
    }
    pub fn set_discs(&mut self, discs: (usize, usize, bool)) {
        self.discs = discs;
    }
    pub fn take_disc_change(&mut self) -> Option<usize> {
        self.disc_change.take()
    }
    pub fn set_disc_menu_faces(&mut self, faces: Vec<(TexId, u32, u32)>) {
        self.disc_faces = faces;
    }
    pub fn set_disc_title(&mut self, title: (TexId, u32, u32)) {
        self.disc_title = Some(title);
    }

    pub fn game_menu(&self) -> Option<GameMenu> {
        self.game_menu
    }

    pub fn take_link_transport(&mut self) -> Option<(u16, Box<dyn LinkChannel>)> {
        self.link_transport.take()
    }

    pub fn take_link_reload(&mut self) -> Option<(String, &'static str)> {
        self.link_reload.take()
    }

    pub fn link_player(&self) -> Option<u8> {
        self.link_player
    }

    pub fn take_colour_correction(&mut self) -> Option<bool> {
        self.colour_pending.take()
    }

    pub fn gb_palette(&self) -> Option<GbPalette> {
        self.state.gb_palettes.then_some(self.state.gb_palette)
    }

    pub fn set_palette_live(&mut self, live: bool) {
        self.palette_live = live;
    }

    pub fn palette_live(&self) -> bool {
        self.palette_live
    }

    pub fn take_gb_palette(&mut self) -> Option<GbPalette> {
        self.palette_pending.take()
    }

    pub fn core(&self) -> Core {
        self.core
    }

    pub fn link_reload_done(&mut self) {
        let Some(reload) = self.reload.take() else {
            return;
        };
        if reload.fallback {
            self.link_choices.insert(reload.stem, reload.from);
            self.close_game_menu();
            return self.refuse();
        }
        if reload.cancelled {
            return self.close_game_menu();
        }
        let since = match self.game_menu {
            Some(GameMenu::Working { since, .. }) => since,
            _ => self.now(),
        };
        self.start_link_from(
            LinkStarter::spawn(reload.role.role(), link_port()),
            reload.role.client_id(),
            since,
        );
    }

    pub fn link_reload_failed(&mut self) {
        let Some(mut reload) = self.reload.take() else {
            return;
        };
        if !reload.fallback {
            self.link_reload = Some((reload.stem.clone(), reload.from_serial));
            reload.fallback = true;
            self.reload = Some(reload);
            return;
        }
        self.link_choices.insert(reload.stem, reload.from);
        self.refuse_seated();
    }

    pub fn selected_stem(&self) -> Option<&str> {
        if self.shelf_kind == ShelfKind::Music {
            return None;
        }
        self.shelf()
            .carts
            .get(self.shelf().index)
            .map(|c| c.stem.as_str())
    }

    pub fn battery(&self) -> Option<Battery> {
        self.battery
    }

    pub fn poweroff(&mut self) {
        self.settle_saves();
        if let Some(power) = &mut self.power {
            power.poweroff();
        }
    }

    pub fn apply(&mut self, action: Action) {
        match action {
            Action::LidClose => return self.close_lid(),
            Action::LidOpen => return self.open_lid(),
            _ => {}
        }
        if self.screen_off {
            if !matches!(
                action,
                Action::GbaUp(_) | Action::FfStop | Action::RewindStop | Action::PowerOff
            ) {
                self.restore_screen();
                self.woke_on_press = action == Action::PowerPress;
            }
            return;
        }
        if self.power_menu.is_some() {
            return self.power_menu_input(action);
        }
        match action {
            Action::PowerPress => {
                if self.link_active() {
                    self.end_link();
                    return;
                }
                self.woke_on_press = matches!(self.phase, Phase::Doze { .. });
                if self.woke_on_press {
                    self.wake();
                }
                return self.flush_resume();
            }
            Action::PowerTap => return self.power_press(),
            Action::PowerHold => return self.open_power_menu(),
            Action::PowerOff => return,
            _ => {}
        }
        if let Phase::SetClock {
            picker, from_menu, ..
        } = &mut self.phase
        {
            match action {
                Action::GbaDown(Btn::Left) | Action::ShelfLeft => picker.left(),
                Action::GbaDown(Btn::Right) | Action::ShelfRight => picker.right(),
                Action::GbaDown(Btn::Up) => picker.up(),
                Action::GbaDown(Btn::Down) => picker.down(),
                Action::GbaDown(Btn::A) | Action::Insert => self.confirm_clock(),
                Action::GbaDown(Btn::B) if *from_menu => {
                    self.phase = Phase::QuickMenu {
                        row: QuickRow::DateTime,
                    }
                }
                _ => {}
            }
            return;
        }
        if self.adjust(action) {
            return;
        }
        if matches!(self.phase, Phase::Shelf)
            && self.core_picker.is_none()
            && self.shelf_kind == ShelfKind::Music
        {
            match action {
                Action::QuickMenu => self.open_quick_menu(),
                Action::GbaDown(Btn::L1) if !self.music.flipped && !self.music.fullscreen => {
                    self.switch_shelf(-1)
                }
                Action::GbaDown(Btn::R1) if !self.music.flipped && !self.music.fullscreen => {
                    self.switch_shelf(1)
                }
                _ => self.music.input(action, self.now()),
            }
            return;
        }
        match action {
            Action::GbaUp(Btn::Left) => self.shelf_mut().release_left(),
            Action::GbaUp(Btn::Right) => self.shelf_mut().release_right(),
            _ => {}
        }
        if let Some((mode, row)) = self.state_slot_menu {
            let rows = SAVE_SLOTS + if mode == StateSlotMode::Load { 2 } else { 1 };
            match action {
                Action::GbaDown(Btn::Up) => {
                    self.state_slot_menu = Some((mode, (row + rows - 1) % rows))
                }
                Action::GbaDown(Btn::Down) => self.state_slot_menu = Some((mode, (row + 1) % rows)),
                Action::GbaDown(Btn::B) => self.state_slot_menu = None,
                Action::QuickMenu => {
                    self.state_slot_menu = None;
                    self.state_menu = None;
                }
                Action::GbaDown(Btn::X) => {
                    self.undo(self.now());
                    self.refresh_state_slots();
                }
                Action::GbaDown(Btn::A) if row == rows - 1 => self.state_slot_menu = None,
                Action::GbaDown(Btn::A) if row < SAVE_SLOTS => {
                    let done = match mode {
                        StateSlotMode::Save => self.save_slot(row),
                        StateSlotMode::Load => self.load_slot(row),
                    };
                    if done {
                        self.state_slot_menu = None;
                        self.state_menu = None;
                    }
                }
                Action::GbaDown(Btn::A) | Action::Polaroids => {
                    self.open_polaroids();
                    if matches!(self.phase, Phase::Polaroids { .. }) {
                        self.state_slot_menu = None;
                        self.state_menu = None;
                    }
                }
                _ => {}
            }
            return;
        }
        if self.state_menu.is_some() && self.players_open {
            if matches!(
                action,
                Action::GbaDown(Btn::A | Btn::B | Btn::Start) | Action::QuickMenu
            ) {
                self.players_open = false;
            }
            return;
        }
        if let Some(row) = self.state_menu {
            if self.playback.is_some() {
                match action {
                    Action::GbaUp(Btn::A) => self.stop_playback(),
                    Action::Eject => {
                        self.stop_playback();
                        self.eject();
                    }
                    Action::GbaDown(Btn::B) | Action::QuickMenu => {
                        self.stop_playback();
                        self.state_menu = None;
                    }
                    _ => {}
                }
                return;
            }
            match action {
                Action::GbaDown(Btn::Up) => {
                    self.state_menu = Some((row + STATE_ROWS - 1) % STATE_ROWS)
                }
                Action::GbaDown(Btn::Down) => self.state_menu = Some((row + 1) % STATE_ROWS),
                Action::GbaDown(Btn::B) | Action::QuickMenu => self.state_menu = None,
                Action::GbaDown(Btn::A) if row == 0 => {
                    self.open_state_slots(StateSlotMode::Save);
                }
                Action::GbaDown(Btn::A) if row == 1 => self.open_state_slots(StateSlotMode::Load),
                Action::GbaDown(Btn::A | Btn::Right) | Action::ShelfRight
                    if row == STATE_ROW_RUN_AHEAD =>
                {
                    self.step_run_ahead(true)
                }
                Action::GbaDown(Btn::Left) | Action::ShelfLeft if row == STATE_ROW_RUN_AHEAD => {
                    self.step_run_ahead(false)
                }
                Action::GbaDown(Btn::A) if row == STATE_ROW_PLAYERS => self.players_open = true,
                Action::GbaDown(Btn::A) if row == STATE_ROW_REWIND => {
                    if self.may_rewind() {
                        self.playback = Some(Playback::Rewind);
                    } else {
                        self.refuse();
                    }
                }
                Action::GbaDown(Btn::A) if row == STATE_ROW_FAST_FORWARD => {
                    if self.may_fast_forward() {
                        self.playback = Some(Playback::FastForward);
                    } else {
                        self.refuse();
                    }
                }
                Action::GbaDown(Btn::A) if row == STATE_ROW_BACK => self.state_menu = None,
                Action::GbaDown(Btn::A) if row == STATE_ROW_RESET => {
                    // Never ask the stalled core for another save before recovery.
                    self.pending = None;
                    self.game_reset_requested = true;
                }
                // The second tap of a double tap lands here, the first having opened this menu.
                Action::GbaDown(Btn::A) | Action::Polaroids => {
                    self.open_polaroids();
                    if matches!(self.phase, Phase::Polaroids { .. }) {
                        self.state_menu = None;
                    }
                }
                _ => {}
            }
            return;
        }
        if let Some(row) = self.disc_menu {
            match action {
                Action::GbaDown(Btn::Up) => self.disc_menu = Some((row + 2) % 3),
                Action::GbaDown(Btn::Down) => self.disc_menu = Some((row + 1) % 3),
                Action::GbaDown(Btn::B) | Action::GameMenu => self.close_game_menu(),
                Action::GbaDown(Btn::A) if row == 2 => self.close_game_menu(),
                Action::GbaDown(Btn::A) => {
                    let (index, count, _) = self.discs;
                    let wanted = if row == 0 {
                        index.checked_sub(1)
                    } else {
                        (index + 1 < count).then_some(index + 1)
                    };
                    match wanted {
                        Some(i) => self.disc_change = Some(i),
                        None => self.refuse(),
                    }
                }
                _ => {}
            }
            return;
        }
        if self.game_menu.is_some() {
            return self.game_menu_input(action);
        }
        let now = self.now();
        match self.phase {
            Phase::Shelf => match action {
                Action::GbaDown(Btn::Start) if self.core_picker.is_none() => {
                    self.open_core_picker()
                }
                _ if self.core_picker.is_some() => self.core_picker_input(action),
                Action::GbaDown(Btn::Up) => self.jump_letter(-1),
                Action::GbaDown(Btn::Down) => self.jump_letter(1),
                Action::ShelfLeft | Action::GbaDown(Btn::Left) => self.shelf_mut().hold_left(now),
                Action::ShelfRight | Action::GbaDown(Btn::Right) => {
                    self.shelf_mut().hold_right(now)
                }
                Action::QuickMenu => self.open_quick_menu(),
                Action::GbaDown(Btn::A) => self.play_held = Some(now),
                Action::GbaUp(Btn::A) => {
                    if self.play_held.take().is_some() {
                        self.insert(!self.state.eject_save);
                    }
                }
                Action::Insert => self.insert(!self.state.eject_save),
                Action::GbaDown(Btn::L1) => self.switch_shelf(-1),
                Action::GbaDown(Btn::R1) => self.switch_shelf(1),
                _ => {}
            },
            Phase::Inserting { .. } if action == Action::Eject => self.eject(),
            Phase::Playing { .. } => match action {
                Action::GbaDown(Btn::L1) if self.slot_owns_the_shoulders() => {
                    self.set_picture(VideoMode::Stretch)
                }
                Action::GbaDown(Btn::R1) if self.slot_owns_the_shoulders() => {
                    self.set_picture(VideoMode::Actual)
                }
                Action::Eject => self.eject(),
                Action::QuickMenu => self.open_state_menu(),
                Action::GameMenu => self.game_menu_shortcut(),
                Action::Polaroids => self.open_polaroids(),
                Action::SaveState => self.save_state(),
                Action::LoadState => self.load_newest(),
                Action::RewindStart if !self.may_rewind() => self.refuse(),
                Action::FfStart if !self.may_fast_forward() => self.refuse(),
                _ => {}
            },
            Phase::Polaroids { .. } => match action {
                Action::ShelfLeft | Action::GbaDown(Btn::Left) => self.flick(Polaroids::left),
                Action::ShelfRight | Action::GbaDown(Btn::Right) => self.flick(Polaroids::right),
                Action::GbaDown(Btn::A) => self.load_selected(),
                Action::GbaDown(Btn::B) | Action::Polaroids => self.close_polaroids(),
                Action::GbaDown(Btn::X) => self.undo(self.now()),
                Action::GbaDown(Btn::Y) => self.delete_selected(),
                _ => {}
            },
            Phase::About if action == Action::GbaDown(Btn::B) || action == Action::QuickMenu => {
                self.phase = Phase::QuickMenu {
                    row: QuickRow::About,
                }
            }
            Phase::Players
                if matches!(
                    action,
                    Action::GbaDown(Btn::A | Btn::B | Btn::Start) | Action::QuickMenu
                ) =>
            {
                self.phase = Phase::QuickMenu {
                    row: QuickRow::ControllerPlayers,
                }
            }
            Phase::QuickMenu { row } => self.quick_menu_input(row, action),
            _ => {}
        }
    }

    fn open_state_menu(&mut self) {
        if self.link_active() {
            return self.refuse();
        }
        self.players_open = false;
        self.stop_playback();
        self.state_menu = Some(0);
    }

    fn refresh_state_slots(&mut self) {
        self.state_slot_metadata = std::array::from_fn(|slot| {
            self.ring()
                .and_then(|r| r.slot_path(slot).ok())
                .and_then(|path| path.metadata().ok())
                .filter(|metadata| metadata.is_file())
        });
    }

    fn open_state_slots(&mut self, mode: StateSlotMode) {
        self.refresh_state_slots();
        self.state_slot_menu = Some((mode, 0));
    }

    fn open_quick_menu(&mut self) {
        self.phase = Phase::QuickMenu {
            row: QuickRow::MAIN[0],
        };
    }

    fn quick_menu_input(&mut self, row: QuickRow, action: Action) {
        let row = match action {
            Action::GbaDown(Btn::Up) => row.up(),
            Action::GbaDown(Btn::Down) => row.down(),
            Action::GbaDown(Btn::Left) => return self.change_setting(row, false),
            Action::GbaDown(Btn::Right) => return self.change_setting(row, true),
            Action::GbaDown(Btn::A) => return self.open_quick_row(row),
            Action::GbaDown(Btn::B) | Action::QuickMenu => {
                self.phase = match row.parent() {
                    Some(row) => Phase::QuickMenu { row },
                    None => Phase::Shelf,
                };
                return;
            }
            _ => return,
        };
        self.phase = Phase::QuickMenu { row };
    }

    fn open_quick_row(&mut self, row: QuickRow) {
        match row {
            QuickRow::ScreenOff => {
                self.phase = Phase::Shelf;
                self.screen_off = true;
                self.shelf_mut().release_hold();
                self.play_held = None;
                self.music.release();
                if let Some(power) = &mut self.power {
                    power.set_screen_off(true);
                }
            }
            QuickRow::BluetoothScan => self.bluetooth.request(crate::bluetooth::Operation::Scan),
            QuickRow::BluetoothOff => {
                if self.bluetooth_audio {
                    self.music.commands.push(crate::music::Command::Pause);
                }
                self.bluetooth
                    .request(crate::bluetooth::Operation::PowerOff);
            }
            QuickRow::BluetoothConnect => {
                self.bluetooth.request(crate::bluetooth::Operation::Connect)
            }
            QuickRow::BluetoothDisconnect => self
                .bluetooth
                .request(crate::bluetooth::Operation::Disconnect),
            QuickRow::BluetoothForget => {
                self.bluetooth.request(crate::bluetooth::Operation::Forget)
            }
            QuickRow::BluetoothSelect | QuickRow::BluetoothStatus => {}
            QuickRow::Music => {
                self.shelf_mut().release_hold();
                self.play_held = None;
                self.shelf_kind = ShelfKind::Music;
                self.phase = Phase::QuickMenu {
                    row: QuickRow::MusicPlay,
                }
            }
            QuickRow::MusicPlay => self.music.toggle(),
            QuickRow::MusicPrevious => self.music.commands.push(crate::music::Command::Previous),
            QuickRow::MusicNext => self.music.commands.push(crate::music::Command::Next),
            QuickRow::MusicRescan => {
                self.music.rescan = true;
                self.music.scanning = true;
            }
            QuickRow::MusicStyle => {
                if self.music.snapshot.album.is_some() {
                    self.shelf_kind = ShelfKind::Music;
                    self.music.fullscreen = true;
                    self.phase = Phase::Shelf;
                }
            }
            QuickRow::MusicShuffle | QuickRow::MusicRepeat | QuickRow::MusicLid => {
                self.change_setting(row, true)
            }
            QuickRow::DateTime => {
                self.phase = clock_screen(self.utc_secs(), self.state.utc_offset_min, true);
            }
            QuickRow::About => self.phase = Phase::About,
            QuickRow::ControllerPlayers => self.phase = Phase::Players,
            QuickRow::DeviceInfo => {
                // The system menu is shelf-only: no game is running here.
                if self.root.as_ref().is_some_and(|root| {
                    root.join("System/device-info/device-info").is_file()
                        && root.join("System/device-info.sh").is_file()
                }) {
                    self.state_writer.flush();
                    self.persist();
                    self.device_info_requested = true;
                } else {
                    eprintln!("slot: Device Info is missing; rebuild the app bundle");
                    self.refuse();
                }
            }
            QuickRow::Screen
            | QuickRow::Game
            | QuickRow::VideoOut
            | QuickRow::Controllers
            | QuickRow::Bluetooth
            | QuickRow::Battery => {
                if let Some(row) = row.child() {
                    self.phase = Phase::QuickMenu { row };
                }
            }
            QuickRow::ControllerReset => self.change_controller_setting(row, true),
            QuickRow::ControllerSelect
            | QuickRow::ControllerSwapAb
            | QuickRow::ControllerSwapXy
            | QuickRow::ControllerDeadzone
            | QuickRow::ControllerPause => {}
            QuickRow::EjectSave
            | QuickRow::Turbo
            | QuickRow::Rewind
            | QuickRow::FastForward
            | QuickRow::FastForwardSound
            | QuickRow::ColourCorrection
            | QuickRow::GbaShader
            | QuickRow::GbShader
            | QuickRow::SnesShader
            | QuickRow::PsxShader
            | QuickRow::GbPalettes
            | QuickRow::CoverFlow
            | QuickRow::HdmiOutput
            | QuickRow::HdmiResolution
            | QuickRow::HdmiScale
            | QuickRow::HdmiMargin
            | QuickRow::DockedLid
            | QuickRow::AudioOutput
            | QuickRow::Rumble
            | QuickRow::BatteryStatus => {}
            QuickRow::SleepShutdown | QuickRow::ShutdownCharging => self.change_setting(row, true),
        }
    }

    fn change_setting(&mut self, row: QuickRow, right: bool) {
        if row.page() == QuickRow::BLUETOOTH {
            if row == QuickRow::BluetoothSelect {
                self.bluetooth.step(right);
            }
            return;
        }
        if row.page() == QuickRow::MUSIC {
            match row {
                QuickRow::MusicShuffle => {
                    self.music.settings.shuffle = !self.music.settings.shuffle
                }
                QuickRow::MusicLid => {
                    self.music.settings.keep_playing_closed =
                        !self.music.settings.keep_playing_closed
                }
                QuickRow::MusicStyle => {
                    self.music.settings.style = match self.music.settings.style {
                        slot_store::VisualizerStyle::Bars => slot_store::VisualizerStyle::Waveform,
                        slot_store::VisualizerStyle::Waveform => slot_store::VisualizerStyle::Bars,
                    }
                }
                QuickRow::MusicRepeat => {
                    let options = [
                        slot_store::Repeat::Off,
                        slot_store::Repeat::Album,
                        slot_store::Repeat::Track,
                    ];
                    let i = options
                        .iter()
                        .position(|r| *r == self.music.settings.repeat)
                        .unwrap();
                    self.music.settings.repeat = options[(i + if right { 1 } else { 2 }) % 3];
                }
                _ => return,
            }
            self.music
                .commands
                .push(crate::music::Command::Settings(self.music.settings.clone()));
            self.sync_lid_policy();
            return;
        }
        if row.page() == QuickRow::CONTROLLERS {
            if row != QuickRow::ControllerReset {
                self.change_controller_setting(row, right);
            }
            return;
        }
        let s = &mut self.state;
        match row {
            QuickRow::SleepShutdown => s.sleep_shutdown = s.sleep_shutdown.step(right),
            QuickRow::ShutdownCharging => s.shutdown_charging = !s.shutdown_charging,
            QuickRow::Battery | QuickRow::BatteryStatus => return,
            QuickRow::Bluetooth
            | QuickRow::BluetoothScan
            | QuickRow::BluetoothSelect
            | QuickRow::BluetoothConnect
            | QuickRow::BluetoothDisconnect
            | QuickRow::BluetoothForget
            | QuickRow::BluetoothOff
            | QuickRow::BluetoothStatus => return,
            QuickRow::FastForward => {
                let to = ff_next(s.ff_speed, right);
                if to == s.ff_speed {
                    return;
                }
                s.ff_speed = to;
            }
            QuickRow::FastForwardSound => s.ff_sound = !s.ff_sound,
            QuickRow::ColourCorrection => {
                s.colour_correction = !s.colour_correction;
                self.colour_pending = Some(s.colour_correction);
            }
            QuickRow::GbaShader => {
                let to = s.shader_gba.step(&Shader::ALL, right);
                if to == s.shader_gba {
                    return;
                }
                s.shader_gba = to;
            }
            QuickRow::GbShader => {
                let to = s.shader_gb.step(&Shader::ALL, right);
                if to == s.shader_gb {
                    return;
                }
                s.shader_gb = to;
            }
            QuickRow::SnesShader => s.shader_snes = s.shader_snes.step(&Shader::ALL, right),
            QuickRow::PsxShader => s.shader_psx = s.shader_psx.step(&Shader::ALL, right),
            QuickRow::Rumble => s.rumble = !s.rumble,
            QuickRow::EjectSave => s.eject_save = !s.eject_save,
            QuickRow::Turbo => s.turbo = !s.turbo,
            QuickRow::Rewind => s.rewind = !s.rewind,
            QuickRow::GbPalettes => s.gb_palettes = !s.gb_palettes,
            QuickRow::CoverFlow => s.coverflow = !s.coverflow,
            QuickRow::HdmiOutput => s.hdmi_auto = !s.hdmi_auto,
            QuickRow::HdmiResolution => s.hdmi_1080p = !s.hdmi_1080p,
            QuickRow::DockedLid => s.docked_lid = !s.docked_lid,
            QuickRow::AudioOutput => s.audio_output = s.audio_output.step(right),
            QuickRow::HdmiScale => s.hdmi_scale = s.hdmi_scale.step(right),
            QuickRow::HdmiMargin => {
                s.hdmi_margin = (s.hdmi_margin + if right { 5 } else { 10 }) % 15
            }
            QuickRow::DateTime
            | QuickRow::ScreenOff
            | QuickRow::DeviceInfo
            | QuickRow::About
            | QuickRow::Screen
            | QuickRow::Game
            | QuickRow::VideoOut
            | QuickRow::Controllers
            | QuickRow::ControllerPlayers
            | QuickRow::ControllerSelect
            | QuickRow::ControllerSwapAb
            | QuickRow::ControllerSwapXy
            | QuickRow::ControllerDeadzone
            | QuickRow::ControllerPause
            | QuickRow::ControllerReset => return,
            QuickRow::Music
            | QuickRow::MusicPlay
            | QuickRow::MusicPrevious
            | QuickRow::MusicNext
            | QuickRow::MusicShuffle
            | QuickRow::MusicRepeat
            | QuickRow::MusicStyle
            | QuickRow::MusicLid
            | QuickRow::MusicRescan => return,
        }
        self.sync_shelf_settings();
        self.persist();
        if row == QuickRow::SleepShutdown {
            if let Some(power) = &mut self.power {
                power.set_timeout(self.state.sleep_shutdown.duration());
            }
        }
        if row == QuickRow::DockedLid {
            self.sync_lid_policy();
        }
    }

    pub fn apply_at(&mut self, action: Action, now: Millis) {
        self.clock = now as f64;
        self.apply(action);
    }

    fn step_shader(&mut self, right: bool) {
        if !matches!(self.phase, Phase::Inserting { .. } | Phase::Playing { .. }) {
            return;
        }
        let shader = match self.platform {
            Platform::Gba => &mut self.state.shader_gba,
            Platform::Gb | Platform::Gbc => &mut self.state.shader_gb,
            Platform::Psx => &mut self.state.shader_psx,
            Platform::Snes => &mut self.state.shader_snes,
        };
        *shader = shader.step(&Shader::ALL, right);
        let said = match *shader {
            Shader::Off => Toast::ShaderOff,
            Shader::Lcd3x => Toast::ShaderLcd3x,
            Shader::Grid => Toast::ShaderGrid,
            Shader::Dot => Toast::ShaderDot,
            Shader::Simpletex => Toast::ShaderSimpletex,
        };
        self.persist();
        self.hud.toast(said, self.now());
    }

    fn adjust(&mut self, action: Action) -> bool {
        if action == Action::MuteToggle {
            self.mute_toggle();
            return true;
        }
        if let Action::ShaderNext | Action::ShaderPrev = action {
            self.step_shader(action == Action::ShaderNext);
            return true;
        }
        if let Action::PaletteNext | Action::PalettePrev = action {
            if self.palette_live && matches!(self.phase, Phase::Playing { .. }) {
                let to = match action {
                    Action::PalettePrev => self.state.gb_palette.prev(),
                    _ => self.state.gb_palette.next(),
                };
                self.state.gb_palette = to;
                self.palette_pending = Some(to);
                self.persist();
                self.hud.toast(Toast::Palette(to), self.now());
            }
            return true;
        }
        if action == Action::ColourCorrectionToggle {
            self.change_setting(QuickRow::ColourCorrection, true);
            let said = match self.state.colour_correction {
                true => Toast::ColourOn,
                false => Toast::ColourOff,
            };
            self.hud.toast(said, self.now());
            return true;
        }
        let s = &self.state;
        let (kind, value) = match action {
            Action::BrightnessUp => (HudKind::Brightness, up(s.brightness, 1, BRIGHTNESS_MAX)),
            Action::BrightnessDown => (HudKind::Brightness, s.brightness.saturating_sub(1)),
            Action::BlueLightUp => (HudKind::BlueLight, up(s.blue_light, 1, BLUE_LIGHT_MAX)),
            Action::BlueLightDown => (HudKind::BlueLight, s.blue_light.saturating_sub(1)),
            Action::VolumeUp => (HudKind::Volume, up(self.level(), VOLUME_STEP, VOLUME_MAX)),
            Action::VolumeDown => (HudKind::Volume, self.level().saturating_sub(VOLUME_STEP)),
            _ => return false,
        };
        if kind == HudKind::Volume {
            self.remember_volume();
        }
        let level = match kind {
            HudKind::Brightness => &mut self.state.brightness,
            HudKind::BlueLight => &mut self.state.blue_light,
            HudKind::Volume if self.headphones => &mut self.state.volume_hp,
            HudKind::Volume => &mut self.state.volume,
            HudKind::Rewind => return false,
        };
        let moved = *level != value;
        *level = value;
        let unmuted = kind == HudKind::Volume && std::mem::take(self.muted_mut());
        let (shown, now) = (self.hud_value(kind, value), self.now());
        self.hud.show(kind, shown, self.muted(), now);
        if let (HudKind::Brightness, Some(power)) = (kind, &mut self.power) {
            power.set_backlight(value);
        }
        if moved || unmuted {
            self.persist_soon();
        }
        true
    }

    fn remember_volume(&mut self) {
        if self.vol_before.len() == 2 {
            self.vol_before.remove(0);
        }
        self.vol_before
            .push((self.level(), self.muted(), self.now()));
    }

    fn mute_toggle(&mut self) {
        let now = self.now();
        if let Some((volume, muted, _)) = self
            .vol_before
            .iter()
            .find(|(_, _, at)| now.saturating_sub(*at) <= MUTE_CHORD_MS)
            .copied()
        {
            *self.level_mut() = volume;
            *self.muted_mut() = muted;
        }
        self.vol_before.clear();
        let muted = !self.muted();
        *self.muted_mut() = muted;
        self.hud
            .show(HudKind::Volume, self.output_volume(), muted, now);
        self.persist_soon();
    }

    fn hud_value(&self, kind: HudKind, value: u8) -> u8 {
        match kind {
            HudKind::Volume => self.output_volume(),
            _ => value,
        }
    }

    pub fn show_rewind(&mut self, fill: u8) {
        let now = self.now();
        self.hud.show(HudKind::Rewind, fill, false, now);
    }

    pub fn hide_rewind(&mut self) {
        self.hud.release_rewind();
    }

    pub fn set_ff(&mut self, ff: FfState) {
        self.hud.set_ff(ff);
    }

    pub fn ff_badge(&self) -> Option<Icon> {
        self.hud.badge()
    }

    pub fn blue_light(&self) -> u8 {
        self.state.blue_light
    }

    pub fn brightness(&self) -> u8 {
        self.state.brightness
    }

    pub fn screen_off(&self) -> bool {
        self.screen_off
    }

    pub fn restore_screen(&mut self) {
        self.screen_off = false;
        if let Some(power) = &mut self.power {
            power.set_screen_off(false);
        }
    }

    pub fn hdmi_scaling(&self) -> (slot_store::HdmiScale, u8) {
        (self.state.hdmi_scale, self.state.hdmi_margin)
    }

    pub fn audio_output(&self) -> slot_store::AudioOutput {
        self.state.audio_output
    }

    pub fn set_hdmi_audio(&mut self, active: bool) {
        self.hdmi_audio = active;
        self.hud.set_headphones(self.headphones_active());
    }

    /// Only a successfully activated external display grants the lid exception.
    pub fn set_docked(&mut self, docked: bool) {
        self.docked = docked;
        self.sync_lid_policy();
    }

    fn sync_lid_policy(&mut self) {
        if !self.lid_closed {
            return;
        }
        if (self.docked && self.state.docked_lid)
            || (self.music.snapshot.playing
                && self.music.settings.keep_playing_closed
                && !self.shutting_down())
        {
            // Docking must not undo a deliberate sleep from the power button.
            if self.lid_sleeping {
                self.wake();
                self.lid_sleeping = false;
            }
            if let Some(power) = &mut self.power {
                power.on_close();
            }
        } else if !matches!(self.phase, Phase::Doze { .. }) {
            self.doze();
            self.lid_sleeping = true;
        }
    }

    fn close_lid(&mut self) {
        self.lid_closed = true;
        self.sync_lid_policy();
    }

    fn open_lid(&mut self) {
        self.lid_closed = false;
        self.restore_screen();
        if self.lid_sleeping || !(self.docked && self.state.docked_lid) {
            self.wake();
        }
        self.lid_sleeping = false;
        if !matches!(self.phase, Phase::Doze { .. }) {
            if let Some(power) = &mut self.power {
                power.on_open();
            }
        }
    }

    pub fn music_visible(&self) -> bool {
        matches!(self.phase, Phase::Shelf)
            && !self.screen_off
            && self.shelf_kind == ShelfKind::Music
            && (!self.lid_closed || self.docked)
            && self.power_menu.is_none()
    }
    pub fn headphones_active(&self) -> bool {
        self.bluetooth_audio || (self.headphones && !self.hdmi_audio)
    }
    pub fn lid_closed(&self) -> bool {
        self.lid_closed
    }
    pub fn music_display_can_resume(&self) -> bool {
        self.shelf_kind == ShelfKind::Music
            && (matches!(self.phase, Phase::Shelf)
                || (self.lid_sleeping
                    && ((self.docked && self.state.docked_lid)
                        || self.music.settings.keep_playing_closed)))
    }
    pub fn sync_music_lid(&mut self) {
        self.sync_lid_policy();
    }

    pub fn volume(&self) -> u8 {
        self.level()
    }

    fn level(&self) -> u8 {
        if self.headphones_active() {
            self.state.volume_hp
        } else {
            self.state.volume
        }
    }

    fn level_mut(&mut self) -> &mut u8 {
        if self.headphones_active() {
            &mut self.state.volume_hp
        } else {
            &mut self.state.volume
        }
    }

    pub fn muted(&self) -> bool {
        if self.headphones_active() {
            self.state.muted_hp
        } else {
            self.state.muted
        }
    }

    fn muted_mut(&mut self) -> &mut bool {
        if self.headphones_active() {
            &mut self.state.muted_hp
        } else {
            &mut self.state.muted
        }
    }

    pub fn output_volume(&self) -> u8 {
        if self.muted() {
            0
        } else {
            self.level()
        }
    }

    pub fn hud_icon(&self) -> Icon {
        self.hud.glyph()
    }

    pub fn now(&self) -> Millis {
        self.clock as Millis
    }

    fn flick(&mut self, step: fn(&mut Polaroids)) {
        if let Some(p) = &mut self.polaroids {
            step(p);
        }
    }

    pub fn update(&mut self, dt: f32) {
        if matches!(self.phase, Phase::Shelf) && self.shelf_kind == ShelfKind::Music {
            self.music.update(dt, self.now());
        } else {
            self.music.release();
        }
        self.clock += dt as f64 * 1000.0;
        if self.slot_letter.is_some()
            && self.shelf_named.is_some_and(|at| {
                self.now().saturating_sub(at)
                    >= SLOT_NAME_IN_MS + SLOT_NAME_HOLD_MS + SLOT_NAME_OUT_MS
            })
        {
            self.slot_letter = None;
            self.shelf_named = None;
            self.clear_shelf_platform();
        }
        if self.name_pending && self.shelf_platform.face.is_some() {
            self.name_pending = false;
            self.shelf_named = Some(self.now());
        }
        self.timers();
        self.poll_link();
        let now = self.now();
        let ready = self.core_faces_ready();
        if let Some(picker) = &mut self.core_picker {
            if picker.waiting() && (ready || picker.waited(now) >= FACES_WAIT_MS) {
                picker.start(now);
            }
        }
        if self.core_picker.is_some_and(|p| p.finished(now)) {
            self.core_picker = None;
        }
        if !self.on_shelf() {
            self.shelf_mut().release_hold();
        }
        let mut touched = false;
        let at = self.shelf_at;
        let seated_at = seated_at(self.shelves[self.shelf_at].0);
        let next = match &mut self.phase {
            Phase::Shelf => {
                let shelf = &mut self.shelves[at].1;
                shelf.tick(now);
                shelf.update(dt);
                None
            }
            Phase::Inserting {
                cart,
                t,
                core_ready,
                resumed,
                ..
            } => {
                let was = *t;
                *t += dt;
                let at = seated_at - Sfx::Insert.lead();
                touched = !*resumed && was < at && *t >= at;
                (*t >= seated_at + INSERT_HOLD_S && *core_ready).then(|| Phase::Playing {
                    cart: std::mem::take(cart),
                })
            }
            Phase::Ejecting { t, .. } => {
                if self.screen <= 0.0 {
                    let was = *t;
                    *t += dt;
                    touched = was < 0.0 && *t >= 0.0;
                }
                (*t >= seated_at).then_some(Phase::Shelf)
            }
            _ => None,
        };
        if touched {
            self.sfx = Some(match self.phase {
                Phase::Ejecting { .. } => Sfx::Eject,
                _ => Sfx::Insert,
            });
        }
        if let Some(phase) = next {
            self.phase = phase;
            self.refused_from = None;
            let seated = match &self.phase {
                Phase::Playing { cart } => Some(cart.clone()),
                _ => None,
            };
            if seated.is_none() {
                self.link_loaded = None;
            }
            self.record_cart(seated);
        }
        self.step_screen(dt);
    }

    fn step_screen(&mut self, dt: f32) {
        let lit = matches!(self.phase, Phase::Playing { .. } | Phase::Polaroids { .. });
        let step = if lit {
            dt / POWER_ON_S
        } else {
            -dt / POWER_OFF_S
        };
        self.screen = (self.screen + step).clamp(0.0, 1.0);
        let docked = self.power_menu.is_none()
            && matches!(
                self.phase,
                Phase::Shelf | Phase::Inserting { .. } | Phase::Ejecting { .. }
            );
        let sliding = self.power_menu.is_none()
            && matches!(
                self.phase,
                Phase::Inserting { .. }
                    | Phase::Playing { .. }
                    | Phase::Polaroids { .. }
                    | Phase::Ejecting { .. }
            );
        let reach = if sliding { dt / POWER_ON_S } else { 1.0 };
        self.lift = match docked {
            true => (self.lift - reach).max(0.0),
            false => (self.lift + reach).min(1.0),
        };
    }

    pub fn frame_split(&self) -> Option<f32> {
        match (&self.phase, self.power_menu) {
            (Phase::QuickMenu { .. }, None) => Some(slot_ui::QUICK_SPLIT),
            _ => None,
        }
    }

    pub fn frame_lift(&self) -> f32 {
        self.lift
    }

    pub fn screen_power(&self) -> f32 {
        self.screen
    }

    pub fn set_game_ready(&mut self, ready: bool) {
        self.game_ready = ready;
    }

    pub fn game_visible(&self) -> bool {
        self.game_ready && self.screen > 0.0
    }

    pub fn tick_ms(&mut self, now: Millis) {
        self.clock = self.clock.max(now as f64);
        self.timers();
    }

    fn timers(&mut self) {
        self.bluetooth.poll();
        self.play_hold();
        let offer = self.undo_label();
        if let Some(p) = &mut self.polaroids {
            p.set_undo(offer);
        }
        if self.now() >= self.autosave_at {
            self.autosave();
        }
        if self.now() >= self.battery_at {
            self.battery_at = self.now() + BATTERY_POLL_MS;
            self.battery = self.power.as_ref().and_then(|p| p.battery());
        }
        if self.now() >= self.headphones_at {
            self.headphones_at = self.now() + HEADPHONES_POLL_MS;
            let on = self.power.as_ref().is_some_and(|p| p.headphones());
            if on != self.headphones {
                self.headphones = on;
                self.hud.set_headphones(self.headphones_active());
                let now = self.now();
                self.hud
                    .show(HudKind::Volume, self.output_volume(), self.muted(), now);
            }
        }
        if self.now() >= self.charge_at {
            self.charge_at = self.now() + CHARGE_POLL_MS;
            if let (Some(power), Some(b)) = (self.power.as_ref(), self.battery.as_mut()) {
                b.charge = power.charge();
            }
            let state = self.led_state();
            self.set_led(state);
        }
        // Read charging before deciding whether standby may shut down. A cable
        // connected at the deadline must win over the cached previous status.
        if let Some(b) = self.battery {
            self.on_battery(b);
        }
        if matches!(self.phase, Phase::Doze { .. })
            && !self.state.shutdown_charging
            && self
                .battery
                .is_some_and(|b| matches!(b.charge, Charge::Charging | Charge::Full))
        {
            self.dozed_at = self.now();
        }
        if self.doze_expired() {
            self.on_doze_timeout();
        }
        if let Some(GameMenu::Linked {
            since,
            opened: false,
            ..
        }) = self.game_menu
        {
            if self.now().saturating_sub(since) >= LINKED_HOLD_MS {
                self.game_menu = None;
            }
        }
        if let Some(GameMenu::Unplug { since, .. }) = self.game_menu {
            if self.now().saturating_sub(since) >= UNPLUG_HOLD_MS {
                self.game_menu = None;
            }
        }
        if let Some(at) = self.link.as_ref().and_then(|s| s.lost_at) {
            if self.now().saturating_sub(at) >= LINK_LOST_MS {
                self.end_link();
            }
        }
    }

    pub fn led_state(&self) -> LedState {
        let Some(b) = self.battery else {
            return LedState::Running;
        };
        match b.charge {
            Charge::Charging => LedState::Charging,
            Charge::Full => LedState::Charged,
            _ if b.percent <= BATTERY_LOW => LedState::Low,
            _ => LedState::Running,
        }
    }

    fn set_led(&mut self, state: LedState) {
        if self.shutting_down() && state != LedState::Off {
            return;
        }
        if self.last_led == Some(state) {
            return;
        }
        self.last_led = Some(state);
        if let Some(power) = self.power.as_mut() {
            power.set_led(state);
        }
    }

    fn record_cart(&mut self, cart: Option<String>) {
        let platform = self.seated_cart().map(|c| c.platform);
        if self.state.cart == cart && self.state.cart_platform == platform {
            return;
        }
        self.state.cart = cart;
        self.state.cart_platform = platform;
        self.persist();
    }

    fn persist(&self) {
        let Some(root) = &self.root else {
            return;
        };
        self.state_writer.flush();
        if let Err(e) = write_slot_state(root, &self.state) {
            eprintln!("slot: slot.state: {e}");
        }
    }

    fn persist_soon(&self) {
        if let Some(root) = &self.root {
            self.state_writer.save(root, &self.state);
        }
    }

    pub fn flush_state(&self) {
        self.state_writer.flush();
    }

    pub fn on_core_ready(&mut self) {
        let Phase::Inserting {
            cart, core_ready, ..
        } = &mut self.phase
        else {
            return;
        };
        *core_ready = true;
        let cart = cart.clone();
        self.retire_refused_resume(&cart);
    }

    fn retire_refused_resume(&mut self, stem: &str) {
        let Some(snapshot) = &self.snapshot else {
            return;
        };
        if snapshot.resume_trusted() {
            return;
        }
        if !self.named_core {
            return;
        }
        let Some(root) = &self.root else {
            return;
        };
        if let Some(h) = self.pending_save.take() {
            let _ = h.join();
        }
        let ring = StateRing::new(root, self.platform, self.core, stem);
        match ring.retire_resume(&format_stamp(self.wall_secs())) {
            Ok(Some(to)) => eprintln!(
                "slot: resume: {} refused this state, moved it to {}",
                self.core.as_str(),
                to.display()
            ),
            Ok(None) => {}
            Err(e) => eprintln!("slot: resume: could not move the refused state aside: {e}"),
        }
    }

    pub fn on_core_failed(&mut self) {
        let caught = self.seat();
        let Phase::Inserting { cart, .. } = &mut self.phase else {
            return;
        };
        let cart = std::mem::take(cart);
        self.refuse_out(cart, caught);
    }

    fn refuse_out(&mut self, cart: String, caught: f32) {
        let t = (1.0 - caught) * seated_at(self.shelves[self.shelf_at].0);
        self.phase = Phase::Ejecting { cart, t };
        self.refused_from = Some(t);
    }

    pub fn refuse(&mut self) {
        self.refusal = Some(Refusal::started(self.now()));
    }

    pub fn refusal_active(&self, now: Millis) -> bool {
        self.refusal.is_some_and(|r| r.active(now))
    }

    pub fn seat(&self) -> f32 {
        match &self.phase {
            Phase::Shelf => 0.0,
            Phase::Inserting { t, resumed, .. } => {
                if *resumed {
                    1.0
                } else {
                    (t / seated_at(self.shelves[self.shelf_at].0)).clamp(0.0, 1.0)
                }
            }
            Phase::Ejecting { t, .. } => {
                1.0 - (t / seated_at(self.shelves[self.shelf_at].0)).clamp(0.0, 1.0)
            }
            _ => 1.0,
        }
    }

    pub fn draw(&self, out: &mut Vec<Draw>) {
        if self.screen_off {
            out.push(Draw::Rect {
                x: 0.0,
                y: 0.0,
                w: OUT_W as f32,
                h: OUT_H as f32,
                colour: [0.0, 0.0, 0.0, 1.0],
            });
            return;
        }
        if let Some(index) = self.power_menu {
            self.draw_power_menu(index, out);
            return;
        }
        if let Some(index) = self.state_menu.filter(|_| self.playback.is_none()) {
            out.push(Draw::Rect {
                x: 0.0,
                y: 0.0,
                w: OUT_W as f32,
                h: OUT_H as f32,
                colour: slot_ui::opening(),
            });
            if let Some((mode, index)) = self.state_slot_menu {
                if let Some(&(tex, w, h)) =
                    self.state_faces
                        .get(if mode == StateSlotMode::Save { 0 } else { 1 })
                {
                    out.push(Draw::Tex {
                        x: (OUT_W as f32 - w as f32) / 2.0,
                        y: 56.0,
                        w: w as f32,
                        h: h as f32,
                        tex,
                        alpha: 1.0,
                    });
                }
                draw_menu_rows(
                    &self.state_slot_faces,
                    Some(index),
                    centred_top(self.state_slot_faces.len()),
                    out,
                );
                if let Some((tex, w, h)) = self.state_slot_undo_face {
                    out.push(Draw::Tex {
                        x: (OUT_W as f32 - w as f32) / 2.0,
                        y: OUT_H as f32 - 60.0,
                        w: w as f32,
                        h: h as f32,
                        tex,
                        alpha: 1.0,
                    });
                }
                return;
            }
            if self.players_open {
                if let Some(tex) = self.players_face {
                    out.push(Draw::Tex {
                        x: 0.0,
                        y: 0.0,
                        w: OUT_W as f32,
                        h: OUT_H as f32,
                        tex,
                        alpha: 1.0,
                    });
                }
                return;
            }
            let rows: Vec<_> = match (
                self.state_faces.as_slice(),
                self.run_ahead_faces.get(usize::from(self.run_ahead)),
            ) {
                ([save, load, back, players, rewind, fast, reset], Some(run_ahead)) => {
                    vec![
                        *save, *load, *run_ahead, *players, *rewind, *fast, *reset, *back,
                    ]
                }
                _ => Vec::new(),
            };
            draw_menu_rows(&rows, Some(index), centred_top(rows.len()), out);
            return;
        }
        if let Some(index) = self.disc_menu {
            out.push(Draw::Rect {
                x: 0.0,
                y: 0.0,
                w: OUT_W as f32,
                h: OUT_H as f32,
                colour: slot_ui::opening(),
            });
            draw_menu_rows(
                &self.disc_faces,
                Some(index),
                centred_top(self.disc_faces.len()),
                out,
            );
            if let Some((tex, w, h)) = self.disc_title {
                out.push(Draw::Tex {
                    x: (OUT_W as f32 - w as f32) / 2.0,
                    y: 60.0,
                    w: w as f32,
                    h: h as f32,
                    tex,
                    alpha: 1.0,
                });
            }
            return;
        }
        if self.shutting_down() {
            out.push(Draw::Rect {
                x: 0.0,
                y: 0.0,
                w: OUT_W as f32,
                h: OUT_H as f32,
                colour: [0.0, 0.0, 0.0, 1.0],
            });
            let which = if self.restarting {
                PowerChoice::Restart
            } else {
                PowerChoice::PowerOff
            };
            if let Some((tex, w, h)) = self.shutdown_faces.get(which.index()).copied() {
                out.push(Draw::Tex {
                    x: ((OUT_W - w) / 2) as f32,
                    y: ((OUT_H - h) / 2) as f32,
                    w: w as f32,
                    h: h as f32,
                    tex,
                    alpha: 1.0,
                });
            }
            return;
        }
        match &self.phase {
            Phase::SetClock {
                picker, from_menu, ..
            } => {
                let (line, hint) = match self.clock_faces {
                    Some((line, hint)) => (Some(line), Some(hint)),
                    None => (None, None),
                };
                let back = self
                    .quick_menu_faces
                    .as_ref()
                    .filter(|_| *from_menu)
                    .map(|f| f.legend[0]);
                picker.draw(line, hint, back, out);
                return;
            }
            Phase::QuickMenu { row } => QuickMenu {
                row: *row,
                values: QuickRow::ALL.map(|r| self.quick_value(r)),
                clock: self.quick_clock_faces,
                controller: self.quick_controller_faces,
                bluetooth: self.quick_bluetooth_faces,
                battery: self.quick_battery_faces,
                faces: self.quick_menu_faces.as_ref(),
            }
            .draw(out),
            Phase::Shelf => {
                draw_backdrop(self.wallpaper, out);
                if self.shelf_kind == ShelfKind::Music {
                    self.music.draw(self.now(), out);
                    if !self.music.flipped && !self.music.fullscreen {
                        self.draw_shelf_switch_hints(out);
                    }
                    self.hud.draw(self.now(), out);
                    return;
                }
                match (self.core_picker_shown(), self.selected_stem()) {
                    (Some(picker), Some(stem)) => {
                        let open = ease(picker.openness(self.now()));
                        let dim = 1.0 + (CORE_PICKER_DIM - 1.0) * open;
                        self.shelf()
                            .draw_row(Some(stem), 0.0, CORE_PICKER_RECEDE * open, dim, out);
                        draw_empty_slot(out);
                    }
                    _ => {
                        self.shelf().draw(self.shelf_shake(), out);
                        draw_slot_name(self.shelf_platform, self.slot_name_alpha(), out);
                    }
                }
                draw_footer(
                    self.battery,
                    self.battery_percent,
                    self.bolt,
                    self.shelf_clock,
                    out,
                );
                if self.core_picker.is_none() && self.game_menu.is_none() {
                    self.draw_shelf_switch_hints(out);
                }
            }
            Phase::About => {
                draw_backdrop(self.wallpaper, out);
                draw_sticker(self.sticker_face, out);
                return;
            }
            Phase::Players => {
                draw_backdrop(self.wallpaper, out);
                out.push(Draw::Rect {
                    x: 0.0,
                    y: 0.0,
                    w: OUT_W as f32,
                    h: OUT_H as f32,
                    colour: slot_ui::opening(),
                });
                if let Some(tex) = self.players_face {
                    out.push(Draw::Tex {
                        x: 0.0,
                        y: 0.0,
                        w: OUT_W as f32,
                        h: OUT_H as f32,
                        tex,
                        alpha: 1.0,
                    });
                }
                return;
            }
            Phase::Inserting { cart, resumed, .. } => {
                if !resumed {
                    draw_backdrop(self.wallpaper, out);
                    self.shelf()
                        .draw_row(Some(cart), 0.0, self.seat(), 1.0, out);
                }
                self.chrome(cart, self.seat(), out);
            }
            Phase::Ejecting { cart, .. } => {
                draw_backdrop(self.wallpaper, out);
                self.shelf()
                    .draw_row(Some(cart), 0.0, self.seat(), 1.0, out);
                self.chrome(cart, self.seat(), out);
            }
            Phase::Playing { cart } if self.screen < 1.0 => self.chrome(cart, 0.0, out),
            Phase::Playing { .. } => self.push_game(out),
            Phase::Polaroids { .. } => {
                self.push_game(out);
                if let Some(p) = &self.polaroids {
                    p.draw(
                        self.battery,
                        self.battery_percent,
                        self.bolt,
                        self.shelf_clock,
                        out,
                    );
                }
            }
            Phase::Doze { .. } => {
                out.push(Draw::Rect {
                    x: 0.0,
                    y: 0.0,
                    w: OUT_W as f32,
                    h: OUT_H as f32,
                    colour: [0.0, 0.0, 0.0, 1.0],
                });
                return;
            }
        }
        if let Some(picker) = self.core_picker_shown() {
            self.draw_core_picker(&picker, out);
        }
        if let Some(menu) = self.game_menu {
            self.draw_game_menu(menu, out);
        }
        self.hud.draw(self.now(), out);
    }

    fn draw_shelf_switch_hints(&self, out: &mut Vec<Draw>) {
        for (i, hint) in self.shelf_switch_hints.iter().enumerate() {
            if let Some(tex) = hint.face {
                out.push(Draw::Tex {
                    x: if i == 0 {
                        24.0
                    } else {
                        OUT_W as f32 - 24.0 - hint.w as f32
                    },
                    y: 24.0,
                    w: hint.w as f32,
                    h: HINT_H as f32,
                    tex,
                    alpha: 1.0,
                });
            }
        }
    }

    pub fn screen_shake(&self) -> f32 {
        self.shake_at(self.now())
    }

    pub fn shake_at(&self, now: Millis) -> f32 {
        self.shake_when(matches!(self.phase, Phase::Playing { .. }), now)
    }

    pub fn shelf_shake(&self) -> f32 {
        self.shake_when(self.on_shelf() && self.core_picker.is_none(), self.now())
    }

    fn shake_when(&self, mine: bool, now: Millis) -> f32 {
        if !mine {
            return 0.0;
        }
        self.refusal.map_or(0.0, |r| r.offset(now))
    }

    fn push_game(&self, out: &mut Vec<Draw>) {
        if self.game_visible() {
            out.push(Draw::Game);
        }
    }

    fn chrome(&self, stem: &str, dim: f32, out: &mut Vec<Draw>) {
        let Some((cart, face)) = self.shelf().find(stem) else {
            return;
        };
        let alpha = self.alert_alpha();
        let (rest, scale) = self.shelf().selected_at();
        SlotChrome {
            cart,
            face,
            rest,
            scale,
            seat: self.seat(),
            alert: self.alert_face.filter(|_| alpha > 0.0).map(|t| (t, alpha)),
            dim,
            screen: self.screen,
            game: self.game_ready,
        }
        .draw_with_media(self.shelf().selected_yaw(), self.psx_textures, out);
    }

    pub fn alert_visible(&self) -> bool {
        self.alert_alpha() > 0.0
    }

    pub fn alert_alpha(&self) -> f32 {
        let (Some(from), Phase::Ejecting { t, .. }) = (self.refused_from, &self.phase) else {
            return 0.0;
        };
        let span = seated_at(self.shelves[self.shelf_at].0) - from;
        if span <= 0.0 {
            return 0.0;
        }
        let u = ((t - from) / span).clamp(0.0, 1.0);
        ((ALERT_GONE - u) / (ALERT_GONE - ALERT_HOLD)).clamp(0.0, 1.0)
    }

    pub fn set_alert_face(&mut self, face: TexId) {
        self.alert_face = Some(face);
    }

    pub fn set_psx_textures(&mut self, parts: PsxTextures) {
        self.psx_textures = Some(parts);
    }

    pub fn set_shutdown_faces(&mut self, faces: Vec<(TexId, u32, u32)>) {
        self.shutdown_faces = faces;
    }

    pub fn set_power_menu_faces(&mut self, faces: Vec<(TexId, u32, u32)>) {
        self.power_menu_faces = faces;
    }

    pub fn set_core_board_faces(&mut self, board: TexId, lid: TexId) {
        self.core_board_face = Some(board);
        self.core_lid_face = Some(lid);
        self.core_faces_stem = self.selected_stem().map(str::to_string);
    }

    pub fn set_core_part_faces(
        &mut self,
        sockets: Vec<TexId>,
        chips: Vec<TexId>,
        blank: TexId,
        shadow: TexId,
    ) {
        self.core_socket_faces = sockets;
        self.core_chip_faces = chips;
        self.core_blank_chip_face = Some(blank);
        self.core_chip_shadow_face = Some(shadow);
    }

    pub fn set_core_legend_faces(&mut self, faces: Vec<(TexId, u32)>) {
        self.core_legend_faces = faces;
    }

    pub fn set_link_menu_faces(&mut self, faces: Vec<(TexId, u32, u32)>) {
        self.link_menu_faces = faces;
    }

    pub fn set_link_linked_face(&mut self, face: (TexId, u32, u32)) {
        self.link_linked_face = Some(face);
    }

    pub fn set_link_step_faces(&mut self, faces: Vec<(TexId, u32, u32)>) {
        self.link_step_faces = faces;
    }

    pub fn set_link_fail_faces(&mut self, faces: Vec<(TexId, u32, u32)>) {
        self.link_fail_faces = faces;
    }

    pub fn set_link_legend_faces(&mut self, faces: Vec<(TexId, u32)>) {
        self.link_legend_faces = faces;
    }

    pub fn set_link_sprites(&mut self, sprites: LinkSprites) {
        self.link_sprites = Some(sprites);
    }

    pub fn link_sprites_ready(&self) -> bool {
        self.link_sprites.is_some()
    }

    fn draw_power_menu(&self, index: usize, out: &mut Vec<Draw>) {
        out.push(Draw::Rect {
            x: 0.0,
            y: 0.0,
            w: OUT_W as f32,
            h: OUT_H as f32,
            colour: slot_ui::opening(),
        });
        draw_menu_rows(
            &self.power_menu_faces,
            Some(index),
            centred_top(self.power_menu_faces.len()),
            out,
        );
    }

    fn core_picker_shown(&self) -> Option<CorePicker> {
        self.core_picker.filter(|p| !p.waiting())
    }

    fn draw_core_picker(&self, picker: &CorePicker, out: &mut Vec<Draw>) {
        let now = self.now();
        let progress = picker.openness(now);
        let lift = lift_of(progress);
        let (rest, scale) = self.shelf().selected_at();
        let shelf = shelf_cart_at(rest, scale);
        let board = board_from(shelf, progress);
        let zoom = board_zoom(board);
        let ready = self.core_faces_ready();

        if ready {
            if let Some(tex) = self.core_board_face {
                out.push(Draw::Tex {
                    x: board.x,
                    y: board.y,
                    w: board.w,
                    h: board.h,
                    tex,
                    alpha: 1.0,
                });
            }
            for (i, tex) in self.core_socket_faces.iter().copied().enumerate() {
                let (x, y) = on_board(board, SOCKET_U[i], SOCKET_V);
                out.push(Draw::Tex {
                    x: x.round(),
                    y: y.round(),
                    w: SOCKET_W as f32 * zoom,
                    h: SOCKET_H as f32 * zoom,
                    tex,
                    alpha: 1.0,
                });
            }

            let chip = picker.chip(now);
            let u = CHIP_U[0] + (CHIP_U[1] - CHIP_U[0]) * chip.across;
            if chip.lift > 0.0 {
                if let Some(tex) = self.core_chip_shadow_face {
                    let (cx, cy) = on_board(board, u + 19.0, CHIP_V + 29.4);
                    let (w, h) = (SHADOW_W as f32 * zoom, SHADOW_H as f32 * zoom);
                    out.push(Draw::Tex {
                        x: cx - w / 2.0,
                        y: cy - h / 2.0,
                        w,
                        h,
                        tex,
                        alpha: 0.6 * chip.lift * lift,
                    });
                }
            }
            let face = match chip.seated {
                Some(core) => self.core_chip_faces.get(core.index()).copied(),
                None => self.core_blank_chip_face,
            };
            if let Some(tex) = face {
                let (x, y) = on_board(board, u, CHIP_V - HOP_LIFT * chip.lift);
                let body = Placed {
                    x: x + chip.shake,
                    y,
                    w: CHIP_W as f32 * zoom,
                    h: CHIP_H as f32 * zoom,
                };
                let at = grown(body, TURN_PAD as f32 * zoom);
                out.push(Draw::Turned {
                    x: at.x.round(),
                    y: at.y.round(),
                    w: at.w,
                    h: at.h,
                    tex,
                    alpha: 1.0,
                    turn: chip.tip,
                });
            }
        }

        if let Some(tex) = self.core_chip_shadow_face {
            let (lid, _) = lid_from(shelf, progress);
            let k = lid.w / lid_at(1.0).0.w;
            let (w, h) = (LID_SHADOW_W * k, LID_SHADOW_H * k);
            out.push(Draw::Tex {
                x: lid.x + (lid.w - w) / 2.0,
                y: lid.y + lid.h + LID_SHADOW_DROP * k - h / 2.0,
                w,
                h,
                tex,
                alpha: LID_SHADOW_ALPHA * lift,
            });
        }

        if ready {
            if let Some(tex) = self.core_lid_face {
                let (lid, turn) = lid_from(shelf, progress);
                let at = grown(lid, TURN_PAD as f32 * lid.w / CART_W as f32);
                out.push(Draw::Turned {
                    x: at.x,
                    y: at.y,
                    w: at.w,
                    h: at.h,
                    tex,
                    alpha: 1.0,
                    turn,
                });
            }
        } else if let Some((_, Some(tex))) = self
            .selected_stem()
            .and_then(|stem| self.shelf().find(stem))
        {
            let (lid, turn) = lid_from(shelf, progress);
            out.push(Draw::Turned {
                x: lid.x,
                y: lid.y,
                w: lid.w,
                h: lid.h,
                tex,
                alpha: 1.0,
                turn,
            });
        }

        if let [cancel, swap, choose] = self.core_legend_faces.as_slice() {
            let right = BOARD_X + BOARD_W as f32;
            let seen = |w: u32| w.saturating_sub(HINT_EDGE) as f32;
            for (tex, w, x) in [
                (cancel.0, cancel.1, BOARD_X),
                (swap.0, swap.1, (OUT_W as f32 - seen(swap.1)) / 2.0),
                (choose.0, choose.1, right - seen(choose.1)),
            ] {
                out.push(Draw::Tex {
                    x: x.round(),
                    y: CORE_LEGEND_Y,
                    w: w as f32,
                    h: HINT_H as f32,
                    tex,
                    alpha: lift,
                });
            }
        }
    }

    fn draw_game_menu(&self, menu: GameMenu, out: &mut Vec<Draw>) {
        out.push(Draw::Rect {
            x: 0.0,
            y: 0.0,
            w: OUT_W as f32,
            h: OUT_H as f32,
            colour: slot_ui::opening(),
        });
        if let Some(sprites) = &self.link_sprites {
            crate::link_screen::draw_link_art(menu, self.link_hardware, self.now(), sprites, out);
        }
        let line = match menu {
            GameMenu::Pick(role) => self.link_menu_faces.get(role.index()).copied(),
            GameMenu::Working { step, .. } => self
                .link_step_faces
                .get(step.shown(self.radio.warmed()).index())
                .copied(),
            GameMenu::Linked { .. } => self.link_linked_face,
            GameMenu::Failed { fail, .. } => fail
                .shown()
                .and_then(|i| self.link_fail_faces.get(i).copied()),
            GameMenu::Unplug { .. } => None,
        };
        if let Some((tex, w, h)) = line {
            out.push(Draw::Tex {
                x: ((OUT_W as f32 - w as f32) / 2.0).round(),
                y: LINK_TEXT_Y,
                w: w as f32,
                h: h as f32,
                tex,
                alpha: 1.0,
            });
        }
        let switchable = self.seated().is_some_and(|stem| self.link_switchable(stem));
        let keys: &[LinkLegend] = match menu {
            GameMenu::Pick(_) if switchable => &[
                LinkLegend::Cancel,
                LinkLegend::Mode,
                LinkLegend::Swap,
                LinkLegend::Link,
            ],
            GameMenu::Pick(_) => &[LinkLegend::Cancel, LinkLegend::Swap, LinkLegend::Link],
            GameMenu::Working { .. } => &[LinkLegend::Cancel],
            GameMenu::Linked { opened: true, .. } => &[LinkLegend::Back, LinkLegend::EndLink],
            GameMenu::Linked { .. } => &[],
            GameMenu::Failed { .. } => &[LinkLegend::Ok],
            GameMenu::Unplug { .. } => &[],
        };
        let faces: Vec<(TexId, u32)> = keys
            .iter()
            .filter_map(|k| self.link_legend_faces.get(k.index()).copied())
            .collect();
        let seen = |w: u32| w.saturating_sub(HINT_EDGE) as f32;
        let total: f32 = faces.iter().map(|(_, w)| seen(*w)).sum::<f32>()
            + LINK_LEGEND_GAP * faces.len().saturating_sub(1) as f32;
        let mut x = ((OUT_W as f32 - total) / 2.0).round();
        for (tex, w) in faces {
            out.push(Draw::Tex {
                x,
                y: LINK_LEGEND_Y,
                w: w as f32,
                h: HINT_H as f32,
                tex,
                alpha: 1.0,
            });
            x += (seen(w) + LINK_LEGEND_GAP).round();
        }
    }

    fn auto_link(&self, stem: &str) -> Option<(&Cart, LinkKind)> {
        let cart = self.shelf().carts.iter().find(|c| c.stem == stem)?;
        let auto = link_kind(&cart.code, &cart.title, slot_store::header_clean(&cart.rom));
        Some((cart, auto))
    }

    fn on_shelf(&self) -> bool {
        matches!(self.phase, Phase::Shelf) && self.shelf_kind != ShelfKind::Music
    }

    fn insert(&mut self, clean: bool) {
        if !self.on_shelf() {
            return;
        }
        let at = self.shelf().index;
        let Some(cart) = self.shelf_mut().carts.get_mut(at).map(|c| {
            c.read_header();
            c.stem.clone()
        }) else {
            return;
        };
        self.battery_saved = None;
        self.battery_retry_at = 0;
        self.play_held = None;
        self.refusal = None;
        self.refused_from = None;
        self.phase = Phase::Inserting {
            cart,
            t: 0.0,
            core_ready: false,
            resumed: false,
            clean,
        };
    }

    pub fn starting_clean(&self) -> bool {
        matches!(self.phase, Phase::Inserting { clean: true, .. })
    }

    fn play_hold(&mut self) {
        let Some(at) = self.play_held else {
            return;
        };
        if !self.on_shelf() {
            self.play_held = None;
            return;
        }
        if self.now().saturating_sub(at) >= PLAY_HOLD_MS {
            self.insert(true);
        }
    }

    fn eject(&mut self) {
        if self.single_cart() {
            return self.refuse();
        }
        let psx_seat = (self.shelves[self.shelf_at].0 == Platform::Psx
            && matches!(self.phase, Phase::Inserting { .. }))
        .then(|| self.seat());
        let cart = match &mut self.phase {
            Phase::Playing { cart } | Phase::Inserting { cart, .. } => std::mem::take(cart),
            _ => return,
        };
        self.end_link();
        self.close_game_menu();
        self.flush_eject(&cart);
        self.pending = None;
        self.refusal = None;
        self.refused_from = None;
        self.phase = Phase::Ejecting {
            cart,
            t: psx_seat.map_or(-EJECT_HOLD_S, |seat| (1.0 - seat) * PSX_SEATED_AT),
        };
    }

    fn flush_eject(&mut self, stem: &str) {
        self.settle_saves();
        let (Some(root), Some(snapshot)) = (&self.root, &self.snapshot) else {
            return;
        };
        let Some(state) = snapshot.state() else {
            eprintln!("slot: eject: the core gave up no state");
            return;
        };
        let (state, sav) = trusted_write(snapshot.as_ref(), state, "eject");
        let state = state.filter(|_| self.state.eject_save);
        match persist::eject(
            root,
            self.platform,
            self.core,
            stem,
            state.as_deref(),
            sav.as_deref(),
        ) {
            Ok(()) => {
                self.state.cart = None;
                self.state.cart_platform = None;
            }
            Err(e) => eprintln!("slot: eject: {e}"),
        }
    }

    fn doze(&mut self) {
        if self.link_active() {
            self.end_link();
        }
        self.close_game_menu();
        self.core_picker = None;
        if matches!(self.phase, Phase::Doze { .. }) {
            return;
        }
        self.state_writer.flush();
        self.flush_resume();
        // A closed-lid undock restarts with the saved cart still inserting.
        // Preserve that load transition so opening the lid can finish it.
        self.sleep_inserting = if matches!(self.phase, Phase::Inserting { .. }) {
            Some(std::mem::replace(&mut self.phase, Phase::Shelf))
        } else {
            None
        };
        let cart = match &mut self.phase {
            Phase::Playing { cart } | Phase::Polaroids { cart } => Some(std::mem::take(cart)),
            _ => match &self.sleep_inserting {
                Some(Phase::Inserting { cart, .. }) => Some(cart.clone()),
                _ => None,
            },
        };
        self.polaroids = None;
        self.phase = Phase::Doze { cart };
        self.dozed_at = self.now();
        self.radio.ask(RadioJob::Cool);
        if let Some(power) = &mut self.power {
            power.on_close();
            power.set_screen_off(false);
        }
        self.screen_off = false;
    }

    fn wake(&mut self) {
        let Phase::Doze { cart } = &mut self.phase else {
            return;
        };
        self.phase = self
            .sleep_inserting
            .take()
            .unwrap_or_else(|| match cart.take() {
                Some(cart) => Phase::Playing { cart },
                None => Phase::Shelf,
            });
        self.lid_sleeping = false;
        if let Some(power) = &mut self.power {
            power.on_open();
            if self.lid_closed {
                power.on_close();
            }
        }
    }

    pub fn on_doze_timeout(&mut self) {
        if !matches!(self.phase, Phase::Doze { .. })
            || self.state.sleep_shutdown == slot_store::SleepShutdown::Never
            || (!self.state.shutdown_charging
                && self
                    .battery
                    .is_some_and(|b| matches!(b.charge, Charge::Charging | Charge::Full)))
        {
            return;
        }
        self.begin_power_off();
    }

    fn power_press(&mut self) {
        if std::mem::take(&mut self.woke_on_press) {
            return;
        }
        match self.phase {
            Phase::Doze { .. } => self.wake(),
            _ => self.doze(),
        }
    }

    fn open_power_menu(&mut self) {
        if self.power_menu.is_some() {
            return;
        }
        if self.link_active() {
            return self.refuse();
        }
        self.stop_playback();
        self.flush_resume();
        self.power_menu = Some(0);
    }

    fn power_menu_input(&mut self, action: Action) {
        let Some(index) = self.power_menu else {
            return;
        };
        let last = PowerChoice::ALL.len() - 1;
        match action {
            Action::GbaDown(Btn::Up) => {
                self.power_menu = Some(if index == 0 { last } else { index - 1 })
            }
            Action::GbaDown(Btn::Down) => {
                self.power_menu = Some(if index >= last { 0 } else { index + 1 })
            }
            Action::GbaDown(Btn::B) => self.power_menu = None,
            Action::GbaDown(Btn::A) => {
                self.power_menu = None;
                self.close_game_menu();
                match PowerChoice::ALL[index] {
                    PowerChoice::Restart => {
                        self.restarting = true;
                        self.act_at = self.now() + SHUTDOWN_SHOW_MS;
                        self.set_led(LedState::Off);
                    }
                    PowerChoice::PowerOff => self.begin_power_off(),
                }
            }
            _ => {}
        }
    }

    fn open_core_picker(&mut self) {
        let Some(root) = self.root.clone() else {
            return;
        };
        let Some(cart) = self.shelf().carts.get(self.shelf().index) else {
            return;
        };
        if cart.platform != Platform::Gba {
            return;
        }
        let seat = slot_store::core_for(&root, &cart.stem);
        let now = self.now();
        let mut picker = CorePicker::open(seat, now);
        if self.core_faces_ready() {
            picker.start(now);
        }
        self.core_picker = Some(picker);
        self.shelf_mut().release_hold();
        self.play_held = None;
    }

    fn core_faces_ready(&self) -> bool {
        self.core_faces_stem
            .as_deref()
            .is_some_and(|stem| self.selected_stem() == Some(stem))
    }

    fn core_picker_input(&mut self, action: Action) {
        let press = match action {
            Action::GbaDown(Btn::Left) | Action::ShelfLeft => Press::Left,
            Action::GbaDown(Btn::Right) | Action::ShelfRight => Press::Right,
            Action::GbaDown(Btn::A) => Press::Keep,
            Action::GbaDown(Btn::B) => Press::Back,
            _ => return,
        };
        let now = self.now();
        let Some(picker) = &mut self.core_picker else {
            return;
        };
        let outcome = picker.press(press, now);
        if let Outcome::Write(core) = outcome {
            self.write_core(core);
        }
    }

    fn open_game_menu(&mut self) {
        if self.platform == Platform::Psx {
            self.disc_menu = Some(1);
            return;
        }
        if self.game_menu.is_some() {
            return;
        }
        if self.link_active() {
            return self.refuse();
        }
        if self.core == Core::Mgba {
            let wireless = self
                .seated()
                .is_some_and(|stem| self.link_mode(stem).0 == LinkKind::Wireless);
            if wireless {
                self.hud.toast(Toast::NeedsGpsp, self.now());
                return;
            }
            self.link_hardware = LinkKind::Cable;
            self.radio.ask(RadioJob::Warm);
            self.game_menu = Some(GameMenu::Pick(self.last_role));
            return;
        }
        if self.platform != Platform::Gba {
            self.hud.toast(Toast::NoLink, self.now());
            return;
        }
        let carried = self
            .seated()
            .and_then(|stem| self.auto_link(stem))
            .is_some_and(|(cart, _)| link_carried(&cart.code, &cart.title));
        if !carried {
            self.hud.toast(Toast::NoLink, self.now());
            return;
        }
        if self.core != Core::Gpsp {
            self.hud.toast(Toast::NeedsGpsp, self.now());
            return;
        }
        let hardware = self
            .seated()
            .map_or(LinkKind::Cable, |stem| self.link_mode(stem).0);
        self.link_hardware = hardware;
        self.radio.ask(RadioJob::Warm);
        self.game_menu = Some(GameMenu::Pick(self.last_role));
    }

    fn game_menu_shortcut(&mut self) {
        let Some(client_id) = self.link_client_id() else {
            return self.open_game_menu();
        };
        let now = self.now();
        self.game_menu = Some(GameMenu::Linked {
            role: LinkRow::from_client_id(client_id),
            worked: now,
            since: now,
            opened: true,
        });
    }

    fn end_link_from_menu(&mut self) {
        let role = self
            .link_client_id()
            .map_or(self.last_role, LinkRow::from_client_id);
        self.end_link();
        self.hud.toast(Toast::LinkEnded, self.now());
        self.unplug(role);
    }

    fn unplug(&mut self, role: LinkRow) {
        self.game_menu = Some(GameMenu::Unplug {
            role,
            since: self.now(),
        });
    }

    pub fn set_radio_jobs(&mut self, jobs: Box<dyn RadioJobs>) {
        self.radio = jobs;
    }

    fn game_menu_input(&mut self, action: Action) {
        let Some(menu) = self.game_menu else {
            return;
        };
        match menu {
            GameMenu::Pick(role) => match action {
                Action::GbaDown(Btn::Left) | Action::GbaDown(Btn::Right) => {
                    self.last_role = role.other();
                    self.game_menu = Some(GameMenu::Pick(role.other()));
                }
                Action::GbaDown(Btn::Select) => self.switch_hardware(),
                Action::GbaDown(Btn::A) => self.pick_link(role),
                Action::GbaDown(Btn::B) | Action::GameMenu => self.close_game_menu(),
                _ => {}
            },
            GameMenu::Working { .. } => {
                if action == Action::GbaDown(Btn::B) {
                    if let Some(starting) = &mut self.starting {
                        starting.starter.cancel();
                    }
                    if let Some(reload) = &mut self.reload {
                        reload.cancelled = true;
                    }
                }
            }
            GameMenu::Linked { opened: true, .. } => match action {
                Action::GbaDown(Btn::A) => self.end_link_from_menu(),
                Action::GbaDown(Btn::B) | Action::GameMenu => self.close_game_menu(),
                _ => {}
            },
            GameMenu::Linked { .. } => {}
            GameMenu::Failed { .. } => {
                if matches!(
                    action,
                    Action::GbaDown(Btn::A) | Action::GbaDown(Btn::B) | Action::GameMenu
                ) {
                    self.close_game_menu();
                }
            }
            GameMenu::Unplug { .. } => {}
        }
    }

    pub fn start_link(&mut self, starter: LinkStarter, client_id: u16) {
        self.start_link_from(starter, client_id, self.now());
    }

    fn start_link_from(&mut self, starter: LinkStarter, client_id: u16, since: Millis) {
        if let Some(mut old) = self.starting.replace(LinkStarting { starter, client_id }) {
            old.starter.cancel();
        }
        self.game_menu = Some(GameMenu::Working {
            role: LinkRow::from_client_id(client_id),
            step: LinkStep::Radio,
            since,
        });
    }

    fn switch_hardware(&mut self) {
        let Some(stem) = self.seated().map(str::to_string) else {
            return;
        };
        if !self.link_switchable(&stem) {
            return self.refuse();
        }
        let other = self.link_hardware.other();
        self.link_hardware = other;
        self.link_choices.insert(stem, other);
    }

    fn link_switchable(&self, stem: &str) -> bool {
        self.auto_link(stem).is_some_and(|(cart, auto)| {
            serial_option(self.link_hardware.other(), auto, &cart.code, &cart.title)
                != serial_option(self.link_hardware, auto, &cart.code, &cart.title)
        })
    }

    fn pick_link(&mut self, role: LinkRow) {
        let Some(stem) = self.seated().map(str::to_string) else {
            return;
        };
        let loaded = self.link_loaded.unwrap_or("auto");
        if self.core == Core::Mgba {
            let player = role.client_id() as u8;
            if self.link_player == Some(player) {
                return self.start_link(
                    LinkStarter::spawn(role.role(), link_port()),
                    role.client_id(),
                );
            }
            if self.snapshot.as_ref().is_some_and(|s| !s.resume_trusted()) {
                return self.refuse();
            }
            self.link_player = Some(player);
            self.link_reload = Some((stem.clone(), loaded));
            self.reload = Some(Reload {
                stem,
                role,
                cancelled: false,
                from: self.link_hardware,
                from_serial: loaded,
                fallback: false,
            });
            self.game_menu = Some(GameMenu::Working {
                role,
                step: LinkStep::Radio,
                since: self.now(),
            });
            return;
        }
        let (_, serial) = self.link_mode(&stem);
        if serial == loaded {
            return self.start_link(
                LinkStarter::spawn(role.role(), link_port()),
                role.client_id(),
            );
        }
        if self.snapshot.as_ref().is_some_and(|s| !s.resume_trusted()) {
            return self.refuse();
        }
        self.link_reload = Some((stem.clone(), serial));
        self.reload = Some(Reload {
            stem,
            role,
            cancelled: false,
            from: self.link_hardware.other(),
            from_serial: loaded,
            fallback: false,
        });
        self.game_menu = Some(GameMenu::Working {
            role,
            step: LinkStep::Radio,
            since: self.now(),
        });
    }

    fn refuse_seated(&mut self) {
        self.close_game_menu();
        self.pending = None;
        let caught = self.seat();
        let cart = match &mut self.phase {
            Phase::Playing { cart } => Some(std::mem::take(cart)),
            Phase::Doze { cart } => {
                *cart = None;
                None
            }
            _ => None,
        };
        if let Some(cart) = cart {
            self.refuse_out(cart, caught);
        }
    }

    fn close_game_menu(&mut self) {
        self.stop_playback();
        self.state_menu = None;
        self.players_open = false;
        self.state_slot_menu = None;
        self.disc_menu = None;
        self.disc_change = None;
        self.game_menu = None;
        if let Some(mut starting) = self.starting.take() {
            starting.starter.cancel();
        }
        if !self.link_active() {
            self.radio.ask(RadioJob::Cool);
        }
        let uncollected =
            self.link_reload.is_some() && self.reload.as_ref().is_some_and(|r| !r.fallback);
        if uncollected {
            self.link_reload = None;
            self.reload = None;
        } else if let Some(reload) = &mut self.reload {
            reload.cancelled = true;
        }
    }

    fn working_role(&self, client_id: u16) -> (LinkRow, Millis) {
        match self.game_menu {
            Some(GameMenu::Working { role, since, .. }) => (role, since),
            _ => (LinkRow::from_client_id(client_id), self.now()),
        }
    }

    fn poll_link(&mut self) {
        if self.power_menu.is_some() {
            return;
        }
        let Some(mut starting) = self.starting.take() else {
            return;
        };
        match starting.starter.poll() {
            None => self.starting = Some(starting),
            Some(LinkProgress::At(step)) => {
                if let Some(GameMenu::Working { role, since, .. }) = self.game_menu {
                    self.game_menu = Some(GameMenu::Working { role, step, since });
                }
                self.starting = Some(starting);
            }
            Some(LinkProgress::Ready(link)) => {
                let (role, worked) = self.working_role(starting.client_id);
                self.game_menu = Some(GameMenu::Linked {
                    role,
                    worked,
                    since: self.now(),
                    opened: false,
                });
                self.begin_link(starting.client_id);
                self.link_transport = Some((starting.client_id, Box::new(link)));
            }
            Some(LinkProgress::Failed(LinkFail::Cancelled)) => self.game_menu = None,
            Some(LinkProgress::Failed(fail)) => {
                let (role, worked) = self.working_role(starting.client_id);
                self.game_menu = Some(GameMenu::Failed {
                    role,
                    fail,
                    worked,
                    since: self.now(),
                });
            }
        }
    }

    fn write_core(&self, core: Core) {
        let (Some(root), Some(cart)) = (
            self.root.clone(),
            self.shelf().carts.get(self.shelf().index),
        ) else {
            return;
        };
        if let Err(e) = slot_store::write_selected_core(&root, &cart.stem, core) {
            eprintln!("slot: core: could not write selected_core.txt: {e}");
        }
    }

    fn begin_power_off(&mut self) {
        if self.powering_off {
            return;
        }
        if self.link_active() {
            self.end_link();
        }
        self.close_game_menu();
        self.powering_off = true;
        self.act_at = self.now() + SHUTDOWN_SHOW_MS;
        self.set_led(LedState::Off);
    }

    pub fn on_battery(&mut self, b: Battery) {
        self.battery = Some(b);
        if self.shutting_down() {
            return;
        }
        if matches!(b.charge, Charge::Charging | Charge::Full) || b.percent > BATTERY_RECOVERED {
            self.battery_warned = false;
            self.battery_saved = None;
            self.battery_retry_at = 0;
            return;
        }
        if b.percent <= BATTERY_CRITICAL {
            if self.link_active() {
                self.end_link();
            }
            if self.seated().is_some() {
                if let Err(e) = self.battery_checkpoint() {
                    eprintln!("slot: critical battery save: {e}");
                }
            } else {
                self.settle_saves();
            }
            self.begin_power_off();
            return;
        }
        if b.percent > BATTERY_SAVE {
            return;
        }
        if !self.battery_warned {
            self.hud.toast(Toast::BatteryLow, self.now());
            self.battery_warned = true;
        }
        // Link snapshots and games still loading are not safe resume points.
        if self.link_active() || self.now() < self.battery_retry_at {
            return;
        }
        let Some(cart) = self.seated() else {
            return;
        };
        let key = (self.platform, self.core, cart.to_owned());
        if self.battery_saved.as_ref() == Some(&key) {
            return;
        }
        match self.battery_checkpoint() {
            Ok(()) => {
                self.battery_saved = Some(key);
                self.hud.toast(Toast::BatterySaved, self.now());
            }
            Err(e) => {
                eprintln!("slot: low battery save: {e}");
                self.battery_retry_at = self.now() + BATTERY_RETRY_MS;
                self.hud.toast(Toast::BatterySaveFailed, self.now());
            }
        }
    }

    fn battery_checkpoint(&mut self) -> std::io::Result<()> {
        self.settle_saves();
        let (Some(root), Some(snapshot), Some(cart)) = (&self.root, &self.snapshot, self.seated())
        else {
            return Err(std::io::Error::other("no game snapshot available"));
        };
        if !snapshot.resume_trusted() || !snapshot.save_ram_trusted() {
            return Err(std::io::Error::other("core refused its saved data"));
        }
        let state = snapshot.state();
        // Preserve battery RAM even if serialization fails or the resume path
        // cannot be written. Never report success without a durable resume.
        if let Some(sav) = snapshot.save_ram() {
            persist::write_sav(root, self.platform, cart, &sav)?;
        }
        let state = state.ok_or_else(|| std::io::Error::other("core gave no resume"))?;
        StateRing::new(root, self.platform, self.core, cart).write_resume(&state)?;
        self.autosave_at = self.now() + AUTOSAVE_MS;
        Ok(())
    }

    fn doze_expired(&self) -> bool {
        if self.link_active() {
            return false;
        }
        let (Phase::Doze { .. }, Some(power)) = (&self.phase, &self.power) else {
            return false;
        };
        if power.timeout().is_zero()
            || (!self.state.shutdown_charging
                && self
                    .battery
                    .is_some_and(|b| matches!(b.charge, Charge::Charging | Charge::Full)))
        {
            return false;
        }
        self.now().saturating_sub(self.dozed_at) >= power.timeout().as_millis() as Millis
    }

    pub fn settle_saves(&mut self) {
        self.state_writer.flush();
        if let Some(h) = self.pending_save.take() {
            let _ = h.join();
        }
    }

    fn autosave(&mut self) {
        self.autosave_at = self.now() + AUTOSAVE_MS;
        self.settle_saves();
        let (Some(root), Some(snapshot), Some(cart)) = (&self.root, &self.snapshot, self.seated())
        else {
            return;
        };
        let Some(state) = snapshot.state() else {
            eprintln!("slot: autosave: the core gave up no state");
            return;
        };
        let (state, sav) = trusted_write(snapshot.as_ref(), state, "autosave");
        let (root, platform, core, cart) =
            (root.clone(), self.platform, self.core, cart.to_owned());
        let write = move || {
            if let Err(e) = persist::flush(
                &root,
                platform,
                core,
                &cart,
                state.as_deref(),
                sav.as_deref(),
            ) {
                eprintln!("slot: autosave: {e}");
            }
        };
        match std::thread::Builder::new()
            .name("slot-autosave".into())
            .spawn(write)
        {
            Ok(h) => self.pending_save = Some(h),
            Err(e) => eprintln!("slot: autosave: no writer thread: {e}"),
        }
    }

    pub fn flush_resume(&mut self) {
        self.settle_saves();
        self.autosave_at = self.now() + AUTOSAVE_MS;
        let (Some(root), Some(snapshot), Some(cart)) = (&self.root, &self.snapshot, self.seated())
        else {
            return;
        };
        let Some(state) = snapshot.state() else {
            eprintln!("slot: flush: the core gave up no state");
            return;
        };
        let (state, sav) = trusted_write(snapshot.as_ref(), state, "flush");
        if let Err(e) = persist::flush(
            root,
            self.platform,
            self.core,
            cart,
            state.as_deref(),
            sav.as_deref(),
        ) {
            eprintln!("slot: flush: {e}");
        }
    }

    pub fn checkpoint_for_display(&mut self) -> std::io::Result<bool> {
        // Keep links and animation/core-load transitions alive; retry once the
        // game is stable. A dozing game already has a resume, but should stay asleep.
        if self.link_active()
            || self.shutting_down()
            || matches!(
                self.phase,
                Phase::Inserting { .. } | Phase::Ejecting { .. } | Phase::Doze { .. }
            )
        {
            return Ok(false);
        }
        self.settle_saves();
        if let Some(cart) = self.seated() {
            let (Some(root), Some(snapshot)) = (&self.root, &self.snapshot) else {
                return Err(std::io::Error::other(
                    "no snapshot available for HDMI restart",
                ));
            };
            if !snapshot.resume_trusted() || !snapshot.save_ram_trusted() {
                return Err(std::io::Error::other(
                    "core refused its saved data; HDMI restart deferred",
                ));
            }
            let state = snapshot
                .state()
                .ok_or_else(|| std::io::Error::other("core gave no resume for HDMI restart"))?;
            let sav = snapshot.save_ram();
            persist::flush(
                root,
                self.platform,
                self.core,
                cart,
                Some(&state),
                sav.as_deref(),
            )?;
        }
        if let Some(root) = &self.root {
            write_slot_state(root, &self.state)?;
        }
        Ok(true)
    }

    fn ring(&self) -> Option<StateRing> {
        let (Some(root), Some(cart)) = (&self.root, self.seated()) else {
            return None;
        };
        Some(StateRing::new(root, self.platform, self.core, cart))
    }

    fn seated(&self) -> Option<&str> {
        match &self.phase {
            Phase::Playing { cart } | Phase::Polaroids { cart } => Some(cart),
            _ => None,
        }
    }

    fn entries(&self) -> Vec<StateEntry> {
        self.ring().and_then(|r| r.list().ok()).unwrap_or_default()
    }

    fn open_polaroids(&mut self) {
        if self.link_active() {
            return self.refuse();
        }
        let entries = self.entries();
        if entries.is_empty() {
            return self.refuse();
        }
        let Phase::Playing { cart } = &mut self.phase else {
            return;
        };
        let cart = std::mem::take(cart);
        let mut p = Polaroids::new(entries);
        p.set_undo(self.undo_label());
        self.polaroids = Some(p);
        self.phase = Phase::Polaroids { cart };
        self.push_hint_faces();
    }

    fn close_polaroids(&mut self) {
        let Phase::Polaroids { cart } = &mut self.phase else {
            return;
        };
        let cart = std::mem::take(cart);
        self.polaroids = None;
        self.phase = Phase::Playing { cart };
    }

    fn load_selected(&mut self) {
        let state = self
            .polaroids
            .as_ref()
            .and_then(|p| p.selected())
            .map(|e| e.state.clone());
        let refused = state.map(|state| self.load_file(&state)) == Some(false);
        if !refused {
            self.close_polaroids();
        }
    }

    fn delete_selected(&mut self) {
        let stamp = self
            .polaroids
            .as_ref()
            .and_then(|p| p.selected())
            .map(|e| e.stamp.clone());
        let (Some(stamp), Some(ring)) = (stamp, self.ring()) else {
            return;
        };
        if let Err(e) = ring.remove(&stamp) {
            eprintln!("slot: delete: {e}");
            return;
        }
        if self.undo_targets(&stamp) {
            self.pending = None;
        }
        let Some(p) = &mut self.polaroids else {
            return;
        };
        p.remove_selected();
        if p.is_empty() {
            self.close_polaroids();
        }
    }

    fn undo_targets(&self, stamp: &str) -> bool {
        match self.pending.as_ref() {
            Some((PendingUndo::Save { stamp: pending, .. }, _)) => pending == stamp,
            _ => false,
        }
    }

    fn load_newest(&mut self) {
        let Some(newest) = self.entries().first().map(|e| e.state.clone()) else {
            return self.refuse();
        };
        self.load_file(&newest);
    }

    fn load_file(&mut self, state: &Path) -> bool {
        if !self.may_load_state() {
            self.refuse();
            return false;
        }
        let Some(snapshot) = &self.snapshot else {
            return false;
        };
        let bytes = match std::fs::read(state) {
            Ok(bytes) => bytes,
            Err(e) => {
                eprintln!("slot: load: {e}");
                return false;
            }
        };
        let prior = snapshot.state();
        snapshot.load(bytes);
        self.hud.toast(Toast::StateLoaded, self.now());
        if let Some(prior) = prior {
            self.pending = Some((PendingUndo::Load { prior }, self.now()));
        }
        true
    }

    fn save_state(&mut self) {
        let (Some(ring), Some(snapshot)) = (self.ring(), &self.snapshot) else {
            return;
        };
        if !snapshot.resume_trusted() {
            eprintln!("slot: save: the core refused the resume it was given, not pushing a state");
            return self.refuse();
        }
        let Some(state) = snapshot.state() else {
            eprintln!("slot: save: the core gave up no state");
            return;
        };
        let thumb = snapshot.thumb().unwrap_or_default();
        let stamp = free_stamp(&ring, self.wall_secs());
        let evicted = doomed(&ring);
        if let Err(e) = ring.push(&state, &thumb, &stamp) {
            eprintln!("slot: save: {e}");
            return;
        }
        self.hud.toast(Toast::StateSaved, self.now());
        self.pending = Some((PendingUndo::Save { stamp, evicted }, self.now()));
    }

    fn save_slot(&mut self, slot: usize) -> bool {
        let (Some(ring), Some(snapshot)) = (self.ring(), &self.snapshot) else {
            return false;
        };
        if !snapshot.resume_trusted() {
            self.refuse();
            return false;
        }
        let Some(state) = snapshot.state() else {
            eprintln!("slot: save slot: the core gave up no state");
            self.refuse();
            return false;
        };
        // Read the old state before replacing it so a failed read cannot destroy
        // a save, and the existing undo grace period can restore an overwrite.
        let prior = match ring.read_slot(slot) {
            Ok(prior) => prior,
            Err(e) => {
                eprintln!("slot: save slot: {e}");
                self.refuse();
                return false;
            }
        };
        let prior_modified = ring
            .slot_path(slot)
            .and_then(std::fs::metadata)
            .and_then(|metadata| metadata.modified())
            .ok();
        if let Err(e) = ring.write_slot(slot, &state) {
            eprintln!("slot: save slot: {e}");
            self.refuse();
            return false;
        }
        self.pending = Some((
            PendingUndo::SaveSlot {
                slot,
                prior,
                prior_modified,
            },
            self.now(),
        ));
        self.hud.toast(Toast::StateSaved, self.now());
        true
    }

    fn load_slot(&mut self, slot: usize) -> bool {
        let Some(path) = self.ring().and_then(|r| r.slot_path(slot).ok()) else {
            return false;
        };
        if !path.is_file() {
            self.refuse();
            return false;
        }
        self.load_file(&path)
    }

    pub fn undo_available(&self, now: Millis) -> bool {
        self.pending
            .as_ref()
            .is_some_and(|(_, at)| now.saturating_sub(*at) <= UNDO_GRACE_MS)
    }

    pub fn undo_label(&self) -> Option<&'static str> {
        if !self.undo_available(self.now()) {
            return None;
        }
        match self.pending.as_ref()?.0 {
            PendingUndo::Save { .. } | PendingUndo::SaveSlot { .. } => Some("undo save"),
            PendingUndo::Load { .. } => Some("undo load"),
        }
    }

    pub fn set_legend_faces(&mut self, faces: Vec<TexId>) {
        self.legend_faces = faces;
        self.push_hint_faces();
    }

    pub fn set_undo_face(&mut self, face: Option<TexId>) {
        self.undo_face = face;
        self.push_hint_faces();
    }

    fn push_hint_faces(&mut self) {
        let mut faces = self.legend_faces.clone();
        faces.extend(self.undo_face);
        if let Some(p) = &mut self.polaroids {
            p.set_hint_faces(faces);
        }
    }

    pub fn set_icon_faces(&mut self, faces: Vec<TexId>) {
        self.hud.set_icons(faces);
    }

    pub fn set_toast_faces(&mut self, faces: Vec<TexId>) {
        self.hud.set_toasts(faces);
    }

    pub fn toast(&self) -> Option<Toast> {
        self.hud.said(self.now())
    }

    pub fn undo(&mut self, now: Millis) {
        if !self.undo_available(now) {
            self.pending = None;
            return;
        }
        if matches!(&self.pending, Some((PendingUndo::Load { .. }, _))) && !self.may_load_state() {
            return self.refuse();
        }
        let Some((what, _)) = self.pending.take() else {
            return;
        };
        match what {
            PendingUndo::SaveSlot {
                slot,
                prior,
                prior_modified,
            } => {
                if let Some(ring) = self.ring() {
                    let result = match prior {
                        Some(state) => ring.write_slot(slot, &state).and_then(|()| {
                            if let Some(modified) = prior_modified {
                                std::fs::OpenOptions::new()
                                    .write(true)
                                    .open(ring.slot_path(slot)?)?
                                    .set_modified(modified)?;
                            }
                            Ok(())
                        }),
                        None => ring.remove_slot(slot),
                    };
                    if let Err(e) = result {
                        eprintln!("slot: undo save slot: {e}");
                    }
                }
            }
            PendingUndo::Save { stamp, evicted } => self.undo_save(&stamp, evicted),
            PendingUndo::Load { prior } => {
                if let Some(snapshot) = &self.snapshot {
                    snapshot.load(prior);
                }
            }
        }
        self.close_polaroids();
    }

    fn undo_save(&self, stamp: &str, evicted: Option<(String, Vec<u8>, Vec<u8>)>) {
        let Some(ring) = self.ring() else {
            return;
        };
        if let Err(e) = ring.remove(stamp) {
            eprintln!("slot: undo: {e}");
            return;
        }
        let Some((stamp, state, thumb)) = evicted else {
            return;
        };
        if let Err(e) = ring.push(&state, &thumb, &stamp) {
            eprintln!("slot: undo: {e}");
        }
    }

    pub fn polaroid_entries(&self) -> &[StateEntry] {
        match &self.polaroids {
            Some(p) => &p.entries,
            None => &[],
        }
    }

    pub fn set_polaroid_faces(&mut self, faces: Vec<TexId>) {
        if let Some(p) = &mut self.polaroids {
            p.set_faces(faces);
        }
    }

    pub fn polaroid_stamp(&self) -> Option<&str> {
        self.polaroids
            .as_ref()
            .and_then(|p| p.selected())
            .map(|e| e.stamp.as_str())
    }

    pub fn polaroid_title(&self, now: &str) -> String {
        self.polaroids
            .as_ref()
            .map_or_else(String::new, |p| p.title(now))
    }

    pub fn set_polaroid_title_face(&mut self, face: TexId) {
        if let Some(p) = &mut self.polaroids {
            p.set_title_face(Some(face));
        }
    }
}

fn trusted_write(
    snapshot: &dyn Snapshot,
    state: Vec<u8>,
    verb: &str,
) -> (Option<Vec<u8>>, Option<Vec<u8>>) {
    let state = if snapshot.resume_trusted() {
        Some(state)
    } else {
        eprintln!(
            "slot: {verb}: the core refused the resume it was given, not overwriting the saved one"
        );
        None
    };
    let sav = snapshot.save_ram();
    let sav = if snapshot.save_ram_trusted() {
        sav
    } else {
        if sav.is_some() {
            eprintln!(
                "slot: {verb}: the core refused the save ram it was given, not overwriting the saved one"
            );
        }
        None
    };
    (state, sav)
}

fn up(level: u8, step: u8, max: u8) -> u8 {
    level.saturating_add(step).min(max)
}

fn shader_value(shader: Shader) -> QuickValue {
    match shader {
        Shader::Off => QuickValue::Off,
        Shader::Lcd3x => QuickValue::Lcd3x,
        Shader::Grid => QuickValue::Grid,
        Shader::Dot => QuickValue::Dot,
        Shader::Simpletex => QuickValue::Simpletex,
    }
}

fn ff_next(from: u8, right: bool) -> u8 {
    let n = FF_SPEEDS.len();
    let at = FF_SPEEDS.iter().position(|&v| v == from).unwrap_or(0);
    FF_SPEEDS[if right {
        (at + 1) % n
    } else {
        (at + n - 1) % n
    }]
}

fn clock_screen(utc: i64, offset_min: i16, from_menu: bool) -> Phase {
    Phase::SetClock {
        picker: ClockPicker::local(utc, offset_min),
        seed: utc - utc.rem_euclid(60),
        from_menu,
    }
}

fn system_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn doomed(ring: &StateRing) -> Option<(String, Vec<u8>, Vec<u8>)> {
    let entries = ring.list().ok()?;
    let oldest = entries.get(RING_MAX - 1)?;
    let (state, thumb) = ring.read(&oldest.stamp).ok()?;
    Some((oldest.stamp.clone(), state, thumb))
}

fn free_stamp(ring: &StateRing, now: i64) -> String {
    let taken: Vec<String> = ring
        .list()
        .map(|l| l.into_iter().map(|e| e.stamp).collect())
        .unwrap_or_default();
    let mut secs = now;
    let mut stamp = format_stamp(secs);
    while taken.contains(&stamp) {
        secs += 1;
        stamp = format_stamp(secs);
    }
    stamp
}

const STATE_ROWS: usize = 8;
const STATE_ROW_RUN_AHEAD: usize = 2;
const STATE_ROW_PLAYERS: usize = 3;
const STATE_ROW_REWIND: usize = 4;
const STATE_ROW_FAST_FORWARD: usize = 5;
const STATE_ROW_RESET: usize = 6;
const STATE_ROW_BACK: usize = 7;

fn centred_top(rows: usize) -> f32 {
    (OUT_H as f32 - POWER_MENU_PITCH * rows as f32) / 2.0
}

fn draw_menu_rows(
    faces: &[(TexId, u32, u32)],
    index: Option<usize>,
    top: f32,
    out: &mut Vec<Draw>,
) {
    for (row, (tex, w, h)) in faces.iter().copied().enumerate() {
        let y = top + POWER_MENU_PITCH * row as f32;
        let x = ((OUT_W as f32 - w as f32) / 2.0).round();
        if index == Some(row) {
            out.push(Draw::Rect {
                x,
                y: y + POWER_MENU_BAR_INSET,
                w: w as f32,
                h: POWER_MENU_PITCH - 2.0 * POWER_MENU_BAR_INSET,
                colour: slot_ui::edge(),
            });
        }
        out.push(Draw::Tex {
            x,
            y: y + (POWER_MENU_PITCH - h as f32) / 2.0,
            w: w as f32,
            h: h as f32,
            tex,
            alpha: 1.0,
        });
    }
}
